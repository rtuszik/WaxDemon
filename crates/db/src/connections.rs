use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct EncryptedConnection {
    pub key_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

pub async fn save_connection(
    pool: &PgPool,
    user_id: i64,
    connection: &EncryptedConnection,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if !matches!(status.as_deref(), Some("pending" | "approved")) {
        tx.rollback().await?;
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO discogs_connections (user_id, key_id, nonce, ciphertext)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id) DO UPDATE SET
             key_id = EXCLUDED.key_id, nonce = EXCLUDED.nonce,
             ciphertext = EXCLUDED.ciphertext, updated_at = now()",
    )
    .bind(user_id)
    .bind(&connection.key_id)
    .bind(&connection.nonce)
    .bind(&connection.ciphertext)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn load_approved_connection(
    pool: &PgPool,
    user_id: i64,
) -> Result<Option<EncryptedConnection>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.key_id, c.nonce, c.ciphertext
         FROM discogs_connections c JOIN users u ON u.id = c.user_id
         WHERE c.user_id = $1 AND u.status = 'approved'",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}
