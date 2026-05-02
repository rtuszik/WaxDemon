pub mod dashboard;

use crate::state::AppState;
use axum::response::{Html, IntoResponse};
use axum::{extract::State, routing::get, Router};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .with_state(state)
}

pub async fn index_handler(State(st): State<AppState>) -> impl IntoResponse {
    Html(dashboard::render(&st).await)
}
