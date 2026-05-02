//! End-to-end sync test: wiremock upstream + real Postgres.
//! Gated on `TEST_DATABASE_URL`.

use waxdemon_db::{
    get_setting,
    items::{self},
    run_migrations,
    stats_history::{self},
};
use waxdemon_discogs::client::Client;
use waxdemon_sync::run::{run_collection_sync, SyncConfig};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn fresh_pool() -> Option<sqlx::PgPool> {
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
    Some(pool)
}

#[tokio::test]
async fn full_sync_happy_path() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let upstream = MockServer::start().await;

    // Collection page: one release, no next page.
    Mock::given(method("GET"))
        .and(path("/users/u1/collection/folders/0/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pagination": {
                "page": 1, "pages": 1, "per_page": 100, "items": 1,
                "urls": {}
            },
            "releases": [
                {
                    "id": 4242,
                    "instance_id": 9999,
                    "folder_id": 0,
                    "rating": 0,
                    "date_added": "2024-01-01T00:00:00Z",
                    "basic_information": {
                        "id": 4242,
                        "title": "Blue Train",
                        "year": 1957,
                        "resource_url": "",
                        "thumb": "",
                        "cover_image": "https://i.discogs.com/blue.jpg",
                        "formats": [{"name": "Vinyl", "qty": "1"}],
                        "labels": [],
                        "artists": [{"name": "John Coltrane", "id": 1}],
                        "genres": ["Jazz"],
                        "styles": ["Hard Bop"]
                    }
                }
            ]
        })))
        .expect(1)
        .mount(&upstream)
        .await;

    // Collection value.
    Mock::given(method("GET"))
        .and(path("/users/u1/collection/value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "minimum": "$10.00",
            "median": "$25.00",
            "maximum": "$50.00"
        })))
        .expect(1)
        .mount(&upstream)
        .await;

    // Price suggestions.
    Mock::given(method("GET"))
        .and(path("/marketplace/price_suggestions/4242"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Near Mint (NM or M-)": {"currency": "USD", "value": 42.42}
        })))
        .expect(1)
        .mount(&upstream)
        .await;

    let mut client = Client::with_base("token", upstream.uri());
    client.initial_delay_ms = 1;
    client.disable_sleep = true;

    let cfg = SyncConfig {
        username: "u1".into(),
        token: "token".into(),
    };

    let outcome = run_collection_sync(&pool, &client, &cfg).await.unwrap();
    assert_eq!(outcome.item_count, 1);

    // Verify item persisted with chosen suggested_value.
    let rows = items::select_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, 9999);
    assert_eq!(rows[0].release_id, 4242);
    assert_eq!(rows[0].title.as_deref(), Some("Blue Train"));
    assert!((rows[0].suggested_value.unwrap() - 42.42).abs() < 1e-4);

    // Snapshot row with parsed value totals.
    let snap = stats_history::latest_snapshot(&pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(snap.total_items, 1);
    assert!((snap.value_min.unwrap() - 10.0).abs() < 1e-9);
    assert!((snap.value_mean.unwrap() - 25.0).abs() < 1e-9);
    assert!((snap.value_max.unwrap() - 50.0).abs() < 1e-9);

    // Status ended in idle and progress counters advanced.
    assert_eq!(
        get_setting(&pool, "sync_status").await.unwrap().as_deref(),
        Some("idle")
    );
    assert_eq!(
        get_setting(&pool, "sync_total_items")
            .await
            .unwrap()
            .as_deref(),
        Some("1")
    );
}

#[tokio::test]
async fn sync_handles_missing_price_suggestions_as_none() {
    let Some(pool) = fresh_pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let upstream = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/users/u1/collection/folders/0/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "pagination": {"page": 1, "pages": 1, "per_page": 100, "items": 1, "urls": {}},
            "releases": [{
                "id": 1,
                "instance_id": 100,
                "folder_id": 0,
                "rating": 0,
                "date_added": "2024-01-01T00:00:00Z",
                "basic_information": {
                    "id": 1,
                    "title": "T",
                    "year": 2000,
                    "resource_url": "",
                    "thumb": "",
                    "cover_image": "",
                    "formats": [],
                    "labels": [],
                    "artists": [{"name": "A", "id": 1}]
                }
            }]
        })))
        .expect(1)
        .mount(&upstream)
        .await;

    Mock::given(method("GET"))
        .and(path("/users/u1/collection/value"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&upstream)
        .await;

    Mock::given(method("GET"))
        .and(path("/marketplace/price_suggestions/1"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&upstream)
        .await;

    let mut client = Client::with_base("token", upstream.uri());
    client.initial_delay_ms = 1;
    client.disable_sleep = true;

    let cfg = SyncConfig {
        username: "u1".into(),
        token: "token".into(),
    };

    run_collection_sync(&pool, &client, &cfg).await.unwrap();

    let rows = items::select_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].suggested_value.is_none());

    let snap = stats_history::latest_snapshot(&pool)
        .await
        .unwrap()
        .unwrap();
    assert!(snap.value_min.is_none());
    assert!(snap.value_max.is_none());
}
