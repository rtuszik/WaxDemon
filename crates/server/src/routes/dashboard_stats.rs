use crate::{error::AppError, state::AppState};
use axum::{
    extract::{Query, State},
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use waxdemon_core::{
    build_dashboard_stats, time_range_filter, DashboardStats, DbItem, HistoryRow, TimeRange,
};

#[derive(Debug, Deserialize, Default)]
pub struct Params {
    #[serde(rename = "timeRange")]
    pub time_range: Option<String>,
}

pub async fn handler(
    State(st): State<AppState>,
    Query(params): Query<Params>,
) -> Result<Json<DashboardStats>, AppError> {
    let range = params
        .time_range
        .as_deref()
        .map(TimeRange::parse)
        .unwrap_or(TimeRange::ThreeMonths);

    let all_items = waxdemon_db::items::select_all(&st.db).await?;
    let latest_stats = waxdemon_db::stats_history::latest_snapshot(&st.db).await?;
    let start = time_range_filter(range, Utc::now());
    let history = waxdemon_db::stats_history::range_query(&st.db, start.as_deref()).await?;

    let db_items: Vec<DbItem> = all_items
        .into_iter()
        .map(|r| DbItem {
            id: r.id as i64,
            release_id: r.release_id as i64,
            artist: r.artist,
            title: r.title,
            year: r.year,
            format: r.format,
            genres: r.genres,
            cover_image_url: r.cover_image_url,
            condition: r.condition,
            suggested_value: r.suggested_value,
            added_date: r.added_date,
        })
        .collect();

    let latest_row = latest_stats.map(|s| HistoryRow {
        timestamp: s.timestamp,
        total_items: s.total_items as i64,
        value_min: s.value_min,
        value_mean: s.value_mean,
        value_max: s.value_max,
    });

    let history_rows: Vec<HistoryRow> = history
        .into_iter()
        .map(|s| HistoryRow {
            timestamp: s.timestamp,
            total_items: s.total_items as i64,
            value_min: s.value_min,
            value_mean: s.value_mean,
            value_max: s.value_max,
        })
        .collect();

    let stats = build_dashboard_stats(&db_items, latest_row.as_ref(), &history_rows);
    Ok(Json(stats))
}
