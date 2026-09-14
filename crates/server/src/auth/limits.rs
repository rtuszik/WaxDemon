use super::AuthSession;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_client_addr::{ClientIpConfig, ClientIpSource};
use governor::{
    Quota, RateLimiter,
    clock::{Clock, DefaultClock, Reference},
    state::{InMemoryState, NotKeyed},
};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const IDLE_TTL: Duration = Duration::from_secs(60);
const MAX_IDENTITIES: usize = 4096;
type Limiter<C> = RateLimiter<
    NotKeyed,
    InMemoryState,
    C,
    governor::middleware::NoOpMiddleware<<C as Clock>::Instant>,
>;

#[derive(Clone)]
pub(super) struct RequestLimits {
    oauth: Arc<Budget>,
    dashboard: Arc<Budget>,
    trusted_proxies: ClientIpConfig,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self {
            oauth: Arc::new(Budget::new(30, 4, 10, 2, DefaultClock::default())),
            dashboard: Arc::new(Budget::new(120, 8, 30, 2, DefaultClock::default())),
            trusted_proxies: ClientIpConfig::default(),
        }
    }
}

impl RequestLimits {
    pub(super) fn with_trusted_proxies(mut self, config: ClientIpConfig) -> Self {
        self.trusted_proxies = config;
        self
    }

