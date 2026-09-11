use anyhow::{Context, Result, bail};
use chacha20poly1305::aead::Generate;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};
use waxdemon_discogs::oauth::{Identity, OAuthClient, OAuthCredentials, RequestToken};
use waxdemon_server::credential_vault::CredentialVault;

const REQUEST_TOKEN_LIFETIME: Duration = Duration::from_secs(15 * 60);
const HELP: &str = "Usage: waxdemon-oauth-smoke [--help]

Interactive Discogs OAuth test for an application without a callback URL.
Prompts privately for the consumer key, consumer secret, and verifier.
DISCOGS_CONSUMER_KEY and DISCOGS_CONSUMER_SECRET may supply the app credentials.
Does not load .env files, access databases, or save access tokens.
Authorization remains on Discogs until you revoke it in your account settings.";

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{HELP}");
        return Ok(());
    }
    if !args.is_empty() {
        bail!("{HELP}");
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!(
            "Run this command in an interactive terminal; input/output redirection is not supported."
        );
    }
    println!("Discogs OAuth smoke test. No application users, sessions, or database writes.");
    let consumer = OAuthCredentials::new(
        credential("DISCOGS_CONSUMER_KEY", "Consumer key (hidden): ")?,
        credential("DISCOGS_CONSUMER_SECRET", "Consumer secret (hidden): ")?,
    )?;
    let client = OAuthClient::new(consumer)?;
    let started = Instant::now();
    let request = client
        .request_token_out_of_band()
        .await
        .context("Request-token step failed")?;
    println!(
        "Open this URL in your browser and authorize the intended Discogs account:\n{}",
        request.authorization_url()
    );
    println!("Enter the displayed verifier here within 15 minutes. Do not paste it into chat.");
    std::io::stdout().flush()?;
    let verifier = prompt("Verifier (hidden): ")?;
    let identity = verify(&client, &request, &verifier, started.elapsed()).await?;
    println!(
        "PASS: authenticated as {} (Discogs ID {}).",
        serde_json::to_string(&identity.username)?,
        identity.id
    );
    println!(
        "PASS: encrypted/decrypted access credentials in memory and verified the same identity again."
    );
    println!(
        "Nothing was saved. Revoke WaxDemon's authorization in Discogs if this was only a temporary test."
    );
    Ok(())
}

fn credential(name: &str, label: &str) -> Result<SecretString> {
    match std::env::var(name) {
        Ok(value) => nonempty(value.into()),
        Err(std::env::VarError::NotPresent) => prompt(label),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{name} must contain valid Unicode"),
    }
}

fn prompt(label: &str) -> Result<SecretString> {
    let value =
        rpassword::prompt_password(label).context("Could not read hidden terminal input")?;
    nonempty(value.into())
}

fn nonempty(value: SecretString) -> Result<SecretString> {
    if value.expose_secret().trim().is_empty() {
        bail!("Input must not be empty; restart the smoke test.");
    }
    Ok(value)
}

async fn verify(
    client: &OAuthClient,
    request: &RequestToken,
    verifier: &SecretString,
    elapsed: Duration,
) -> Result<Identity> {
    if elapsed >= REQUEST_TOKEN_LIFETIME {
        bail!("Request token expired; restart the smoke test.");
    }
    let access = client.exchange(request, verifier).await.context(
        "Access-token exchange failed; restart the smoke test rather than reusing the verifier",
    )?;
    let first = client
        .identity(&access)
        .await
        .context("Initial identity lookup failed")?;
    let key =
        SecretBox::new(Box::new(<[u8; 32]>::try_generate().map_err(|_| {
            anyhow::anyhow!("Could not generate a temporary encryption key")
        })?));
    let vault = CredentialVault::new(
        "smoke-test".into(),
        BTreeMap::from([("smoke-test".into(), key)]),
    )?;
    let encrypted = vault.encrypt(1, &access)?;
    drop(access);
    let restored = vault.decrypt(1, &encrypted)?;
    let second = client
        .identity(&restored)
        .await
        .context("Identity lookup after encryption round-trip failed")?;
    if first.id != second.id {
        bail!("Identity changed after the credential round-trip.");
    }
    Ok(second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    async fn fixture() -> (MockServer, OAuthClient, RequestToken) {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/oauth/request_token"))
            .respond_with(ResponseTemplate::new(200).set_body_string("oauth_token=request&oauth_token_secret=request-secret&oauth_callback_confirmed=true"))
            .mount(&server).await;
        let client = OAuthClient::with_base(
            OAuthCredentials::new("consumer".into(), "consumer-secret".into()).unwrap(),
            &server.uri(),
        )
        .unwrap();
        let request = client.request_token_out_of_band().await.unwrap();
        (server, client, request)
    }

    #[tokio::test]
    async fn verifies_live_flow_shape_and_restored_credentials_without_database() {
        let (server, client, request) = fixture().await;
        Mock::given(method("POST"))
            .and(path("/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("oauth_token=access&oauth_token_secret=access-secret"),
            )
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/oauth/identity"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 42, "username": "owner"})),
            )
            .expect(2)
            .mount(&server)
            .await;
        let identity = verify(&client, &request, &"verifier".into(), Duration::ZERO)
            .await
            .unwrap();
        assert_eq!(identity.id, 42);
        for request in server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.url.path() == "/oauth/identity")
        {
            let auth = request
                .headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap();
            assert!(auth.contains("oauth_token=\"access\""));
            assert!(auth.contains("consumer-secret&access-secret"));
        }
    }

    #[tokio::test]
    async fn rejects_expired_attempt_before_exchanging() {
        let (server, client, request) = fixture().await;
        Mock::given(path("/oauth/access_token"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;
        let error = verify(
            &client,
            &request,
            &"verifier".into(),
            REQUEST_TOKEN_LIFETIME,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("expired"));
    }

    #[tokio::test]
    async fn fails_when_the_identity_changes_after_round_trip() {
        let (server, client, request) = fixture().await;
        Mock::given(path("/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("oauth_token=access&oauth_token_secret=access-secret"),
            )
            .mount(&server)
            .await;
        Mock::given(path("/oauth/identity"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 42, "username": "owner"})),
            )
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(path("/oauth/identity"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"id": 43, "username": "other"})),
            )
            .with_priority(2)
            .mount(&server)
            .await;
        let error = verify(&client, &request, &"verifier".into(), Duration::ZERO)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Identity changed"));
    }

    #[test]
    fn empty_input_errors_do_not_disclose_values() {
        let error = nonempty(" \n".into()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Input must not be empty; restart the smoke test."
        );
        assert_eq!(nonempty("value".into()).unwrap().expose_secret(), "value");
    }
}
