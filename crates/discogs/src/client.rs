use crate::error::DiscogsError;
use crate::types::{CollectionPage, CollectionValue, PriceSuggestionsResponse};
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::{debug, info, warn};

pub const DISCOGS_API_BASE_URL: &str = "https://api.discogs.com";
pub const USER_AGENT: &str = "WaxDemonApp/0.1 (+https://github.com/rtuszik/waxdemon)";

/// Fallback interval used only until we've seen our first response. Once the
/// server reports `x-discogs-ratelimit`, pacing switches to that observed
/// quota (60-second window).
pub const DEFAULT_FALLBACK_INTERVAL: Duration = Duration::from_millis(1100);

/// How long to wait when Discogs reports zero remaining quota. The window is
/// a 60-second rolling window, so sleeping this long guarantees at least some
/// quota has rolled back in.
const EMPTY_BUCKET_SLEEP: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default)]
pub struct PacerState {
    pub last_request: Option<Instant>,
    /// Most recently observed `x-discogs-ratelimit-remaining`.
    pub remaining: Option<u32>,
    /// Most recently observed `x-discogs-ratelimit` (total quota per window).
    pub quota: Option<u32>,
}

/// Pure pacing decision — how long to sleep before firing the next request.
/// Split out from the async path so we can unit-test the rules directly.
pub fn required_sleep(st: &PacerState, fallback_interval: Duration, now: Instant) -> Duration {
    if matches!(st.remaining, Some(0)) {
        return EMPTY_BUCKET_SLEEP;
    }
    let target = match st.quota {
        Some(q) if q > 0 => Duration::from_millis(60_000 / u64::from(q)),
        _ => fallback_interval,
    };
    match st.last_request {
        Some(prev) => target.saturating_sub(now.saturating_duration_since(prev)),
        None => Duration::ZERO,
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    token: String,
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    /// If true, skip actually sleeping between retries and between paced requests
    /// (used for tests to keep them fast).
    pub disable_sleep: bool,
    /// Fallback spacing between requests before the first response arrives or
    /// when the server doesn't report a quota. Set to `Duration::ZERO` in tests.
    pub min_interval: Duration,
    /// Shared pacer state — `last_request` timestamp plus the most recently
    /// observed Discogs rate-limit headers. Sharing across clones is why this
    /// is `Arc<Mutex<…>>`: a single `Client::new` produces one pacer that
    /// covers every downstream task.
    pacer: Arc<Mutex<PacerState>>,
}

impl Client {
    pub fn new(token: impl Into<String>) -> Self {
        Self::with_base(token, DISCOGS_API_BASE_URL.to_string())
    }

    pub fn with_base(token: impl Into<String>, base_url: String) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .build()
                .expect("reqwest client"),
            base_url,
            token: token.into(),
            max_retries: 3,
            initial_delay_ms: 1500,
            disable_sleep: false,
            min_interval: DEFAULT_FALLBACK_INTERVAL,
            pacer: Arc::new(Mutex::new(PacerState::default())),
        }
    }

    /// `endpoint` must start with `/` — it's appended to `base_url` verbatim.
    pub async fn request_json<T: for<'de> serde::Deserialize<'de>>(
        &self,
        endpoint: &str,
    ) -> Result<T, DiscogsError> {
        let url = format!("{}{}", self.base_url, endpoint);
        let mut last_error: Option<DiscogsError> = None;

        self.wait_for_slot().await;

        for attempt in 0..=self.max_retries {
            let resp = self
                .http
                .get(&url)
                .header(
                    reqwest::header::AUTHORIZATION,
                    format!("Discogs token={}", self.token),
                )
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .send()
                .await;

            let response = match resp {
                Ok(r) => r,
                Err(e) => {
                    warn!(%e, attempt, "network error");
                    last_error = Some(DiscogsError::Transport(e));
                    if attempt < self.max_retries {
                        self.backoff(attempt, None).await;
                        continue;
                    }
                    break;
                }
            };

            self.record_rate_limit_headers(response.headers()).await;

            let status = response.status();
            if status.is_success() {
                if status.as_u16() == 204 {
                    // 204 has no body; only valid when T deserializes from JSON null.
                    return serde_json::from_value::<T>(serde_json::Value::Null)
                        .map_err(DiscogsError::Parse);
                }
                let bytes = response.bytes().await?;
                return serde_json::from_slice::<T>(&bytes).map_err(DiscogsError::Parse);
            }

            if status.as_u16() == 429 {
                // A 429 implies the bucket is empty, regardless of what the
                // header claimed. Force it so the next caller blocks long
                // enough for the window to roll.
                self.pacer.lock().await.remaining = Some(0);

                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
                warn!(attempt, "rate limited");
                last_error = Some(DiscogsError::RateLimited);
                if attempt < self.max_retries {
                    self.backoff(attempt, retry_after).await;
                    continue;
                }
                break;
            }

            // Non-429 errors are not retried — surface them to the caller.
            let body = response.text().await.unwrap_or_default();
            return Err(DiscogsError::Http {
                status: status.as_u16(),
                body,
            });
        }

        Err(last_error.unwrap_or_else(|| {
            DiscogsError::Retry(format!(
                "no response after {} attempts",
                self.max_retries + 1
            ))
        }))
    }

    /// Serialise outbound requests so they leave at a rate consistent with the
    /// most recently observed Discogs quota (`x-discogs-ratelimit`). Before
    /// the first response, falls back to `self.min_interval`. The mutex is
    /// held across the sleep so a second caller can't race past it.
    async fn wait_for_slot(&self) {
        let mut st = self.pacer.lock().await;
        let sleep_for = required_sleep(&st, self.min_interval, Instant::now());
        if !sleep_for.is_zero() && !self.disable_sleep {
            let ms = sleep_for.as_millis() as u64;
            // Short per-request pacing is noise at INFO; the 60s empty-bucket
            // wait is exactly the thing the user wants to see.
            if ms >= 5_000 {
                info!(
                    sleep_ms = ms,
                    remaining = st.remaining,
                    quota = st.quota,
                    "pausing for rate-limit window to roll"
                );
            } else {
                debug!(sleep_ms = ms, "pacing request");
            }
            tokio::time::sleep(sleep_for).await;
        }
        st.last_request = Some(Instant::now());
    }

    /// Parse Discogs' rate-limit headers off a response and record the numbers
    /// so the next `wait_for_slot` can pace adaptively. Missing or malformed
    /// headers are ignored — we just keep the last known values.
    async fn record_rate_limit_headers(&self, headers: &reqwest::header::HeaderMap) {
        let remaining = header_u32(headers, "x-discogs-ratelimit-remaining");
        let quota = header_u32(headers, "x-discogs-ratelimit");
        if remaining.is_none() && quota.is_none() {
            return;
        }
        let mut st = self.pacer.lock().await;
        if let Some(r) = remaining {
            st.remaining = Some(r);
        }
        if let Some(q) = quota {
            st.quota = Some(q);
        }
    }

    /// Snapshot of current pacer state — remaining quota and last-request time.
    /// Cheap: clones three `Option`s. Useful for progress logging in callers.
    pub async fn pacer_snapshot(&self) -> PacerState {
        self.pacer.lock().await.clone()
    }

    async fn backoff(&self, attempt: u32, retry_after_secs: Option<u64>) {
        // Exponential backoff: initial_delay * 2^attempt, plus 0–999ms jitter.
        // If the server sent Retry-After, honour it when it exceeds our computed delay.
        let base = self.initial_delay_ms.saturating_mul(1u64 << attempt);
        let jitter = rand::rng().random_range(0..1000u64);
        let mut delay_ms = base + jitter;
        if let Some(ra) = retry_after_secs {
            let candidate = ra * 1000 + 500;
            if candidate > delay_ms {
                delay_ms = candidate;
            }
        }
        debug!(delay_ms, attempt, "backing off");
        if !self.disable_sleep {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }
}

