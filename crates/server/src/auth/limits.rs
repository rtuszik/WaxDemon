use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const WINDOW: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub(super) struct RequestLimits {
    oauth: Arc<Budget>,
    dashboard: Arc<Budget>,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self {
            oauth: Arc::new(Budget::new(30, 4)),
            dashboard: Arc::new(Budget::new(120, 8)),
        }
    }
}

struct Budget {
    maximum: u32,
    window: Mutex<(Instant, u32)>,
    concurrent: Arc<Semaphore>,
}

impl Budget {
    fn new(maximum: u32, concurrent: usize) -> Self {
        Self {
            maximum,
            window: Mutex::new((Instant::now(), 0)),
            concurrent: Arc::new(Semaphore::new(concurrent)),
        }
    }

    fn admit(&self, now: Instant) -> Result<OwnedSemaphorePermit, u64> {
        let permit = self
            .concurrent
            .clone()
            .try_acquire_owned()
            .map_err(|_| 1u64)?;
        let mut window = self
            .window
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let elapsed = now.saturating_duration_since(window.0);
        if elapsed >= WINDOW {
            *window = (now, 0);
        } else if window.1 >= self.maximum {
            return Err((WINDOW - elapsed).as_secs_f64().ceil() as u64);
        }
        window.1 += 1;
        Ok(permit)
    }
}

pub(super) async fn enforce(
    State(limits): State<RequestLimits>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();
    let method = request.method();
    let reading = method == Method::GET || method == Method::HEAD;
    let budget = if (method == Method::POST && matches!(path, "/auth/login" | "/auth/reconnect"))
        || (reading && path == "/auth/callback")
    {
        Some(&limits.oauth)
    } else if reading && matches!(path, "/" | "/api/dashboard") {
        Some(&limits.dashboard)
    } else {
        None
    };
    let _permit = match budget.map(|budget| budget.admit(Instant::now())) {
        Some(Ok(permit)) => Some(permit),
        Some(Err(retry)) => {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                [(header::RETRY_AFTER, retry.to_string())],
                "Too many requests; try again later",
            )
                .into_response();
        }
        None => None,
    };
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::Request,
        middleware,
        routing::{get, post},
    };
    use tower::ServiceExt;

    #[test]
    fn rate_budget_recovers_at_window_boundary_and_is_shared() {
        let budget = Arc::new(Budget::new(2, 1));
        let other = budget.clone();
        let start = budget.window.lock().unwrap().0;
        drop(budget.admit(start).unwrap());
        drop(other.admit(start + Duration::from_secs(1)).unwrap());
        assert_eq!(
            budget
                .admit(start + Duration::from_millis(1500))
                .unwrap_err(),
            59
        );
        drop(other.admit(start + WINDOW).unwrap());
    }

    #[test]
    fn concurrency_rejection_does_not_spend_rate_budget_and_drop_releases_slot() {
        let budget = Budget::new(2, 1);
        let now = Instant::now();
        let first = budget.admit(now).unwrap();
        assert_eq!(budget.admit(now).unwrap_err(), 1);
        drop(first);
        drop(budget.admit(now).unwrap());
        assert!(budget.admit(now).is_err());
    }

    #[tokio::test]
    async fn routes_share_budgets_but_health_and_assets_remain_available() {
        let limits = RequestLimits {
            oauth: Arc::new(Budget::new(1, 1)),
            dashboard: Arc::new(Budget::new(1, 1)),
        };
        let app = Router::new()
            .route("/auth/login", post(|| async {}))
            .route("/auth/reconnect", post(|| async {}))
            .route("/auth/callback", get(|| async {}))
            .route("/api/dashboard", get(|| async {}))
            .route("/", get(|| async {}))
            .route("/health/live", get(|| async {}))
            .route("/assets/test", get(|| async {}))
            .layer(middleware::from_fn_with_state(limits, enforce));
        for (method, path, status) in [
            ("POST", "/auth/login", StatusCode::OK),
            ("POST", "/auth/reconnect", StatusCode::TOO_MANY_REQUESTS),
            ("GET", "/auth/callback", StatusCode::TOO_MANY_REQUESTS),
            ("GET", "/api/dashboard?range=all", StatusCode::OK),
            ("HEAD", "/", StatusCode::TOO_MANY_REQUESTS),
            ("GET", "/health/live", StatusCode::OK),
            ("GET", "/assets/test", StatusCode::OK),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status, "{method} {path}");
            if status == StatusCode::TOO_MANY_REQUESTS {
                assert!(
                    response.headers()[header::RETRY_AFTER]
                        .to_str()
                        .unwrap()
                        .parse::<u64>()
                        .unwrap()
                        > 0
                );
            }
        }
    }
}
