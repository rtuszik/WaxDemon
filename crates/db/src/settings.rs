use crate::{error::DbError, pool::Db};

/// Insert or update a setting.
pub async fn set_setting(pool: &Db, key: &str, value: &str) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = $2",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

/// Fetch a setting.
pub async fn get_setting(pool: &Db, key: &str) -> Result<Option<String>, DbError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(v,)| v))
}
