#[derive(thiserror::Error, Debug)]
pub enum DiscogsError {
    #[error("discogs request failed after retries: {0}")]
    Retry(String),

    #[error("rate limited (HTTP 429)")]
    RateLimited,

    #[error("http error {status}: {body}")]
    Http { status: u16, body: String },

    #[error(transparent)]
    Transport(#[from] reqwest::Error),

    #[error(transparent)]
    Parse(#[from] serde_json::Error),
}

impl DiscogsError {
    pub fn is_not_found(&self) -> bool {
        matches!(self, DiscogsError::Http { status: 404, .. })
    }
}
