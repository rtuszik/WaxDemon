use waxdemon_db::Db;
use waxdemon_discogs::client::Client;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub http: Arc<reqwest::Client>,
    pub discogs: Arc<Client>,
    pub discogs_username: Option<String>,
    /// Allow-list prefix for the image proxy. Defaults to `https://i.discogs.com/`.
    /// Overridable so integration tests can point the proxy at a local wiremock.
    pub image_proxy_prefix: String,
}

impl AppState {
    pub fn new(db: Db, discogs: Client, discogs_username: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(IMAGE_USER_AGENT)
            .build()
            .expect("reqwest client");
        Self {
            db,
            http: Arc::new(http),
            discogs: Arc::new(discogs),
            discogs_username,
            image_proxy_prefix: "https://i.discogs.com/".into(),
        }
    }
}

pub const IMAGE_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
