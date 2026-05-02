//! Environment configuration loaded at server startup.

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub discogs_username: Option<String>,
    pub discogs_token: Option<String>,
    pub sync_cron_schedule: Option<String>,
    pub bind_addr: String,
}

impl Config {
    pub fn from_env() -> Result<Self, anyhow::Error> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL environment variable is not set"))?;
        Ok(Self {
            database_url,
            discogs_username: std::env::var("DISCOGS_USERNAME").ok(),
            discogs_token: std::env::var("DISCOGS_TOKEN").ok(),
            sync_cron_schedule: std::env::var("SYNC_CRON_SCHEDULE").ok(),
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string()),
        })
    }
}
