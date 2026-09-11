use super::{AuthError, AuthState};
use crate::credential_vault::CredentialVault;
use secrecy::{ExposeSecret, SecretBox, SecretString};
use std::{collections::BTreeMap, sync::Arc};
use waxdemon_discogs::oauth::{OAuthClient, OAuthCredentials};

fn required(name: &str) -> anyhow::Result<String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

pub async fn from_env(pool: sqlx::PgPool) -> anyhow::Result<AuthState> {
    let public_url = required("PUBLIC_URL")?;
    let consumer = OAuthCredentials::new(
        required("DISCOGS_CONSUMER_KEY")?.into(),
        required("DISCOGS_CONSUMER_SECRET")?.into(),
    )?;
    let key_file = required("OAUTH_KEYRING_FILE")?;
    let content: SecretString = std::fs::read_to_string(key_file)
        .map_err(|_| anyhow::anyhow!("could not read OAUTH_KEYRING_FILE"))?
        .into();
    let encoded: BTreeMap<String, SecretString> = serde_json::from_str(content.expose_secret())
        .map_err(|_| anyhow::anyhow!("OAUTH_KEYRING_FILE must contain a JSON object mapping key IDs to 64-character hex keys"))?;
    let mut keys = BTreeMap::new();
    for (id, value) in encoded {
        let mut key = SecretBox::new(Box::new([0u8; 32]));
        use secrecy::ExposeSecretMut;
        hex::decode_to_slice(value.expose_secret(), key.expose_secret_mut())
            .map_err(|_| anyhow::anyhow!("each keyring value must be a 32-byte hex key"))?;
        keys.insert(id, key);
    }
    let vault = CredentialVault::new(required("OAUTH_ACTIVE_KEY_ID")?, keys)?;
    let state = AuthState::new(
        pool,
        OAuthClient::new(consumer)?,
        Arc::new(vault),
        &public_url,
    )?;
    let admin: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE role = 'admin' AND status = 'approved')",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AuthError::Internal)?;
    anyhow::ensure!(
        admin,
        "a verified, migrated approved admin is required; run the explicit legacy import first"
    );
    Ok(state)
}
