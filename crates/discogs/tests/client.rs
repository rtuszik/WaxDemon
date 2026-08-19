use std::time::Duration;

use serde_json::json;
use tokio::time::Instant;
use waxdemon_discogs::client::{
    Client, DEFAULT_FALLBACK_INTERVAL, PacerState, extract_next_path, required_sleep,
};
use waxdemon_discogs::{DiscogsError, fetch_price_suggestions};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const TOKEN: &str = "test-token-123";

fn fast_client(base_url: String) -> Client {
    let mut c = Client::with_base(TOKEN, base_url);
    c.initial_delay_ms = 1;
    c.disable_sleep = true;
    c.min_interval = Duration::ZERO;
    c
}

#[tokio::test]
async fn success_response_parsed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/echo"))
        .and(header(
            "Authorization",
            format!("Discogs token={}", TOKEN).as_str(),
        ))
        .and(header(
            "User-Agent",
            "WaxDemonApp/0.1 (+https://github.com/rtuszik/waxdemon)",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"hello":"world"})))
        .expect(1)
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let v: serde_json::Value = client.request_json("/echo").await.unwrap();
    assert_eq!(v["hello"], "world");
}

#[tokio::test]
async fn retries_on_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rl"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "1"))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rl"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let v: serde_json::Value = client.request_json("/rl").await.unwrap();
    assert_eq!(v["ok"], true);
}

#[tokio::test]
async fn gives_up_after_max_retries_on_persistent_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/always-rl"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let err = client
        .request_json::<serde_json::Value>("/always-rl")
        .await
        .unwrap_err();
    assert!(matches!(err, DiscogsError::RateLimited));
}

#[tokio::test]
async fn non_429_error_breaks_immediately_without_retrying() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("internal"))
        .expect(1)
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let err = client
        .request_json::<serde_json::Value>("/boom")
        .await
        .unwrap_err();
    match err {
        DiscogsError::Http { status, body } => {
            assert_eq!(status, 500);
            assert_eq!(body, "internal");
        }
        other => panic!("expected Http error, got {other:?}"),
    }
}

#[tokio::test]
async fn price_suggestions_returns_none_on_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .expect(1)
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let out = fetch_price_suggestions(&client, 42).await.unwrap();
    assert!(out.is_none());
}

#[tokio::test]
async fn price_suggestions_happy_path() {
    let server = MockServer::start().await;
    let body = json!({
        "Near Mint (NM or M-)": {"currency": "USD", "value": 25.5},
        "Very Good (VG)":       {"currency": "USD", "value": 10.0},
    });
    Mock::given(method("GET"))
        .and(path("/marketplace/price_suggestions/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .expect(1)
        .mount(&server)
        .await;

    let client = fast_client(server.uri());
    let out = fetch_price_suggestions(&client, 99).await.unwrap().unwrap();
    assert!((out.get("Near Mint (NM or M-)").unwrap().value - 25.5).abs() < 1e-9);
    assert_eq!(out.get("Very Good (VG)").unwrap().currency, "USD");
}

#[test]
fn extract_next_path_strips_host() {
    let n = extract_next_path(
        "https://api.discogs.com/users/x/collection/folders/0/releases?per_page=100&page=2",
    )
    .unwrap();
    assert_eq!(
        n,
        "/users/x/collection/folders/0/releases?per_page=100&page=2"
    );
}

#[test]
fn extract_next_path_handles_bare_path() {
    let n = extract_next_path("https://example.com/foo").unwrap();
    assert_eq!(n, "/foo");
}

#[test]
fn extract_next_path_returns_none_on_invalid() {
    assert!(extract_next_path("not a url").is_none());
}

#[tokio::test]
async fn request_pacer_enforces_min_interval_across_clones() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tick"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok":true})))
        .mount(&server)
        .await;

    let mut client = Client::with_base(TOKEN, server.uri());
    client.initial_delay_ms = 1;
    client.disable_sleep = false;
    client.min_interval = Duration::from_millis(120);

    let client2 = client.clone();

    let start = std::time::Instant::now();
    let _: serde_json::Value = client.request_json("/tick").await.unwrap();
    let _: serde_json::Value = client2.request_json("/tick").await.unwrap();
    let _: serde_json::Value = client.request_json("/tick").await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(220),
        "expected ≥220ms of pacing, got {elapsed:?}",
    );
}

#[test]
fn required_sleep_no_history_returns_zero() {
    let st = PacerState::default();
    let now = Instant::now();
    let d = required_sleep(&st, DEFAULT_FALLBACK_INTERVAL, now);
    assert_eq!(d, Duration::ZERO);
}

#[test]
fn required_sleep_empty_bucket_waits_full_window() {
    let st = PacerState {
        last_request: Some(Instant::now()),
        remaining: Some(0),
        quota: Some(60),
    };
    let d = required_sleep(&st, DEFAULT_FALLBACK_INTERVAL, Instant::now());
    assert_eq!(d, Duration::from_secs(60));
}

#[test]
fn required_sleep_uses_observed_quota_when_available() {
    let now = Instant::now();
    let st = PacerState {
        last_request: Some(now - Duration::from_millis(200)),
        remaining: Some(30),
        quota: Some(60),
    };
    let d = required_sleep(&st, DEFAULT_FALLBACK_INTERVAL, now);
    assert_eq!(d, Duration::from_millis(800));
}

#[test]
fn required_sleep_returns_zero_when_already_past_interval() {
    let now = Instant::now();
    let st = PacerState {
        last_request: Some(now - Duration::from_secs(5)),
        remaining: Some(30),
        quota: Some(60),
    };
    let d = required_sleep(&st, DEFAULT_FALLBACK_INTERVAL, now);
    assert_eq!(d, Duration::ZERO);
}

#[test]
fn required_sleep_falls_back_without_quota() {
    let now = Instant::now();
    let st = PacerState {
        last_request: Some(now - Duration::from_millis(100)),
        remaining: None,
        quota: None,
    };
    let d = required_sleep(&st, Duration::from_millis(500), now);
    assert_eq!(d, Duration::from_millis(400));
}

#[tokio::test]
async fn pacer_adapts_to_observed_quota_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/quota"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-discogs-ratelimit", "60")
                .insert_header("x-discogs-ratelimit-remaining", "30")
                .insert_header("x-discogs-ratelimit-used", "30")
                .set_body_json(json!({"ok": true})),
        )
        .mount(&server)
        .await;

    let mut client = Client::with_base(TOKEN, server.uri());
    client.initial_delay_ms = 1;
    client.disable_sleep = false;
    client.min_interval = Duration::from_millis(10);

    let start = std::time::Instant::now();
    let _: serde_json::Value = client.request_json("/quota").await.unwrap();
    let _: serde_json::Value = client.request_json("/quota").await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(950),
        "second request should have been paced by observed quota, got {elapsed:?}",
    );
}

#[tokio::test]
async fn rate_limited_response_forces_empty_bucket() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rl"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let client = fast_client(server.uri());

    let err = client
        .request_json::<serde_json::Value>("/rl")
        .await
        .unwrap_err();
    assert!(matches!(err, DiscogsError::RateLimited));
}
