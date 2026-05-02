//! Smoke test for the server-rendered dashboard page.

use axum_test::TestServer;
use sqlx::postgres::PgPoolOptions;
use waxdemon_db::run_migrations;
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
        Client::new("t".to_string()),
        Some("u".into()),
    ))
}

#[tokio::test]
async fn dashboard_page_renders_html_shell() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/").await;
    resp.assert_status_ok();
    let body = resp.text();
    assert!(
        body.starts_with("<!DOCTYPE html>"),
        "missing doctype: {body:.200}"
    );
    assert!(body.contains("WaxDemon"), "missing brand");
    assert!(body.contains("id=\"sync-btn\""), "missing sync button");
    assert!(
        body.contains("id=\"sync-progress\""),
        "missing progress target"
    );
    // Empty DB => zero totals.
    assert!(body.contains("Items"));
    assert!(
        body.contains("href=\"/favicon.ico\""),
        "missing favicon link"
    );
    assert!(
        body.contains("href=\"/assets/site.webmanifest\""),
        "missing webmanifest link"
    );
    assert!(
        body.contains("href=\"/assets/apple-touch-icon.png\""),
        "missing apple-touch-icon link"
    );
}

#[tokio::test]
async fn favicon_route_serves_ico() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/favicon.ico").await;
    resp.assert_status_ok();
    assert_eq!(
        resp.header("content-type"),
        "image/x-icon",
        "favicon must be served as image/x-icon"
    );
    assert!(!resp.as_bytes().is_empty(), "favicon body empty");
}

#[tokio::test]
async fn webmanifest_route_serves_json() {
    let Some(st) = fresh_state().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let server = TestServer::new(router(st));
    let resp = server.get("/assets/site.webmanifest").await;
    resp.assert_status_ok();
    assert_eq!(
        resp.header("content-type"),
        "application/manifest+json",
        "webmanifest must be served as application/manifest+json"
    );
    let body = resp.text();
    assert!(body.contains("WaxDemon"), "manifest missing brand");
    assert!(
        body.contains("/assets/android-chrome-192x192.png"),
        "manifest icon paths must be absolute under /assets"
    );
}
