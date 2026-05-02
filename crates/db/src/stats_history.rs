use crate::{error::DbError, pool::Db};
use sqlx::Postgres;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct StatsSnapshot {
    pub timestamp: String,
    pub total_items: i32,
    pub value_min: Option<f64>,
    pub value_mean: Option<f64>,
    pub value_max: Option<f64>,
}

pub async fn insert_snapshot<'e, E: sqlx::Executor<'e, Database = Postgres>>(
    executor: E,
    snap: &StatsSnapshot,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO collection_stats_history
         (timestamp, total_items, value_min, value_mean, value_max)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&snap.timestamp)
    .bind(snap.total_items)
    .bind(snap.value_min)
    .bind(snap.value_mean)
    .bind(snap.value_max)
    .execute(executor)
    .await?;
    Ok(())
}

const SELECT_FIELDS: &str = "timestamp, total_items,
    value_min::DOUBLE PRECISION AS value_min,
    value_mean::DOUBLE PRECISION AS value_mean,
    value_max::DOUBLE PRECISION AS value_max";

/// Latest snapshot.
pub async fn latest_snapshot(pool: &Db) -> Result<Option<StatsSnapshot>, DbError> {
    let sql = format!(
        "SELECT {SELECT_FIELDS} FROM collection_stats_history ORDER BY timestamp DESC LIMIT 1"
    );
    let row = sqlx::query_as::<_, StatsSnapshot>(&sql)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Range query. `start_iso = None` returns all history ascending.
pub async fn range_query(
    pool: &Db,
    start_iso: Option<&str>,
) -> Result<Vec<StatsSnapshot>, DbError> {
    let rows = match start_iso {
        Some(start) => {
            let sql = format!(
                "SELECT {SELECT_FIELDS} FROM collection_stats_history
                 WHERE timestamp >= $1 ORDER BY timestamp ASC"
            );
            sqlx::query_as::<_, StatsSnapshot>(&sql)
                .bind(start)
                .fetch_all(pool)
                .await?
        }
        None => {
            let sql = format!(
                "SELECT {SELECT_FIELDS} FROM collection_stats_history ORDER BY timestamp ASC"
            );
            sqlx::query_as::<_, StatsSnapshot>(&sql)
                .fetch_all(pool)
                .await?
        }
    };
    Ok(rows)
}
