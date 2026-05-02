use crate::{error::AppError, state::AppState};
use axum::{extract::State, Json};
use serde::Serialize;
use waxdemon_sync::run::{run_collection_sync, SyncConfig};

#[derive(Serialize)]
pub struct Response {
    pub message: String,
}

pub async fn handler(State(st): State<AppState>) -> Result<Json<Response>, AppError> {
    let username = st
        .discogs_username
        .clone()
        .ok_or_else(|| AppError::Internal("DISCOGS_USERNAME not configured".into()))?;
    let token = std::env::var("DISCOGS_TOKEN")
        .map_err(|_| AppError::Internal("DISCOGS_TOKEN not set".into()))?;
    let cfg = SyncConfig { username, token };

    let result = run_collection_sync(&st.db, &st.discogs, &cfg)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(Json(Response {
        message: result.message,
    }))
}
