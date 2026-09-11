use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store,
};

#[derive(Clone, Debug)]
pub struct PgSessionStore {
    pool: PgPool,
}

impl PgSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn delete_expired(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM app_sessions WHERE expires_at <= now()")
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM oauth_attempts WHERE expires_at <= now()")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

pub(super) fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn store_error(_: impl std::fmt::Display) -> session_store::Error {
    session_store::Error::Backend("session storage operation failed".into())
}

fn expiry(record: &Record) -> session_store::Result<DateTime<Utc>> {
    DateTime::from_timestamp(record.expiry_date.unix_timestamp(), 0)
        .ok_or_else(|| store_error("invalid expiry"))
}

#[async_trait]
impl SessionStore for PgSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let data = serde_json::to_value(&record.data).map_err(store_error)?;
        for _ in 0..16 {
            let inserted = sqlx::query("INSERT INTO app_sessions (id_hash, data, expires_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
                .bind(hash(&record.id.to_string())).bind(&data).bind(expiry(record)?)
                .execute(&self.pool).await.map_err(store_error)?.rows_affected();
            if inserted == 1 {
                return Ok(());
            }
            record.id = Id::default();
        }
        Err(store_error("session ID collision"))
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = serde_json::to_value(&record.data).map_err(store_error)?;
        sqlx::query("UPDATE app_sessions SET data = $2, expires_at = LEAST(expires_at, $3) WHERE id_hash = $1 AND expires_at > now()")
            .bind(hash(&record.id.to_string())).bind(data).bind(expiry(record)?)
            .execute(&self.pool).await.map_err(store_error)?;
        Ok(())
    }

    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let row: Option<(serde_json::Value, DateTime<Utc>)> = sqlx::query_as(
            "SELECT data, expires_at FROM app_sessions WHERE id_hash = $1 AND expires_at > now()",
        )
        .bind(hash(&id.to_string()))
        .fetch_optional(&self.pool)
        .await
        .map_err(store_error)?;
        row.map(|(data, expires)| {
            Ok(Record {
                id: *id,
                data: serde_json::from_value(data).map_err(store_error)?,
                expiry_date: OffsetDateTime::from_unix_timestamp(expires.timestamp())
                    .map_err(store_error)?,
            })
        })
        .transpose()
    }

    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        sqlx::query("DELETE FROM app_sessions WHERE id_hash = $1")
            .bind(hash(&id.to_string()))
            .execute(&self.pool)
            .await
            .map_err(store_error)?;
        Ok(())
    }
}
