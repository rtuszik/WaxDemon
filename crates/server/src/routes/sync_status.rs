use crate::{error::AppError, state::AppState};
use axum::{extract::State, Json};
use waxdemon_core::{SyncStatus, SyncStatusResponse};

pub async fn handler(State(st): State<AppState>) -> Result<Json<SyncStatusResponse>, AppError> {
    let status_raw = waxdemon_db::get_setting(&st.db, "sync_status").await?;
    let current_raw = waxdemon_db::get_setting(&st.db, "sync_current_item").await?;
    let total_raw = waxdemon_db::get_setting(&st.db, "sync_total_items").await?;
    let last_err = waxdemon_db::get_setting(&st.db, "sync_last_error").await?;

    let status = SyncStatus::parse(status_raw.as_deref());
    let current_item = current_raw
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let total_items = total_raw
        .as_deref()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let last_error = match last_err {
        Some(s) if s.is_empty() => None,
        other => other,
    };

    Ok(Json(SyncStatusResponse {
        status,
        current_item,
        total_items,
        last_error,
    }))
}
