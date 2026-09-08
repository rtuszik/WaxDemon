use sqlx::postgres::PgPoolOptions;
use std::sync::atomic::{AtomicU64, Ordering};
use waxdemon_db::legacy_import::{ImportMode, import_legacy};

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(0);

async fn fixture() -> Option<(sqlx::PgPool, sqlx::PgPool, String)> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!(
        "legacy_test_{}_{}",
        std::process::id(),
        NEXT_SCHEMA.fetch_add(1, Ordering::Relaxed)
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .after_connect(move |connection, _| {
            let path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0001_init.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(
        "INSERT INTO collection_items
          (id, release_id, artist, title, genres, styles, added_date, folder_id,
           rating, notes, condition, suggested_value, last_value_check)
         VALUES
          (10, 42, 'Artist', 'Old title', '[\"Rock\"]', '[]', '2020-01-01', 2, 4, 'First copy', 'VG', 12.34, '2024-01-01'),
          (11, 42, 'Artist', 'New title', '[\"Rock\"]', '[]', '2021-01-01', 3, 5, 'Second copy', 'NM', 25.50, '2025-01-01'),
          (12, 43, NULL, 'Other release', NULL, NULL, '2022-01-01', NULL, NULL, NULL, NULL, NULL, NULL);
         INSERT INTO collection_stats_history VALUES
          ('2024-01-01T00:00:00Z', 2, 10.5, 20.25, 30.75),
          ('2025-01-01T00:00:00Z', 3, NULL, NULL, NULL);
         INSERT INTO settings VALUES ('sync_status', 'running'), ('custom', NULL);",
    ).execute(&pool).await.unwrap();
    waxdemon_db::run_migrations(&pool).await.unwrap();
    Some((pool, admin, schema))
}

async fn cleanup(pool: sqlx::PgPool, admin: sqlx::PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
async fn populated_legacy_database_preserves_copies_history_and_unknown_currency() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    let report = import_legacy(&pool, 123, "legacy-owner", ImportMode::Commit)
        .await
        .unwrap();
    assert_eq!(
        (
            report.item_count,
            report.release_count,
            report.history_count,
            report.setting_count
        ),
        (3, 2, 2, 2)
    );
    let owner: (i64, String, String) = sqlx::query_as("SELECT discogs_id, role, status FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, (123, "admin".into(), "approved".into()));
    let copies: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT instance_id, notes, suggested_value::text, currency FROM user_collection_items WHERE release_id = 42 ORDER BY instance_id"
    ).fetch_all(&pool).await.unwrap();
    assert_eq!(
        copies,
        vec![
            (10, "First copy".into(), Some("12.34".into()), None),
            (11, "Second copy".into(), Some("25.5".into()), None)
        ]
    );
    let title: String = sqlx::query_scalar("SELECT title FROM releases WHERE id = 42")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(title, "New title");
    let history: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as("SELECT timestamp, value_median::text, currency FROM user_collection_history ORDER BY timestamp").fetch_all(&pool).await.unwrap();
    assert_eq!(
        history,
        vec![
            ("2024-01-01T00:00:00Z".into(), Some("20.25".into()), None),
            ("2025-01-01T00:00:00Z".into(), None, None)
        ]
    );
    let status: String =
        sqlx::query_scalar("SELECT value FROM user_settings WHERE key = 'sync_status'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "idle");
    let old_status: String =
        sqlx::query_scalar("SELECT value FROM settings WHERE key = 'sync_status'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old_status, "running");
    let old_count: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(old_count, 3);
    let different: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
          SELECT id::bigint, release_id::bigint, added_date, folder_id::bigint,
                 rating, notes, condition, suggested_value::text::numeric, last_value_check
          FROM collection_items
          EXCEPT ALL
          SELECT instance_id, release_id, added_date, folder_id,
                 rating, notes, condition, suggested_value, last_value_check
          FROM user_collection_items
        ) differences",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(different, 0, "all per-copy fields must survive the import");
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn preview_rolls_back_and_commit_cannot_be_repeated_or_reassigned() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    import_legacy(&pool, 123, "owner", ImportMode::Preview)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let receipt: i64 = sqlx::query_scalar("SELECT count(*) FROM legacy_import")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipt, 0);
    import_legacy(&pool, 123, "owner", ImportMode::Commit)
        .await
        .unwrap();
    for id in [123, 456] {
        assert!(
            import_legacy(&pool, id, "someone", ImportMode::Commit)
                .await
                .is_err()
        );
    }
    let owner: i64 = sqlx::query_scalar("SELECT discogs_id FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, 123);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn refuses_to_promote_an_existing_signup() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    sqlx::query("INSERT INTO users (discogs_id, username) VALUES (123, 'visitor')")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        import_legacy(&pool, 123, "visitor", ImportMode::Commit)
            .await
            .is_err()
    );
    let state: (String, String) = sqlx::query_as("SELECT role, status FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, ("user".into(), "pending".into()));
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn invalid_money_rolls_back_without_creating_an_admin() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    sqlx::query("UPDATE collection_items SET suggested_value = 'NaN' WHERE id = 10")
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        import_legacy(&pool, 123, "owner", ImportMode::Commit)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn collection_keys_allow_shared_releases_but_keep_ownership_separate() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    let report = import_legacy(&pool, 123, "owner", ImportMode::Commit)
        .await
        .unwrap();
    let other: i64 = sqlx::query_scalar(
        "INSERT INTO users (discogs_id, username) VALUES (456, 'other') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO user_collection_items (user_id, instance_id, release_id, added_date) VALUES ($1, 10, 42, '2026-01-01')").bind(other).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM user_collection_items WHERE user_id = $1")
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM user_collection_items WHERE user_id = $1")
            .bind(report.user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 3);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn empty_legacy_collection_can_still_bootstrap_its_owner() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    sqlx::query("DELETE FROM collection_items")
        .execute(&pool)
        .await
        .unwrap();
    let report = import_legacy(&pool, 123, "owner", ImportMode::Commit)
        .await
        .unwrap();
    assert_eq!(report.item_count, 0);
    assert_eq!(report.release_count, 0);
    assert_eq!(report.history_count, 2);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn late_failure_rolls_back_admin_releases_and_copies() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    sqlx::raw_sql(
        "CREATE FUNCTION reject_history() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN RAISE EXCEPTION 'simulated history write failure'; END $$;
         CREATE TRIGGER reject_history BEFORE INSERT ON user_collection_history
         FOR EACH ROW EXECUTE FUNCTION reject_history();",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(
        import_legacy(&pool, 123, "owner", ImportMode::Commit)
            .await
            .is_err()
    );
    let rows: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users), (SELECT count(*) FROM releases),
         (SELECT count(*) FROM user_collection_items), (SELECT count(*) FROM legacy_import)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(rows, (0, 0, 0, 0));
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn stores_multiple_currencies_without_overwriting_another_quote() {
    let Some((pool, admin, schema)) = fixture().await else {
        return;
    };
    let report = import_legacy(&pool, 123, "owner", ImportMode::Commit)
        .await
        .unwrap();
    for currency in ["EUR", "USD"] {
        sqlx::query("INSERT INTO user_price_suggestions VALUES ($1, 42, $2, 'VG', 12.34, now())")
            .bind(report.user_id)
            .bind(currency)
            .execute(&pool)
            .await
            .unwrap();
    }
    let currencies: Vec<String> =
        sqlx::query_scalar("SELECT currency FROM user_price_suggestions ORDER BY currency")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(currencies, ["EUR", "USD"]);
    cleanup(pool, admin, schema).await;
}
