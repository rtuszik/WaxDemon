use secrecy::ExposeSecret;
use waxdemon_discogs::oauth::{OAuthClient, OAuthCredentials, OAuthError};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client(base: &str) -> OAuthClient {
    OAuthClient::with_base(
        OAuthCredentials::new("consumer-key".into(), "consumer-secret".into()).unwrap(),
        base,
    )
    .unwrap()
}

fn auth(request: &wiremock::Request) -> String {
    request
        .headers
        .get("authorization")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

#[tokio::test]
async fn exchanges_tokens_and_checks_identity_with_each_users_credentials() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/oauth/request_token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "oauth_token=request%2Btoken&oauth_token_secret=request-secret&oauth_callback_confirmed=true",
        ))
        .expect(1)
        .mount(&server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/access_token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("oauth_token=access-token&oauth_token_secret=access-secret"),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/oauth/identity"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"id": 123, "username": "owner"})),
        )
        .expect(2)
        .mount(&server)
        .await;
    let client = client(&server.uri());
    let request = client
        .request_token("https://wax.example/auth/callback")
        .await
        .unwrap();
    let authorize = request.authorization_url();
    assert_eq!(authorize.host_str(), Some("www.discogs.com"));
    assert_eq!(
        authorize.query_pairs().collect::<Vec<_>>(),
        vec![("oauth_token".into(), "request+token".into())]
    );
    assert!(!authorize.as_str().contains("secret"));
    let access = client
        .exchange(&request, &"verify+me".into())
        .await
        .unwrap();
    assert_eq!(access.token().expose_secret(), "access-token");
    let identity = client.identity(&access).await.unwrap();
    assert_eq!((identity.id, identity.username.as_str()), (123, "owner"));
    let other = OAuthCredentials::new("other-token".into(), "other-secret".into()).unwrap();
    client.identity(&other).await.unwrap();
    let requests = server.received_requests().await.unwrap();
    let start = auth(&requests[0]);
    assert!(start.contains("oauth_callback=\"https%3A%2F%2Fwax.example%2Fauth%2Fcallback\""));
    assert!(start.contains("oauth_signature_method=\"PLAINTEXT\""));
    assert!(start.contains("oauth_signature=\"consumer-secret&\""));
    assert!(start.contains("oauth_nonce="));
    assert!(start.contains("oauth_timestamp="));
    assert!(!start.contains("oauth_token="));
    let exchange = auth(&requests[1]);
    assert!(exchange.contains("oauth_verifier=\"verify%2Bme\""));
    assert!(exchange.contains("oauth_token=\"request%2Btoken\""));
    assert!(exchange.contains("oauth_signature=\"consumer-secret&request-secret\""));
    assert!(auth(&requests[2]).contains("consumer-secret&access-secret"));
    assert!(auth(&requests[3]).contains("consumer-secret&other-secret"));
    for request in &requests {
        assert!(request.url.query().is_none());
        assert!(request.body.is_empty());
        assert!(request.headers.contains_key("user-agent"));
    }
}

#[tokio::test]
async fn rejects_unconfirmed_empty_duplicate_and_malformed_token_responses() {
    for body in [
        "oauth_token=t&oauth_token_secret=s",
        "oauth_token=t&oauth_token_secret=s&oauth_callback_confirmed=false",
        "oauth_token=&oauth_token_secret=s&oauth_callback_confirmed=true",
        "oauth_token=t&oauth_token=t2&oauth_token_secret=s&oauth_callback_confirmed=true",
        "oauth_token=t&oauth_token_secret=s&oauth_callback_confirmed=true&oauth_callback_confirmed=true",
        "not a token response",
    ] {
        let server = MockServer::start().await;
        Mock::given(path("/oauth/request_token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server.uri())
                .request_token("https://wax.example/callback")
                .await,
            Err(OAuthError::InvalidResponse)
        ));
    }
}

#[tokio::test]
async fn does_not_follow_redirects_retry_exchanges_or_expose_provider_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/oauth/request_token"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/leak"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/leak"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server.uri())
            .request_token("https://wax.example/callback")
            .await,
        Err(OAuthError::Http(302))
    ));
    server.reset().await;
    Mock::given(path("/oauth/request_token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                "oauth_token=t&oauth_token_secret=s&oauth_callback_confirmed=true",
            ),
        )
        .mount(&server)
        .await;
    Mock::given(path("/oauth/access_token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("DO-NOT-LEAK-provider-secret"))
        .expect(1)
        .mount(&server)
        .await;
    let client = client(&server.uri());
    let token = client
        .request_token("https://wax.example/callback")
        .await
        .unwrap();
    let error = client
        .exchange(&token, &"verifier".into())
        .await
        .unwrap_err();
    assert!(matches!(error, OAuthError::Http(401)));
    assert!(!format!("{error} {error:?}").contains("DO-NOT-LEAK"));
}

#[tokio::test]
async fn rejects_invalid_identities_and_oversized_responses() {
    let server = MockServer::start().await;
    let client = client(&server.uri());
    let token = OAuthCredentials::new("t".into(), "s".into()).unwrap();
    for body in [
        r#"{"id":0,"username":"owner"}"#,
        r#"{"id":1,"username":" "}"#,
        r#"{"username":"owner"}"#,
    ] {
        server.reset().await;
        Mock::given(path("/oauth/identity"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
        assert!(matches!(
            client.identity(&token).await,
            Err(OAuthError::InvalidResponse)
        ));
    }
    server.reset().await;
    Mock::given(path("/oauth/identity"))
        .respond_with(ResponseTemplate::new(200).set_body_string("x".repeat(20_000)))
        .mount(&server)
        .await;
    assert!(matches!(
        client.identity(&token).await,
        Err(OAuthError::InvalidResponse)
    ));
}

#[tokio::test]
async fn rejects_unsafe_urls_before_sending_credentials() {
    let server = MockServer::start().await;
    let client = client(&server.uri());
    for callback in [
        "http://wax.example/callback",
        "https://user:password@wax.example/callback",
        "https://wax.example/#fragment",
        "not a url",
    ] {
        assert!(matches!(
            client.request_token(callback).await,
            Err(OAuthError::InvalidConfiguration)
        ));
    }
    assert!(server.received_requests().await.unwrap().is_empty());
    assert!(
        OAuthClient::with_base(
            OAuthCredentials::new("a".into(), "b".into()).unwrap(),
            "http://discogs.example"
        )
        .is_err()
    );
}

#[test]
fn credentials_are_validated_and_redacted() {
    assert!(OAuthCredentials::new("".into(), "secret".into()).is_err());
    assert!(OAuthCredentials::new("token".into(), "".into()).is_err());
    let token = OAuthCredentials::new("private-token".into(), "private-secret".into()).unwrap();
    let debug = format!("{token:?}");
    assert!(!debug.contains("private-token"));
    assert!(!debug.contains("private-secret"));
}
