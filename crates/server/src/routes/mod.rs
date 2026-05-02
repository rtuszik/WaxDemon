pub mod assets;
pub mod dashboard_stats;
pub mod image_proxy;
pub mod sync;
pub mod sync_status;

use crate::state::AppState;
use crate::views;
use axum::{routing::get, Router};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(views::index_handler))
        .route("/favicon.ico", get(assets::favicon))
        .route("/assets/{name}", get(assets::handler))
        .route("/api/dashboard-stats", get(dashboard_stats::handler))
        .route("/api/collection/sync", get(sync::handler))
        .route("/api/collection/sync/status", get(sync_status::handler))
        .route("/api/image-proxy", get(image_proxy::handler))
        .with_state(state)
}
