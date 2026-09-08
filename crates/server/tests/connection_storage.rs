use secrecy::{ExposeSecret, SecretBox};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use waxdemon_db::connections::{load_approved_connection, save_connection};
use waxdemon_discogs::oauth::OAuthCredentials;
use waxdemon_server::credential_vault::CredentialVault;

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(0);

async fn fixture() -> (PgPool, PgPool, String, CredentialVault) {
    let url = std::env::var("TEST_DATABASE_URL").expect(
        "connection tests require TEST_DATABASE_URL pointing to a disposable PostgreSQL database",
    );
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!(
        "connection_test_{}_{}",
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
    waxdemon_db::run_migrations(&pool).await.unwrap();
    let vault = CredentialVault::new(
        "test-key".into(),
        BTreeMap::from([("test-key".into(), SecretBox::new(Box::new([7; 32])))]),
    )
    .unwrap();
    (pool, admin, schema, vault)
}

async fn cleanup(pool: PgPool, admin: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

async fn user(pool: &PgPool, discogs_id: i64, status: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO users (discogs_id, username, status) VALUES ($1, 'owner', $2) RETURNING id",
    )
    .bind(discogs_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn tokens(token: &str) -> OAuthCredentials {
    OAuthCredentials::new(token.into(), "private-access-secret".into()).unwrap()
}

#[tokio::test]
async fn persists_encrypted_tokens_per_user_and_enforces_approval_on_reads() {
    let (pool, admin, schema, vault) = fixture().await;
    let pending = user(&pool, 1, "pending").await;
    let approved = user(&pool, 2, "approved").await;
    for (id, token) in [(pending, "pending-token"), (approved, "approved-token")] {
        assert!(
            save_connection(&pool, id, &vault.encrypt(id, &tokens(token)).unwrap())
                .await
                .unwrap()
        );
    }
    assert!(
        load_approved_connection(&pool, pending)
            .await
            .unwrap()
            .is_none()
    );
    let row = load_approved_connection(&pool, approved)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        vault
            .decrypt(approved, &row)
            .unwrap()
            .token()
            .expose_secret(),
        "approved-token"
    );
    assert!(vault.decrypt(pending, &row).is_err());
    let stored: Vec<Vec<u8>> = sqlx::query_scalar("SELECT ciphertext FROM discogs_connections")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(stored.len(), 2);
    for ciphertext in stored {
        assert!(
            !ciphertext
                .windows(21)
                .any(|w| w == b"private-access-secret")
        );
    }
    sqlx::query("UPDATE users SET status = 'approved' WHERE id = $1")
        .bind(pending)
        .execute(&pool)
        .await
        .unwrap();
    let row = load_approved_connection(&pool, pending)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        vault
            .decrypt(pending, &row)
            .unwrap()
            .token()
            .expose_secret(),
        "pending-token"
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn replaces_only_owners_connection_and_never_changes_account_permissions() {
    let (pool, admin, schema, vault) = fixture().await;
    let owner = user(&pool, 1, "approved").await;
    let other = user(&pool, 2, "approved").await;
    for id in [owner, other] {
        assert!(
            save_connection(&pool, id, &vault.encrypt(id, &tokens("original")).unwrap())
                .await
                .unwrap()
        );
    }
    assert!(
        save_connection(
            &pool,
            owner,
            &vault.encrypt(owner, &tokens("replacement")).unwrap()
        )
        .await
        .unwrap()
    );
    for (id, expected) in [(owner, "replacement"), (other, "original")] {
        let row = load_approved_connection(&pool, id).await.unwrap().unwrap();
        assert_eq!(
            vault.decrypt(id, &row).unwrap().token().expose_secret(),
            expected
        );
    }
    for status in ["rejected", "disabled"] {
        sqlx::query("UPDATE users SET status = $2 WHERE id = $1")
            .bind(owner)
            .bind(status)
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            !save_connection(
                &pool,
                owner,
                &vault.encrypt(owner, &tokens("blocked")).unwrap()
            )
            .await
            .unwrap()
        );
        assert!(
            load_approved_connection(&pool, owner)
                .await
                .unwrap()
                .is_none()
        );
        let state: (String, String) =
            sqlx::query_as("SELECT role, status FROM users WHERE id = $1")
                .bind(owner)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(state, ("user".into(), status.into()));
    }
    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(owner)
        .execute(&pool)
        .await
        .unwrap();
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM discogs_connections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
    assert!(
        !save_connection(
            &pool,
            owner,
            &vault.encrypt(owner, &tokens("deleted")).unwrap()
        )
        .await
        .unwrap()
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn schema_rejects_invalid_envelopes_without_replacing_existing_credentials() {
    let (pool, admin, schema, vault) = fixture().await;
    let owner = user(&pool, 1, "approved").await;
    let original = vault.encrypt(owner, &tokens("original")).unwrap();
    assert!(save_connection(&pool, owner, &original).await.unwrap());
    for field in 0..3 {
        let mut invalid = original.clone();
        match field {
            0 => invalid.nonce.clear(),
            1 => invalid.ciphertext.clear(),
            _ => invalid.key_id.clear(),
        }
        assert!(save_connection(&pool, owner, &invalid).await.is_err());
        assert_eq!(
            load_approved_connection(&pool, owner)
                .await
                .unwrap()
                .unwrap(),
            original
        );
    }
    cleanup(pool, admin, schema).await;
}
