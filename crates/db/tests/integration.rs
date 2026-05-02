//! DB integration tests. Gated on the `TEST_DATABASE_URL` env var;
//! skipped (with a notice) when no real Postgres is available.

use waxdemon_db::{
    get_setting, init_pool,
    items::{self, UpsertItem},
    run_migrations, set_setting,
    stats_history::{self, StatsSnapshot},
};

async fn pool() -> Option<waxdemon_db::Db> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let p = init_pool(&url).await.expect("init pool");
    // Clean state — strictly ephemeral; tests assume a DB reserved for testing.
    sqlx::query("DROP TABLE IF EXISTS collection_items, collection_stats_history, settings, _sqlx_migrations")
        .execute(&p)
        .await
        .expect("drop tables");
    run_migrations(&p).await.expect("migrate");
    Some(p)
}

#[tokio::test]
async fn settings_round_trip() {
    let Some(p) = pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    set_setting(&p, "foo", "bar").await.unwrap();
    assert_eq!(
        get_setting(&p, "foo").await.unwrap().as_deref(),
        Some("bar")
    );
    // Upsert replaces the value.
    set_setting(&p, "foo", "baz").await.unwrap();
    assert_eq!(
        get_setting(&p, "foo").await.unwrap().as_deref(),
        Some("baz")
    );
    // Missing key returns None.
    assert_eq!(get_setting(&p, "nope").await.unwrap(), None);
}

fn sample_item(id: i32) -> UpsertItem {
    UpsertItem {
        id,
        release_id: 1000 + id,
        artist: Some("Artist".into()),
        title: Some("Title".into()),
        year: Some(2020),
        format: Some("1 x Vinyl".into()),
        genres_json: "[\"Rock\"]".into(),
        styles_json: "[]".into(),
        cover_image_url: None,
        added_date: "2024-01-01T00:00:00Z".into(),
        folder_id: Some(0),
        rating: Some(0),
        notes: None,
        condition: Some("Near Mint (NM or M-)".into()),
        suggested_value: Some(12.5),
        last_value_check: Some("2024-01-01T00:00:00Z".into()),
    }
}

#[tokio::test]
async fn upsert_insert_then_update() {
    let Some(p) = pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };

    items::upsert(&p, &sample_item(1)).await.unwrap();

    let mut updated = sample_item(1);
    updated.title = Some("Updated".into());
    items::upsert(&p, &updated).await.unwrap();

    let rows = items::select_all(&p).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].title.as_deref(), Some("Updated"));
}

#[tokio::test]
async fn stats_history_insert_and_range() {
    let Some(p) = pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    let snap = |ts: &str, n: i32| StatsSnapshot {
        timestamp: ts.into(),
        total_items: n,
        value_min: Some(1.0),
        value_mean: Some(5.0),
        value_max: Some(10.0),
    };
    stats_history::insert_snapshot(&p, &snap("2024-01-01T00:00:00Z", 1))
        .await
        .unwrap();
    stats_history::insert_snapshot(&p, &snap("2024-02-01T00:00:00Z", 2))
        .await
        .unwrap();
    stats_history::insert_snapshot(&p, &snap("2024-03-01T00:00:00Z", 3))
        .await
        .unwrap();

    let latest = stats_history::latest_snapshot(&p).await.unwrap().unwrap();
    assert_eq!(latest.timestamp, "2024-03-01T00:00:00Z");

    let all = stats_history::range_query(&p, None).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].timestamp, "2024-01-01T00:00:00Z");

    let since_feb = stats_history::range_query(&p, Some("2024-02-01T00:00:00Z"))
        .await
        .unwrap();
    assert_eq!(since_feb.len(), 2);
}

#[tokio::test]
async fn items_delete_all_and_reinsert() {
    let Some(p) = pool().await else {
        eprintln!("skipping: TEST_DATABASE_URL not set");
        return;
    };
    items::upsert(&p, &sample_item(1)).await.unwrap();
    items::upsert(&p, &sample_item(2)).await.unwrap();
    items::delete_all(&p).await.unwrap();
    assert!(items::select_all(&p).await.unwrap().is_empty());
    items::upsert(&p, &sample_item(3)).await.unwrap();
    assert_eq!(items::select_all(&p).await.unwrap().len(), 1);
}
