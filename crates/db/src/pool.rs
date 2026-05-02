use crate::error::DbError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

pub type Db = PgPool;

pub async fn init_pool(connection_string: &str) -> Result<Db, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .idle_timeout(Some(Duration::from_secs(30)))
        .acquire_timeout(Duration::from_secs(10))
        .connect(connection_string)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &Db) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}
