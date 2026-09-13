use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::Connection;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::atomic::{AtomicU64, Ordering};
use waxdemon_discogs::oauth::{OAuthClient, OAuthCredentials};
use waxdemon_sync::user::{UserSync, parse_money, run};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::path};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn per_user_lock_prevents_overlap_and_revocation_during_fetch_prevents_writes() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let server = MockServer::start().await;
    let mut lock = pool.acquire().await.unwrap().detach();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(alice.user_id)
        .execute(&mut lock)
        .await
        .unwrap();
    assert!(run(&pool, &client(&server, "alice"), &alice).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    lock.close().await.unwrap();
    Mock::given(path("/users/alice/collection/folders/0/releases"))
        .respond_with(ResponseTemplate::new(200).set_delay(std::time::Duration::from_millis(300)).set_body_json(json!({"pagination":{"page":1,"pages":1,"per_page":100,"items":1,"urls":{}},"releases":[entry(100,"Mint (M)")]})))
        .mount(&server).await;
    let id = alice.user_id;
    let task_pool = pool.clone();
    let api = client(&server, "alice");
    let task = tokio::spawn(async move { run(&task_pool, &api, &alice).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while server.received_requests().await.unwrap().is_empty() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    sqlx::query("UPDATE users SET status='disabled' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(task.await.unwrap().is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_collection_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
    cleanup(pool, admin, schema).await;
}

async fn database() -> (PgPool, PgPool, String) {
    let url =
        std::env::var("TEST_DATABASE_URL").expect("user sync tests require TEST_DATABASE_URL");
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!(
        "user_sync_test_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _| {
            let path = path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path',$1,false)")
                    .bind(path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    waxdemon_db::run_migrations(&pool).await.unwrap();
    (pool, admin, schema)
}

async fn cleanup(pool: PgPool, admin: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn user(pool: &PgPool, id: i64, name: &str) -> UserSync {
    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users(discogs_id,username,status) VALUES ($1,$2,'approved') RETURNING id",
    )
    .bind(id)
    .bind(name)
    .fetch_one(pool)
    .await
    .unwrap();
    let updated: DateTime<Utc> = sqlx::query_scalar("INSERT INTO discogs_connections(user_id,key_id,nonce,ciphertext) VALUES ($1,'test',decode(repeat('00',24),'hex'),decode(repeat('00',16),'hex')) RETURNING updated_at")
        .bind(user_id).fetch_one(pool).await.unwrap();
    let run_id: i64 = sqlx::query_scalar(
        "INSERT INTO user_sync_runs(user_id,status) VALUES ($1,'running') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    UserSync {
        user_id,
        run_id,
        username: name.into(),
        connection_updated_at: updated,
        price_refresh_hours: 24,
    }
}

fn entry(instance: i64, condition: &str) -> Value {
    json!({"id":42,"instance_id":instance,"folder_id":1,"rating":4,"date_added":"2025-01-01T00:00:00Z",
        "notes":[{"field_id":7,"value":condition},{"field_id":8,"value":"My private note"}],
        "basic_information":{"id":42,"title":"Record","year":2025,"resource_url":"","thumb":"","cover_image":"",
            "formats":[{"name":"Vinyl","qty":"1"}],"labels":[],"artists":[{"id":1,"name":"Artist"}],"genres":["Jazz"],"styles":[]}})
}

async fn collection(
    server: &MockServer,
    username: &str,
    entries: Vec<Value>,
    expected_count: usize,
) {
    Mock::given(path(format!("/users/{username}/collection/folders/0/releases")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pagination":{"page":1,"pages":1,"per_page":100,"items":expected_count,"urls":{}},"releases":entries})))
        .mount(server).await;
    Mock::given(path(format!("/users/{username}/collection/fields")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"fields":[{"id":7,"name":"Media"},{"id":8,"name":"Notes"}]})),
        )
        .mount(server)
        .await;
    Mock::given(path(format!("/users/{username}/collection/folders")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"folders":[{"id":1,"name":"Uncategorized"}]})),
        )
        .mount(server)
        .await;
    Mock::given(path(format!("/users/{username}/collection/value")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"minimum":"€10.01","median":"€20.02","maximum":"€30.03"})),
        )
        .mount(server)
        .await;
}

fn client(server: &MockServer, owner: &str) -> waxdemon_discogs::Client {
    let oauth = OAuthClient::with_base(
        OAuthCredentials::new("consumer".into(), "secret".into()).unwrap(),
        &server.uri(),
    )
    .unwrap();
    let mut client = oauth
        .collection_client(OAuthCredentials::new(owner.into(), "owner-secret".into()).unwrap())
        .unwrap();
    client.disable_sleep = true;
    client.max_retries = 0;
    client
}

#[test]
fn parses_money_without_guessing_dollar_currency_or_decimal_commas() {
    assert_eq!(
        parse_money("€1,234.56").unwrap(),
        ("1234.56".parse().unwrap(), Some("EUR".into()))
    );
    assert_eq!(parse_money("$12.34").unwrap().1, None);
    assert_eq!(parse_money("CAD 12.34").unwrap().1, Some("CAD".into()));
    for value in ["€12,34", "€1.234,56", "not money", "-10.00", "XYZ 12.34"] {
        assert!(parse_money(value).is_none(), "{value}");
    }
}

#[tokio::test]
async fn isolates_users_preserves_duplicate_copies_and_decimal_currency_and_reuses_prices() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let bob = user(&pool, 22, "bob").await;
    let server = MockServer::start().await;
    collection(
        &server,
        "alice",
        vec![entry(100, "Very Good (VG)"), entry(101, "Mint (M)")],
        2,
    )
    .await;
    collection(&server, "bob", vec![entry(100, "Mint (M)")], 1).await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(r#"{"Mint (M)":{"currency":"EUR","value":12.123456789123456789},"Very Good (VG)":{"currency":"EUR","value":7.50}}"#,"application/json"))
        .expect(2).mount(&server).await;
    assert_eq!(
        run(&pool, &client(&server, "alice"), &alice).await.unwrap(),
        2
    );
    assert_eq!(run(&pool, &client(&server, "bob"), &bob).await.unwrap(), 1);
    assert_eq!(
        run(&pool, &client(&server, "alice"), &alice).await.unwrap(),
        2
    );
    let rows:Vec<(i64,i64,String,String,String)>=sqlx::query_as("SELECT user_id,instance_id,condition,suggested_value::text,currency FROM user_collection_items ORDER BY user_id,instance_id").fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0],
        (
            alice.user_id,
            100,
            "Very Good (VG)".into(),
            "7.50".into(),
            "EUR".into()
        )
    );
    assert_eq!(rows[1].3, "12.123456789123456789");
    assert_eq!(rows[2].0, bob.user_id);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM releases")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let history: Vec<(String, String)> =
        sqlx::query_as("SELECT value_median::text,currency FROM user_collection_history")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(history, vec![("20.02".into(), "EUR".into()); 2]);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn currency_changes_warn_and_preserve_history_without_affecting_other_users() {
    let (pool, admin, schema) = database().await;
    let mut alice = user(&pool, 11, "alice").await;
    let bob = user(&pool, 22, "bob").await;
    let server = MockServer::start().await;
    collection(&server, "alice", vec![entry(100, "Mint (M)")], 1).await;
    collection(&server, "bob", vec![entry(100, "Mint (M)")], 1).await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Mint (M)":{"currency":"CAD","value":15}
        })))
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    run(&pool, &client(&server, "bob"), &bob).await.unwrap();
    let initial: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(alice.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(initial.is_empty());
    sqlx::query("UPDATE user_sync_runs SET status='completed' WHERE id=$1")
        .bind(alice.run_id)
        .execute(&pool)
        .await
        .unwrap();
    alice.run_id = sqlx::query_scalar(
        "INSERT INTO user_sync_runs(user_id,status) VALUES ($1,'running') RETURNING id",
    )
    .bind(alice.user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE user_price_cache SET fetched_at=now()-interval '2 days' WHERE user_id=$1")
        .bind(alice.user_id)
        .execute(&pool)
        .await
        .unwrap();
    Mock::given(path("/users/alice/collection/value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "minimum":"USD 10", "median":"USD 20", "maximum":"USD 30"
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Mint (M)":{"currency":"USD","value":25}
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    let history: Vec<(String, String)> = sqlx::query_as("SELECT value_median::text,currency FROM user_collection_history WHERE user_id=$1 ORDER BY timestamp::timestamptz")
        .bind(alice.user_id).fetch_all(&pool).await.unwrap();
    assert_eq!(
        history,
        vec![("20.02".into(), "EUR".into()), ("20".into(), "USD".into())]
    );
    let warnings: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(alice.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(warnings.len(), 2);
    assert!(warnings.iter().any(|w| w.contains("EUR to USD")));
    assert!(warnings.iter().any(|w| w.contains("price currency")));
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    let repeated: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(alice.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(repeated, warnings);
    let bob_warnings: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(bob.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(bob_warnings.is_empty());
    let prices: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT user_id,amount::text,currency FROM user_price_suggestions ORDER BY user_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        prices,
        vec![
            (alice.user_id, "25".into(), "USD".into()),
            (bob.user_id, "15".into(), "CAD".into())
        ]
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn mixed_collection_currencies_are_not_saved_as_unknown_values() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let server = MockServer::start().await;
    collection(&server, "alice", vec![], 0).await;
    Mock::given(path("/users/alice/collection/value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "minimum":"€10", "median":"USD 20", "maximum":"€30"
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    let valued: i64 = sqlx::query_scalar("SELECT count(*) FROM user_collection_history WHERE value_min IS NOT NULL OR value_median IS NOT NULL OR value_max IS NOT NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(valued, 0);
    let warnings: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(alice.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("conflicting currencies"))
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn ambiguous_totals_keep_unknown_currency_and_warn_without_using_price_currency() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let server = MockServer::start().await;
    collection(&server, "alice", vec![entry(100, "Mint (M)")], 1).await;
    Mock::given(path("/users/alice/collection/value"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "minimum":"$10", "median":"$20", "maximum":"$30"
        })))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Mint (M)":{"currency":"CAD","value":15}
        })))
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    let history: (String, Option<String>) = sqlx::query_as(
        "SELECT value_median::text,currency FROM user_collection_history WHERE user_id=$1",
    )
    .bind(alice.user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(history, ("20".into(), None));
    let warnings: Vec<String> =
        sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
            .bind(alice.run_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(warnings.iter().any(|w| w.contains("ambiguous")));
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn invalid_price_currencies_preserve_cached_values_and_warn() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let server = MockServer::start().await;
    collection(
        &server,
        "alice",
        vec![
            entry(100, "Mint (M)"),
            entry(101, "Mint (M)"),
            entry(102, "Mint (M)"),
        ],
        3,
    )
    .await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "Mint (M)":{"currency":"EUR","value":15},
            "Very Good (VG)":{"currency":"EUR","value":7}
        })))
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    let cached_before: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY condition) FROM user_price_suggestions p WHERE user_id=$1")
        .bind(alice.user_id).fetch_one(&pool).await.unwrap();
    for currency in ["USD", "XYZ"] {
        sqlx::query("UPDATE user_collection_items SET condition='Mint (M)',suggested_value=15,currency='EUR' WHERE user_id=$1")
            .bind(alice.user_id).execute(&pool).await.unwrap();
        Mock::given(path("/users/alice/collection/folders/0/releases"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"pagination":{"page":1,"pages":1,"per_page":100,"items":3,"urls":{}},"releases":[entry(100,"Mint (M)"),entry(101,"Very Good (VG)"),entry(102,"Poor (P)")]})))
            .with_priority(1).up_to_n_times(1).mount(&server).await;
        sqlx::query("UPDATE user_price_cache SET fetched_at=now()-interval '2 days'")
            .execute(&pool)
            .await
            .unwrap();
        Mock::given(path("/marketplace/price_suggestions/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Mint (M)":{"currency":"EUR","value":100},
                "Very Good (VG)":{"currency":currency,"value":50}
            })))
            .with_priority(1)
            .up_to_n_times(1)
            .mount(&server)
            .await;
        run(&pool, &client(&server, "alice"), &alice).await.unwrap();
        let items: Vec<(String, Option<String>, Option<String>, bool)> = sqlx::query_as(
            "SELECT condition,suggested_value::text,currency,last_value_check IS NOT NULL FROM user_collection_items WHERE user_id=$1 ORDER BY instance_id",
        )
        .bind(alice.user_id)
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            items,
            vec![
                (
                    "Mint (M)".into(),
                    Some("15".into()),
                    Some("EUR".into()),
                    true
                ),
                (
                    "Very Good (VG)".into(),
                    Some("7".into()),
                    Some("EUR".into()),
                    true
                ),
                ("Poor (P)".into(), None, None, false),
            ]
        );
        let cached: Value = sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(p) ORDER BY condition) FROM user_price_suggestions p WHERE user_id=$1")
            .bind(alice.user_id).fetch_one(&pool).await.unwrap();
        assert_eq!(cached, cached_before);
        let warnings: Vec<String> =
            sqlx::query_scalar("SELECT warnings FROM user_sync_runs WHERE id=$1")
                .bind(alice.run_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!warnings.is_empty());
        let fresh: bool = sqlx::query_scalar(
            "SELECT fetched_at>now()-interval '1 day' FROM user_price_cache WHERE user_id=$1",
        )
        .bind(alice.user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!fresh);
    }
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn incomplete_pages_preserve_data_and_empty_collections_only_remove_the_owner() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let bob = user(&pool, 22, "bob").await;
    let server = MockServer::start().await;
    collection(&server, "alice", vec![entry(100, "Mint (M)")], 1).await;
    collection(&server, "bob", vec![entry(100, "Mint (M)")], 1).await;
    Mock::given(path("/marketplace/price_suggestions/42"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    run(&pool, &client(&server, "alice"), &alice).await.unwrap();
    run(&pool, &client(&server, "bob"), &bob).await.unwrap();
    server.reset().await;
    collection(&server, "alice", vec![], 1).await;
    assert!(run(&pool, &client(&server, "alice"), &alice).await.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM user_collection_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    server.reset().await;
    collection(&server, "alice", vec![], 0).await;
    assert_eq!(
        run(&pool, &client(&server, "alice"), &alice).await.unwrap(),
        0
    );
    let owners: Vec<i64> = sqlx::query_scalar("SELECT user_id FROM user_collection_items")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(owners, vec![bob.user_id]);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn disabled_disconnected_or_reconnected_accounts_cannot_sync() {
    let (pool, admin, schema) = database().await;
    let alice = user(&pool, 11, "alice").await;
    let server = MockServer::start().await;
    sqlx::query("UPDATE users SET status='disabled' WHERE id=$1")
        .bind(alice.user_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(run(&pool, &client(&server, "alice"), &alice).await.is_err());
    sqlx::query("UPDATE users SET status='approved' WHERE id=$1")
        .bind(alice.user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE discogs_connections SET updated_at=updated_at+interval '1 second' WHERE user_id=$1",
    )
    .bind(alice.user_id)
    .execute(&pool)
    .await
    .unwrap();
    assert!(run(&pool, &client(&server, "alice"), &alice).await.is_err());
    sqlx::query("DELETE FROM discogs_connections WHERE user_id=$1")
        .bind(alice.user_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(run(&pool, &client(&server, "alice"), &alice).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
    cleanup(pool, admin, schema).await;
}