fn header_u32(headers: &reqwest::header::HeaderMap, name: &str) -> Option<u32> {
    headers
        .get(name)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
}

/// Follow `pagination.urls.next` extracting `path + query` only.
pub fn extract_next_path(next_url: &str) -> Option<String> {
    url::Url::parse(next_url).ok().map(|u| match u.query() {
        Some(q) => format!("{}?{}", u.path(), q),
        None => u.path().to_string(),
    })
}

pub async fn fetch_collection_page(
    client: &Client,
    endpoint: &str,
) -> Result<CollectionPage, DiscogsError> {
    client.request_json(endpoint).await
}

pub async fn fetch_collection_value(
    client: &Client,
    username: &str,
) -> Result<CollectionValue, DiscogsError> {
    client
        .request_json(&format!("/users/{}/collection/value", username))
        .await
}

/// Price suggestions. Returns `None` when Discogs responds 404 (no suggestions for this release).
pub async fn fetch_price_suggestions(
    client: &Client,
    release_id: i64,
) -> Result<Option<PriceSuggestionsResponse>, DiscogsError> {
    let endpoint = format!("/marketplace/price_suggestions/{}", release_id);
    match client
        .request_json::<PriceSuggestionsResponse>(&endpoint)
        .await
    {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.is_not_found() => Ok(None),
        Err(e) => Err(e),
    }
}
