use crate::{error::DbError, pool::Db};

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

pub async fn get_setting(pool: &Db, key: &str) -> Result<Option<String>, DbError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = $1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|(v,)| v))
}

pub async fn recover_interrupted_sync(pool: &Db) -> Result<bool, DbError> {
    if get_setting(pool, "sync_status").await?.as_deref() != Some("running") {
        return Ok(false);
    }

    set_setting(pool, "sync_status", "error").await?;
    set_setting(
        pool,
        "sync_last_error",
        "sync interrupted by application restart; retrying is safe",
    )
    .await?;
    Ok(true)
}
