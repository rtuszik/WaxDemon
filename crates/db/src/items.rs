use crate::{error::DbError, pool::Db};
use sqlx::Postgres;

/// One-to-one with the `collection_items` table.
#[derive(Debug, Clone)]
pub struct UpsertItem {
    pub id: i32,
    pub release_id: i32,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub format: Option<String>,
    pub genres_json: String,
    pub styles_json: String,
    pub cover_image_url: Option<String>,
    pub added_date: String,
    pub folder_id: Option<i32>,
    pub rating: Option<i32>,
    pub notes: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<f64>,
    pub last_value_check: Option<String>,
}

/// Row shape as read for dashboard stats.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CollectionItemRow {
    pub id: i32,
    pub release_id: i32,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub format: Option<String>,
    pub genres: Option<String>,
    pub cover_image_url: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<f64>,
    pub added_date: String,
}

pub async fn upsert<'e, E: sqlx::Executor<'e, Database = Postgres>>(
    executor: E,
    item: &UpsertItem,
) -> Result<(), DbError> {
    sqlx::query(
        "INSERT INTO collection_items
         (id, release_id, artist, title, year, format, genres, styles, cover_image_url,
          added_date, folder_id, rating, notes, condition, suggested_value, last_value_check)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
         ON CONFLICT (id) DO UPDATE SET
            release_id = $2, artist = $3, title = $4, year = $5, format = $6,
            genres = $7, styles = $8, cover_image_url = $9, added_date = $10,
            folder_id = $11, rating = $12, notes = $13, condition = $14,
            suggested_value = $15, last_value_check = $16",
    )
    .bind(item.id)
    .bind(item.release_id)
    .bind(&item.artist)
    .bind(&item.title)
    .bind(item.year)
    .bind(&item.format)
    .bind(&item.genres_json)
    .bind(&item.styles_json)
    .bind(&item.cover_image_url)
    .bind(&item.added_date)
    .bind(item.folder_id)
    .bind(item.rating)
    .bind(&item.notes)
    .bind(&item.condition)
    .bind(item.suggested_value)
    .bind(&item.last_value_check)
    .execute(executor)
    .await?;
    Ok(())
}

pub async fn delete_all<'e, E: sqlx::Executor<'e, Database = Postgres>>(
    executor: E,
) -> Result<(), DbError> {
    sqlx::query("DELETE FROM collection_items")
        .execute(executor)
        .await?;
    Ok(())
}

pub async fn select_all(pool: &Db) -> Result<Vec<CollectionItemRow>, DbError> {
    // Cast REAL -> DOUBLE PRECISION so sqlx maps it directly to f64.
    let rows = sqlx::query_as::<_, CollectionItemRow>(
        "SELECT id, release_id, artist, title, year, format, genres, cover_image_url,
                condition, suggested_value::DOUBLE PRECISION AS suggested_value, added_date
         FROM collection_items",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
