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
    for _ in 0..10 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/callback")
                    .extension(axum::extract::ConnectInfo(
                        "192.0.2.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                    ))
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
                .extension(axum::extract::ConnectInfo(
                    "192.0.2.1:12345".parse::<std::net::SocketAddr>().unwrap(),
                ))
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
    for peer in [
        "192.0.2.1:54321",
        "[::ffff:192.0.2.1]:12345",
        "192.0.2.2:12345",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/callback")
                    .extension(axum::extract::ConnectInfo(
                        peer.parse::<std::net::SocketAddr>().unwrap(),
                    ))
                    .header("forwarded", "for=198.51.100.1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let expected = if peer.starts_with("192.0.2.2:") {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::TOO_MANY_REQUESTS
        };
        assert_eq!(response.status(), expected, "{peer}");
    }
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

#[tokio::test]
async fn oauth_requires_transport_identity_and_real_server_supplies_it() {
    let app = state("https://wax.example").unwrap().router();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/callback")
                .header("x-forwarded-for", "192.0.2.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let client = reqwest::Client::new();
    for index in 0..11 {
        let response = client
            .get(format!("http://{address}/auth/callback"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            if index < 10 { 400 } else { 429 }
        );
    }
    server.abort();
    let _ = server.await;
}

#[tokio::test]
async fn trusted_proxy_separates_clients_and_ignores_spoofed_prefixes() {
    let app = state("https://wax.example")
        .unwrap()
        .with_trusted_proxies("192.0.2.0/24")
        .unwrap()
        .router();
    for index in 0..40 {
        let request = Request::builder()
            .uri("/auth/callback")
            .extension(axum::extract::ConnectInfo(
                "192.0.2.100:12345".parse::<std::net::SocketAddr>().unwrap(),
            ))
            .header(
                "x-forwarded-for",
                format!("198.51.100.{index}, 203.0.113.1"),
            )
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            if index < 10 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            }
        );
    }
    let request = Request::builder()
        .uri("/auth/callback")
        .extension(axum::extract::ConnectInfo(
            "192.0.2.100:12345".parse::<std::net::SocketAddr>().unwrap(),
        ))
        .header("x-forwarded-for", "203.0.113.2")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    for header in [None, Some("invalid"), Some("192.0.2.100")] {
        let mut request =
            Request::builder()
                .uri("/auth/callback")
                .extension(axum::extract::ConnectInfo(
                    "192.0.2.100:12345".parse::<std::net::SocketAddr>().unwrap(),
                ));
        if let Some(value) = header {
            request = request.header("x-forwarded-for", value);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

#[tokio::test]
async fn cidr_proxy_config_validates_networks_and_rejects_unusable_chains() {
    for proxies in [
        "not-a-network",
        "192.0.2.0/33",
        "2001:db8::/129",
        "192.0.2.0/24,",
    ] {
        assert!(
            state("https://wax.example")
                .unwrap()
                .with_trusted_proxies(proxies)
                .is_err(),
            "{proxies}"
        );
    }
    let app = state("https://wax.example")
        .unwrap()
        .with_trusted_proxies("192.0.2.0/24,2001:db8::/64")
        .unwrap()
        .router();
    for (peer, xff, expected) in [
        (
            "192.0.2.201:1234",
            Some("198.51.100.1, 203.0.113.1, 192.0.2.202"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "[::ffff:192.0.2.202]:1234",
            Some("203.0.113.2"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "[2001:db8::1]:1234",
            Some("[2001:db8:1::1]:4321"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "192.0.2.1:1234",
            Some("203.0.113.1, unknown"),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        ("192.0.2.1:1234", None, StatusCode::SERVICE_UNAVAILABLE),
    ] {
        let mut request = Request::builder()
            .uri("/auth/callback")
            .extension(axum::extract::ConnectInfo(
                peer.parse::<std::net::SocketAddr>().unwrap(),
            ))
            .header("forwarded", "for=198.51.100.99;proto=\", for=203.0.113.10");
        if let Some(xff) = xff {
            request = request.header("x-forwarded-for", xff);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            expected,
            "{peer} {xff:?}"
        );
    }
    let request = Request::builder()
        .uri("/auth/callback")
        .extension(axum::extract::ConnectInfo(
            "192.0.2.1:1234".parse::<std::net::SocketAddr>().unwrap(),
        ))
        .header("x-forwarded-for", "198.51.100.99")
        .header("x-forwarded-for", "203.0.113.3")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(request).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
}
