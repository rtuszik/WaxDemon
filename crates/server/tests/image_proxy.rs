//! Integration tests for the image proxy route.

use axum_test::TestServer;
use waxdemon_discogs::client::Client;
use waxdemon_server::{router, AppState};
use sqlx::postgres::PgPoolOptions;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn state_with_upstream(upstream_prefix: String) -> Option<AppState> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .ok()?;
    let mut st = AppState::new(pool, Client::new("token".to_string()), Some("user".into()));
    st.image_proxy_prefix = upstream_prefix;
    Some(st)
}

#[tokio::test]
async fn missing_url_returns_400() {
    let Some(st) = state_with_upstream("https://example.test/".into()).await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/api/image-proxy").await;
    resp.assert_status(http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["message"], "Missing image URL parameter");
}

#[tokio::test]
async fn invalid_url_returns_400() {
    let Some(st) = state_with_upstream("https://i.discogs.com/".into()).await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server
        .get("/api/image-proxy?url=https://evil.example/x.jpg")
        .await;
    resp.assert_status(http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["message"], "Invalid image URL provided");
}

#[tokio::test]
async fn local_path_returns_400_with_specific_message() {
    let Some(st) = state_with_upstream("https://i.discogs.com/".into()).await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/api/image-proxy?url=/local.jpg").await;
    resp.assert_status(http::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["message"], "Proxying local files is not supported");
}

#[tokio::test]
async fn happy_path_streams_bytes_with_cache_header() {
    if std::env::var("TEST_DATABASE_URL").is_err() {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    }
    let upstream = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/img/a.jpg"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(b"PNGDATA".to_vec())
                .insert_header("content-type", "image/png"),
        )
        .expect(1)
        .mount(&upstream)
        .await;

    // Prefix the proxy on the mock server
    let prefix = format!("{}/", upstream.uri());
    let Some(st) = state_with_upstream(prefix.clone()).await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let target = format!("{}img/a.jpg", prefix);
    let resp = server
        .get(&format!(
            "/api/image-proxy?url={}",
            urlencoding::encode(&target)
        ))
        .await;
    resp.assert_status_ok();
    assert_eq!(resp.header("content-type"), "image/png");
    assert_eq!(
        resp.header("cache-control"),
        "public, max-age=604800, immutable"
    );
    assert_eq!(resp.as_bytes().as_ref(), b"PNGDATA");
}