    fn peer(&self, request: &Request) -> Option<IpAddr> {
        let peer = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()?
            .0
            .ip();
        let client = self
            .trusted_proxies
            .resolve_client_ip(request.headers(), peer);
        if self.trusted_proxies.is_trusted_proxy(peer) && client.source() == &ClientIpSource::Socket
        {
            return None;
        }
        Some(client.ip())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Identity {
    Peer(IpAddr),
    User(i64),
}

struct Client<C: Clock> {
    rate: Limiter<C>,
    slots: Arc<Semaphore>,
    last_used: C::Instant,
}

struct Budget<C: Clock = DefaultClock> {
    rate: Limiter<C>,
    slots: Arc<Semaphore>,
    clients: Mutex<HashMap<Identity, Client<C>>>,
    client_quota: Quota,
    client_concurrent: usize,
    clock: C,
}

struct Permit {
    _global: OwnedSemaphorePermit,
    _client: OwnedSemaphorePermit,
}

impl<C: Clock + Clone> Budget<C> {
    fn new(
        maximum: u32,
        concurrent: usize,
        client_maximum: u32,
        client_concurrent: usize,
        clock: C,
    ) -> Self {
        Self {
            rate: RateLimiter::direct_with_clock(
                Quota::per_minute(NonZeroU32::new(maximum).unwrap()),
                clock.clone(),
            ),
            slots: Arc::new(Semaphore::new(concurrent)),
            clients: Mutex::default(),
            client_quota: Quota::per_minute(NonZeroU32::new(client_maximum).unwrap()),
            client_concurrent,
            clock,
        }
    }

    fn admit(&self, identity: Identity) -> Result<Permit, u64> {
        let mut clients = self
            .clients
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = self.clock.now();
        clients.retain(|_, client| {
            Duration::from(now.duration_since(client.last_used)) < IDLE_TTL
                || client.slots.available_permits() < self.client_concurrent
        });
        if let Some(client) = clients.get_mut(&identity) {
            return self.check(client);
        }
        if clients.len() >= MAX_IDENTITIES {
            return Err(1);
        }
        let mut client = Client {
            rate: RateLimiter::direct_with_clock(self.client_quota, self.clock.clone()),
            slots: Arc::new(Semaphore::new(self.client_concurrent)),
            last_used: now,
        };
        let permit = self.check(&mut client)?;
        clients.insert(identity, client);
        Ok(permit)
    }

    fn check(&self, client: &mut Client<C>) -> Result<Permit, u64> {
        let global_slot = self.slots.clone().try_acquire_owned().map_err(|_| 1u64)?;
        let client_slot = client.slots.clone().try_acquire_owned().map_err(|_| 1u64)?;
        let retry = |until: governor::NotUntil<C::Instant>| {
            until
                .wait_time_from(self.clock.now())
                .as_secs_f64()
                .ceil()
                .max(1.0) as u64
        };
        client.rate.check().map_err(retry)?;
        client.last_used = self.clock.now();
        self.rate.check().map_err(retry)?;
        Ok(Permit {
            _global: global_slot,
            _client: client_slot,
        })
    }
}

fn throttled(retry: u64) -> Response {
    (
        StatusCode::TOO_MANY_REQUESTS,
        [(header::RETRY_AFTER, retry.to_string())],
        "Too many requests; try again later",
    )
        .into_response()
}

pub(super) async fn oauth(
    State(limits): State<RequestLimits>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method();
    let path = request.uri().path();
    let limited = (method == Method::POST && matches!(path, "/auth/login" | "/auth/reconnect"))
        || ((method == Method::GET || method == Method::HEAD) && path == "/auth/callback");
    let _permit = if limited {
        let Some(peer) = limits.peer(&request) else {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                "Client address unavailable",
            )
                .into_response();
        };
        match limits.oauth.admit(Identity::Peer(peer)) {
            Ok(permit) => Some(permit),
            Err(retry) => return throttled(retry),
        }
    } else {
        None
    };
    next.run(request).await
}

pub(super) async fn dashboard(
    State(limits): State<RequestLimits>,
    auth: AuthSession,
    request: Request,
    next: Next,
) -> Response {
    let reading = request.method() == Method::GET || request.method() == Method::HEAD;
    let _permit = if reading && matches!(request.uri().path(), "/" | "/api/dashboard") {
        if let Some(user) = auth.user.as_ref().filter(|user| user.status == "approved") {
            match limits.dashboard.admit(Identity::User(user.id)) {
                Ok(permit) => Some(permit),
                Err(retry) => return throttled(retry),
            }
        } else {
            None
        }
    } else {
        None
    };
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use governor::clock::FakeRelativeClock;

    fn budget(
        maximum: u32,
        concurrent: usize,
        client_maximum: u32,
        client_concurrent: usize,
    ) -> (Budget<FakeRelativeClock>, FakeRelativeClock) {
        let clock = FakeRelativeClock::default();
        (
            Budget::new(
                maximum,
                concurrent,
                client_maximum,
                client_concurrent,
                clock.clone(),
            ),
            clock,
        )
    }

    #[test]
    fn bursts_replenish_gradually_without_window_reset() {
        let (budget, clock) = budget(30, 4, 10, 2);
        let identity = Identity::User(1);
        for _ in 0..10 {
            drop(budget.admit(identity).unwrap());
        }
        assert_eq!(budget.admit(identity).err(), Some(6));
        clock.advance(Duration::from_millis(5500));
        assert_eq!(budget.admit(identity).err(), Some(1));
        clock.advance(Duration::from_millis(500));
        drop(budget.admit(identity).unwrap());
        assert_eq!(budget.admit(identity).err(), Some(6));
        clock.advance(Duration::from_secs(53));
        for _ in 0..8 {
            drop(budget.admit(identity).unwrap());
        }
        assert!(budget.admit(identity).is_err());
        clock.advance(Duration::from_secs(1));
        drop(budget.admit(identity).unwrap());
        assert!(budget.admit(identity).is_err());
    }

    #[test]
    fn identity_rejections_preserve_global_budget_and_stale_entries_expire() {
        let (budget, clock) = budget(3, 3, 2, 2);
        for _ in 0..2 {
            drop(budget.admit(Identity::User(1)).unwrap());
        }
        for _ in 0..100 {
            assert!(budget.admit(Identity::User(1)).is_err());
        }
        drop(budget.admit(Identity::User(2)).unwrap());
        for id in 3..100 {
            assert!(budget.admit(Identity::User(id)).is_err());
        }
        assert_eq!(budget.clients.lock().unwrap().len(), 2);
        clock.advance(IDLE_TTL);
        drop(budget.admit(Identity::User(3)).unwrap());
        assert_eq!(budget.clients.lock().unwrap().len(), 1);
    }

    #[test]
    fn semaphore_rejections_do_not_charge_rate_and_drop_releases_both_slots() {
        let (budget, _) = budget(4, 2, 3, 1);
        let first = budget.admit(Identity::User(1)).unwrap();
        assert_eq!(budget.admit(Identity::User(1)).err(), Some(1));
        let second = budget.admit(Identity::User(2)).unwrap();
        assert_eq!(budget.admit(Identity::User(3)).err(), Some(1));
        drop(first);
        drop(budget.admit(Identity::User(1)).unwrap());
        drop(second);
        drop(budget.admit(Identity::User(3)).unwrap());
    }

    #[test]
    fn active_clients_survive_idle_cleanup() {
        let (budget, clock) = budget(3, 2, 2, 1);
        let first = budget.admit(Identity::User(1)).unwrap();
        clock.advance(IDLE_TTL);
        assert_eq!(budget.admit(Identity::User(1)).err(), Some(1));
        drop(first);
        drop(budget.admit(Identity::User(1)).unwrap());
    }
}
