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
    let mut tx = pool.begin().await?;
    let result = sqlx::query(
        "UPDATE settings SET value = 'error' WHERE key = 'sync_status' AND value = 'running'",
    )
    .execute(&mut *tx)
    .await?;

    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Ok(false);
    }

    sqlx::query(
        "INSERT INTO settings (key, value) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind("sync_last_error")
    .bind("sync interrupted by application restart; retrying is safe")
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(true)
}
