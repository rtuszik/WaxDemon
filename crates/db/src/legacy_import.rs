use crate::{Db, DbError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    Preview,
    Commit,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ImportReport {
    pub user_id: i64,
    pub item_count: i64,
    pub release_count: i64,
    pub history_count: i64,
    pub setting_count: i64,
}

pub async fn import_legacy(
    pool: &Db,
    discogs_id: i64,
    username: &str,
    mode: ImportMode,
) -> Result<ImportReport, DbError> {
    if discogs_id <= 0 || username.trim().is_empty() {
        return Err(DbError::Config(
            "a verified Discogs identity is required".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    sqlx::raw_sql(
        "LOCK TABLE legacy_import, users, releases, user_collection_items,
         user_collection_history, user_settings IN EXCLUSIVE MODE;
         LOCK TABLE collection_items, collection_stats_history, settings IN SHARE MODE;",
    )
    .execute(&mut *tx)
    .await?;

    let imported: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM legacy_import)")
        .fetch_one(&mut *tx)
        .await?;
    if imported {
        return Err(DbError::Config(
            "legacy data has already been imported; refusing to copy or reassign it".into(),
        ));
    }

    let occupied: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users)")
        .fetch_one(&mut *tx)
        .await?;
    if occupied {
        return Err(DbError::Config("import requires an empty multi-user destination; existing accounts will not be promoted or overwritten".into()));
    }

    let invalid_money: bool = sqlx::query_scalar(
        "SELECT EXISTS (
           SELECT 1 FROM collection_items WHERE suggested_value::text IN ('NaN', 'Infinity', '-Infinity')
           UNION ALL
           SELECT 1 FROM collection_stats_history WHERE
             value_min::text IN ('NaN', 'Infinity', '-Infinity') OR
             value_mean::text IN ('NaN', 'Infinity', '-Infinity') OR
             value_max::text IN ('NaN', 'Infinity', '-Infinity')
         )",
    )
    .fetch_one(&mut *tx)
    .await?;
    if invalid_money {
        return Err(DbError::Config(
            "legacy data contains non-finite monetary values; resolve these before importing"
                .into(),
        ));
    }

    let user_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (discogs_id, username, role, status)
         VALUES ($1, $2, 'admin', 'approved') RETURNING id",
    )
    .bind(discogs_id)
    .bind(username)
    .fetch_one(&mut *tx)
    .await?;

    let release_count = sqlx::query(
        "INSERT INTO releases (id, artist, title, year, format, genres, styles, cover_image_url)
         SELECT DISTINCT ON (release_id)
           release_id, artist, title, year, format, genres, styles, cover_image_url
         FROM collection_items
         ORDER BY release_id, last_value_check DESC NULLS LAST, id DESC",
    )
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    let item_count = sqlx::query(
        "INSERT INTO user_collection_items
         (user_id, instance_id, release_id, added_date, folder_id, rating, notes,
          condition, suggested_value, currency, last_value_check)
         SELECT $1, id, release_id, added_date, folder_id, rating, notes, condition,
                suggested_value::text::numeric, NULL, last_value_check
         FROM collection_items",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    let history_count = sqlx::query(
        "INSERT INTO user_collection_history
         (user_id, timestamp, total_items, value_min, value_median, value_max, currency)
         SELECT $1, timestamp, total_items, value_min::text::numeric,
                value_mean::text::numeric, value_max::text::numeric, NULL
         FROM collection_stats_history",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    let setting_count = sqlx::query(
        "INSERT INTO user_settings (user_id, key, value)
         SELECT $1, key, value FROM settings",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected() as i64;

    sqlx::query("UPDATE user_settings SET value = 'idle' WHERE user_id = $1 AND key = 'sync_status' AND value = 'running'")
        .bind(user_id).execute(&mut *tx).await?;

    let expected: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM collection_items),
                (SELECT count(DISTINCT release_id) FROM collection_items),
                (SELECT count(*) FROM collection_stats_history),
                (SELECT count(*) FROM settings)",
    )
    .fetch_one(&mut *tx)
    .await?;
    if expected != (item_count, release_count, history_count, setting_count) {
        return Err(DbError::Config(
            "legacy import count verification failed".into(),
        ));
    }

    sqlx::query(
        "INSERT INTO legacy_import (user_id, item_count, release_count, history_count, setting_count)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(item_count)
    .bind(release_count)
    .bind(history_count)
    .bind(setting_count)
    .execute(&mut *tx)
    .await?;

    match mode {
        ImportMode::Preview => tx.rollback().await?,
        ImportMode::Commit => tx.commit().await?,
    }
    Ok(ImportReport {
        user_id,
        item_count,
        release_count,
        history_count,
        setting_count,
    })
}
