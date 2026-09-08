use serde::Deserialize;
use waxdemon_discogs::client::Client;

#[derive(Debug, Deserialize)]
pub struct LegacyIdentity {
    pub id: i64,
    pub username: String,
}

pub async fn verify_owner(
    client: &Client,
    configured_username: &str,
) -> Result<LegacyIdentity, anyhow::Error> {
    if configured_username.trim().is_empty() {
        anyhow::bail!("DISCOGS_USERNAME is required to identify the existing owner");
    }
    let identity: LegacyIdentity = client
        .request_json("/oauth/identity")
        .await
        .map_err(|_| anyhow::anyhow!("could not verify the legacy credential with Discogs"))?;
    if identity.id <= 0 || !identity.username.eq_ignore_ascii_case(configured_username) {
        anyhow::bail!("Discogs credential does not belong to the configured legacy username");
    }
    Ok(identity)
}
