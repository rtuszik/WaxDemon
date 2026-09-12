mod api;
pub mod backend;
pub mod config;
mod health;
mod routes;
pub mod store;
mod ui;

use crate::credential_vault::CredentialVault;
use axum::{
    Router,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use sqlx::PgPool;
use std::sync::Arc;
use url::Url;
use waxdemon_discogs::oauth::OAuthClient;

pub use routes::router;
pub type AuthSession = axum_login::AuthSession<AuthState>;
pub const SESSION_DAYS: i64 = 90;

#[derive(Clone)]
pub struct AuthState {
    pub(crate) pool: PgPool,
    pub(crate) oauth: OAuthClient,
    pub(crate) vault: Arc<CredentialVault>,
    pub(crate) legacy_owner: Option<String>,
    pub(crate) leptos: leptos::prelude::LeptosOptions,
    pub(super) origin: String,
    pub(super) callback: String,
    pub(super) secure: bool,
}

impl AuthState {
    pub fn new(
        pool: PgPool,
        oauth: OAuthClient,
        vault: Arc<CredentialVault>,
        public_url: &str,
    ) -> Result<Self, AuthError> {
        let url = Url::parse(public_url).map_err(|_| AuthError::Configuration)?;
        let local = match url.host() {
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
        if url.host().is_none()
            || !(url.scheme() == "https" || (url.scheme() == "http" && local))
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(AuthError::Configuration);
        }
        Ok(Self {
            pool,
            oauth,
            vault,
            legacy_owner: None,
            leptos: leptos::prelude::LeptosOptions::builder()
                .output_name("waxdemon")
                .site_root(
                    std::env::var("LEPTOS_SITE_ROOT").unwrap_or_else(|_| "target/site".into()),
                )
                .build(),
            origin: url.origin().ascii_serialization(),
            callback: url
                .join("/auth/callback")
                .map_err(|_| AuthError::Configuration)?
                .to_string(),
            secure: url.scheme() == "https",
        })
    }

    pub async fn with_legacy_owner(mut self, username: Option<String>) -> anyhow::Result<Self> {
        let (imported, admin, legacy_data): (bool, bool, bool) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM legacy_import),
                    EXISTS(SELECT 1 FROM users WHERE role = 'admin' AND status = 'approved'),
                    EXISTS(SELECT 1 FROM collection_items UNION ALL
                           SELECT 1 FROM collection_stats_history UNION ALL SELECT 1 FROM settings)",
        )
        .fetch_one(&self.pool)
        .await?;
        if imported || admin {
            anyhow::ensure!(
                admin,
                "an approved administrator is required; completed imports will not be repeated"
            );
            anyhow::ensure!(
                imported || !legacy_data,
                "unimported legacy data exists alongside an administrator; refusing to reassign it"
            );
        } else {
            self.legacy_owner = Some(username.filter(|name| !name.trim().is_empty())
                .ok_or_else(|| anyhow::anyhow!("DISCOGS_USERNAME is required until the owner completes their first OAuth login"))?);
        }
        Ok(self)
    }

    pub fn router(self) -> Router {
        router(self)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid request parameters")]
    InvalidInput,
    #[error("not found")]
    NotFound,
    #[error("invalid authentication configuration")]
    Configuration,
    #[error("authentication service failed")]
    Internal,
    #[error("Discogs authentication failed; start a new login")]
    Provider,
    #[error("invalid or expired login attempt; start a new login")]
    Attempt,
    #[error("authentication required")]
    Unauthorized,
    #[error("access denied")]
    Forbidden,
    #[error("request conflicts with the current account state")]
    Conflict,
    #[error("the last approved administrator cannot be deleted")]
    LastAdmin,
}

impl From<sqlx::Error> for AuthError {
    fn from(_: sqlx::Error) -> Self {
        Self::Internal
    }
}
impl From<tower_sessions::session::Error> for AuthError {
    fn from(_: tower_sessions::session::Error) -> Self {
        Self::Internal
    }
}
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::InvalidInput => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Configuration | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Provider => StatusCode::BAD_GATEWAY,
            Self::Attempt => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Conflict | Self::LastAdmin => StatusCode::CONFLICT,
        };
        (status, self.to_string()).into_response()
    }
}
