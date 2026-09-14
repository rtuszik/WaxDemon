use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use secrecy::SecretBox;
use sqlx::postgres::PgPoolOptions;
use std::{collections::BTreeMap, sync::Arc};
use tower::ServiceExt;
use waxdemon_discogs::oauth::{OAuthClient, OAuthCredentials};
use waxdemon_server::{auth::AuthState, credential_vault::CredentialVault};

fn state(origin: &str) -> Result<AuthState, waxdemon_server::auth::AuthError> {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://localhost/unused")
        .unwrap();
    let oauth =
        OAuthClient::new(OAuthCredentials::new("test".into(), "test".into()).unwrap()).unwrap();
    let vault = CredentialVault::new(
        "test".into(),
        BTreeMap::from([("test".into(), SecretBox::new(Box::new([1; 32])))]),
    )
    .unwrap();
    AuthState::new(pool, oauth, Arc::new(vault), origin)
}

#[tokio::test]
async fn public_origin_rejects_insecure_remote_and_ambiguous_urls() {
    for origin in [
        "http://wax.example",
        "http://0.0.0.0:3000",
        "http://192.168.1.2",
        "https://user:password@wax.example",
        "https://wax.example/path",
        "https://wax.example?x=1",
        "https://wax.example#fragment",
    ] {
        assert!(state(origin).is_err(), "{origin}");
    }
    for origin in [
        "https://wax.example",
        "http://127.0.0.1:3000",
        "http://[::1]:3000",
    ] {
        assert!(state(origin).is_ok(), "{origin}");
    }
}

#[tokio::test]
async fn session_days_reject_zero_negative_and_excessive_lifetimes() {
    for days in [0, -1, 366, i64::MAX] {
        assert!(
            state("https://wax.example")
                .unwrap()
                .with_session_days(days)
                .is_err()
        );
    }
    for days in [1, 30, 90, 365] {
        assert!(
            state("https://wax.example")
                .unwrap()
                .with_session_days(days)
                .is_ok()
        );
    }
}

#[tokio::test]
async fn redirects_errors_and_preflights_keep_security_headers_without_cors() {
    let app = state("https://wax.example").unwrap().router();
    for (method, path, status) in [
        ("GET", "/", StatusCode::SEE_OTHER),
        ("GET", "/missing", StatusCode::NOT_FOUND),
        ("GET", "/api/dashboard", StatusCode::UNAUTHORIZED),
        ("OPTIONS", "/api/dashboard", StatusCode::METHOD_NOT_ALLOWED),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("origin", "https://evil.example")
                    .header("access-control-request-method", "GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{method} {path}");
        let headers = response.headers();
        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers["cache-control"], "no-store");
        assert_eq!(
            headers["permissions-policy"],
            "camera=(), microphone=(), geolocation=()"
        );
        assert!(
            headers["content-security-policy"]
                .to_str()
                .unwrap()
                .contains("frame-ancestors 'none'")
        );
        assert!(!headers.contains_key("access-control-allow-origin"));
        assert!(!headers.contains_key("access-control-allow-credentials"));
    }
}

#[tokio::test]
async fn throttled_requests_keep_headers_and_do_not_block_health_checks() {
    let app = state("https://wax.example").unwrap().router();
    for _ in 0..30 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/callback")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback")
                .header("x-forwarded-for", "203.0.113.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().contains_key("retry-after"));
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert!(response.headers().contains_key("permissions-policy"));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
