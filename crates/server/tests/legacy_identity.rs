use serde_json::json;
use waxdemon_discogs::client::Client;
use waxdemon_server::legacy_migration::verify_owner;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn verifies_legacy_token_owner_and_uses_numeric_identity() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/oauth/identity"))
        .and(header("Authorization", "Discogs token=test-legacy-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id":123,"username":"Owner"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let identity = verify_owner(
        &Client::with_base("test-legacy-token", server.uri()),
        "owner",
    )
    .await
    .unwrap();
    assert_eq!(identity.id, 123);
    assert_eq!(identity.username, "Owner");
}

#[tokio::test]
async fn rejects_wrong_account_and_invalid_identity() {
    for response in [
        json!({"id":123,"username":"other"}),
        json!({"id":0,"username":"owner"}),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/oauth/identity"))
            .respond_with(ResponseTemplate::new(200).set_body_json(response))
            .mount(&server)
            .await;
        assert!(
            verify_owner(&Client::with_base("token", server.uri()), "owner")
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn verification_failure_does_not_expose_provider_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/oauth/identity"))
        .respond_with(ResponseTemplate::new(401).set_body_string("sensitive-provider-body"))
        .mount(&server)
        .await;
    let error = verify_owner(&Client::with_base("token", server.uri()), "owner")
        .await
        .unwrap_err();
    assert!(!format!("{error:?}").contains("sensitive-provider-body"));
}
