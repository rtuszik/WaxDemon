mod api;
pub mod backend;
pub mod config;
mod health;
mod limits;
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
    pub(crate) bootstrap_first_user: bool,
    pub(crate) leptos: leptos::prelude::LeptosOptions,
    pub(super) origin: String,
    pub(super) callback: String,
    pub(super) secure: bool,
    pub(super) session_days: i64,
    pub(super) trusted_proxies: axum_client_addr::ClientIpConfig,
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
            bootstrap_first_user: false,
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
            session_days: SESSION_DAYS,
            trusted_proxies: axum_client_addr::ClientIpConfig::default(),
        })
    }

    pub fn with_trusted_proxies(mut self, proxies: &str) -> Result<Self, AuthError> {
        use axum_client_addr::{ChainHeader, ClientIpConfig, IpCidr};
        let mut config = ClientIpConfig::builder()
            .trusted_proxies()
            .chain_header_order([ChainHeader::x_forwarded_for()]);
        if !proxies.trim().is_empty() {
            for proxy in proxies.split(',') {
                let cidr = proxy
                    .trim()
                    .parse::<IpCidr>()
                    .map_err(|_| AuthError::Configuration)?;
                config = config.proxy(cidr);
            }
        }
        self.trusted_proxies = config.build().map_err(|_| AuthError::Configuration)?;
        Ok(self)
    }

    pub fn with_session_days(mut self, days: i64) -> Result<Self, AuthError> {
        if !(1..=365).contains(&days) {
            return Err(AuthError::Configuration);
        }
        self.session_days = days;
        Ok(self)
    }

    pub async fn with_legacy_owner(mut self, username: Option<String>) -> anyhow::Result<Self> {
        let (imported, admin): (bool, bool) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM legacy_import),
                    EXISTS(SELECT 1 FROM users WHERE role = 'admin' AND status = 'approved')",
        )
        .fetch_one(&self.pool)
        .await?;
        if imported || admin {
            anyhow::ensure!(
                admin,
                "an approved administrator is required; completed imports will not be repeated"
            );
            anyhow::ensure!(
                imported || !waxdemon_db::legacy_import::has_legacy_data(&self.pool).await?,
                "unimported legacy data exists alongside an administrator; refusing to reassign it"
            );
        } else {
            self.legacy_owner = username.filter(|name| !name.trim().is_empty());
            if self.legacy_owner.is_none() {
                anyhow::ensure!(
                    !waxdemon_db::legacy_import::has_legacy_data(&self.pool).await?,
                    "DISCOGS_USERNAME is required until the legacy owner completes their first OAuth login"
                );
                let users_exist: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users)")
                    .fetch_one(&self.pool)
                    .await?;
                anyhow::ensure!(
                    !users_exist,
                    "an approved administrator is required for an existing installation"
                );
                self.bootstrap_first_user = true;
            }
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
