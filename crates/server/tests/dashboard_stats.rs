//! Gated on TEST_DATABASE_URL.

use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;
use waxdemon_db::{
    items::{self, UpsertItem},
    run_migrations,
    stats_history::{self, StatsSnapshot},
};
use waxdemon_discogs::client::Client;
use waxdemon_server::{router, AppState};

async fn fresh_state() -> Option<AppState> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .ok()?;
    // Reset schema to ensure test isolation.
    sqlx::query(
        "DROP TABLE IF EXISTS collection_items, collection_stats_history, settings, _sqlx_migrations",
    )
    .execute(&pool)
    .await
    .ok()?;
    run_migrations(&pool).await.ok()?;

    Some(AppState::new(
        pool,
        Client::new("token".to_string()),
        Some("user".into()),
    ))
}

fn item(
    id: i32,
    release_id: i32,
    year: i32,
    genres: &str,
    format: &str,
    value: f64,
    added: &str,
) -> UpsertItem {
    UpsertItem {
        id,
        release_id,
        artist: Some("A".into()),
        title: Some("T".into()),
        year: Some(year),
        format: Some(format.into()),
        genres_json: genres.into(),
        styles_json: "[]".into(),
        cover_image_url: None,
        added_date: added.into(),
        folder_id: Some(0),
        rating: Some(0),
        notes: None,
        condition: Some("Near Mint (NM or M-)".into()),
        suggested_value: Some(value),
        last_value_check: Some("2024-01-01T00:00:00Z".into()),
    }
}

#[tokio::test]
async fn empty_db_returns_zero_totals_and_empty_arrays() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/api/dashboard-stats?timeRange=all").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["totalItems"], 0);
    assert!(body["latestValueMean"].is_null());
    assert_eq!(body["topValuableItems"], serde_json::json!([]));
    assert_eq!(body["leastValuableItems"], serde_json::json!([]));
    assert_eq!(body["latestAdditions"], serde_json::json!([]));
    assert_eq!(body["itemCountHistory"], serde_json::json!([]));
    assert_eq!(body["valueHistory"], serde_json::json!([]));
    assert_eq!(body["genreDistribution"], serde_json::json!({}));
}

#[tokio::test]
async fn populated_db_produces_stable_shape() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };

    // Seed 4 items, mix of values/years/formats.
    items::upsert(
        &st.db,
        &item(
            1,
            101,
            2020,
            "[\"Rock\",\"Pop\"]",
            "1 x Vinyl",
            25.5,
            "2024-01-05T00:00:00Z",
        ),
    )
    .await
    .unwrap();
    items::upsert(
        &st.db,
        &item(
            2,
            102,
            2021,
            "[\"Rock\"]",
            "1 x CD",
            12.0,
            "2024-02-10T00:00:00Z",
        ),
    )
    .await
    .unwrap();
    items::upsert(
        &st.db,
        &item(
            3,
            103,
            2022,
            "[\"Electronic\"]",
            "1 x Vinyl",
            0.5,
            "2024-03-01T00:00:00Z",
        ),
    )
    .await
    .unwrap();
    items::upsert(
        &st.db,
        &item(
            4,
            104,
            0,
            "[\"Pop\"]",
            "File, FLAC",
            2.0,
            "2024-04-01T00:00:00Z",
        ),
    )
    .await
    .unwrap();

    // Two history snapshots.
    stats_history::insert_snapshot(
        &st.db,
        &StatsSnapshot {
            timestamp: "2024-01-01T10:00:00Z".into(),
            total_items: 3,
            value_min: Some(1.0),
            value_mean: Some(10.0),
            value_max: Some(25.0),
        },
    )
    .await
    .unwrap();
    stats_history::insert_snapshot(
        &st.db,
        &StatsSnapshot {
            timestamp: "2024-04-01T10:00:00Z".into(),
            total_items: 4,
            value_min: Some(0.5),
            value_mean: Some(10.0),
            value_max: Some(25.5),
        },
    )
    .await
    .unwrap();

    let server = TestServer::new(router(st));
    let resp = server.get("/api/dashboard-stats?timeRange=all").await;
    resp.assert_status_ok();
    let raw = resp.text();
    let body: serde_json::Value = serde_json::from_str(&raw).unwrap();

    // totalItems = latest snapshot's total_items (4), not from a COUNT(*).
    assert_eq!(body["totalItems"], 4);
    assert_eq!(body["latestValueMin"], 0.5);
    assert_eq!(body["latestValueMean"], 10.0);
    assert_eq!(body["latestValueMax"], 25.5);
    // averageValuePerItem = 10.0 / 4 = 2.5
    assert!((body["averageValuePerItem"].as_f64().unwrap() - 2.5).abs() < 1e-9);

    // Top valuable items = sorted desc by suggested_value, limit 10.
    let top = body["topValuableItems"].as_array().unwrap();
    assert_eq!(top.len(), 4);
    assert_eq!(top[0]["release_id"], 101);
    assert_eq!(top[0]["suggested_value"], 25.5);
    assert_eq!(top[3]["release_id"], 103);
    assert_eq!(top[3]["suggested_value"], 0.5);

    // Latest additions sorted by added_date desc.
    let latest = body["latestAdditions"].as_array().unwrap();
    assert_eq!(latest[0]["release_id"], 104);
    assert_eq!(latest[3]["release_id"], 101);

    // Genre distribution: Rock=2, Pop=2, Electronic=1.
    let genres = body["genreDistribution"].as_object().unwrap();
    assert_eq!(genres["Rock"], 2);
    assert_eq!(genres["Pop"], 2);
    assert_eq!(genres["Electronic"], 1);
    // Wire ordering must be count-descending: the highest-count genre appears first
    // in the raw JSON (tested against the pre-parse string).
    let gd_start = raw.find("\"genreDistribution\":{").unwrap();
    let gd_slice = &raw[gd_start..gd_start + 120];
    let rock_pos = gd_slice.find("Rock").unwrap();
    let electronic_pos = gd_slice.find("Electronic").unwrap();
    assert!(
        rock_pos < electronic_pos,
        "expected Rock to appear before Electronic in wire ordering, got: {gd_slice}"
    );

    // History is ordered ascending by timestamp.
    let history = body["itemCountHistory"].as_array().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0]["timestamp"], "2024-01-01T10:00:00Z");
    assert_eq!(history[1]["timestamp"], "2024-04-01T10:00:00Z");

    // Format distribution buckets: Vinyl=2, CD=1, File=1.
    let fmt = body["formatDistribution"].as_object().unwrap();
    assert_eq!(fmt["Vinyl"], 2);
    assert_eq!(fmt["CD"], 1);
    assert_eq!(fmt["File"], 1);
}
