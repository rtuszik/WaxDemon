//! Tests for `GET /api/collection/sync/status`.

use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;
use waxdemon_db::{run_migrations, set_setting};
use waxdemon_discogs::client::Client;
use waxdemon_server::{router, AppState};

async fn fresh_state() -> Option<AppState> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .ok()?;
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

#[tokio::test]
async fn unknown_when_no_settings_present() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/api/collection/sync/status").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "unknown");
    assert_eq!(body["currentItem"], 0);
    assert_eq!(body["totalItems"], 0);
    assert!(body["lastError"].is_null());
}

#[tokio::test]
async fn running_state_reflects_settings() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    set_setting(&st.db, "sync_status", "running").await.unwrap();
    set_setting(&st.db, "sync_current_item", "7").await.unwrap();
    set_setting(&st.db, "sync_total_items", "42").await.unwrap();
    let server = TestServer::new(router(st));
    let resp = server.get("/api/collection/sync/status").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "running");
    assert_eq!(body["currentItem"], 7);
    assert_eq!(body["totalItems"], 42);
}

#[tokio::test]
async fn error_state_surfaces_last_error() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    set_setting(&st.db, "sync_status", "error").await.unwrap();
    set_setting(&st.db, "sync_last_error", "boom")
        .await
        .unwrap();
    let server = TestServer::new(router(st));
    let resp = server.get("/api/collection/sync/status").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "error");
    assert_eq!(body["lastError"], "boom");
}

#[tokio::test]
async fn garbage_status_falls_back_to_unknown() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    set_setting(&st.db, "sync_status", "nonsense")
        .await
        .unwrap();
    let server = TestServer::new(router(st));
    let resp = server.get("/api/collection/sync/status").await;
    resp.assert_status_ok();
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "unknown");
}
