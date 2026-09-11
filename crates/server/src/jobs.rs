use crate::auth::AuthState;
use apalis::prelude::*;
use apalis_sql::{Config, postgres::PostgresStorage};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Connection, PgConnection, PgPool};
use std::time::Duration;
use waxdemon_db::connections::EncryptedConnection;
use waxdemon_sync::user::UserSync;

pub const NAMESPACE: &str = "waxdemon.collection-sync.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncJob {
    pub user_id: i64,
    pub run_id: i64,
}

#[derive(sqlx::FromRow)]
struct SyncOwner {
    username: String,
    updated_at: DateTime<Utc>,
    #[sqlx(flatten)]
    credentials: EncryptedConnection,
    refresh_hours: i32,
}

pub async fn setup(pool: &PgPool) -> anyhow::Result<()> {
    let mut connection = pool.acquire().await?.detach();
    sqlx::query("SELECT pg_advisory_lock(-1)")
        .execute(&mut connection)
        .await?;
    sqlx::query("CREATE SCHEMA IF NOT EXISTS waxdemon_queue_migrations")
        .execute(&mut connection)
        .await?;
    sqlx::query("SET search_path TO waxdemon_queue_migrations")
        .execute(&mut connection)
        .await?;
    PostgresStorage::migrations().run(&mut connection).await?;
    connection.close().await?;
    Ok(())
}

pub async fn dispatch(pool: &PgPool) -> anyhow::Result<usize> {
    sqlx::query("UPDATE user_sync_runs r SET status='failed',phase='failed',finished_at=now(),error='Sync worker exhausted its retries' FROM apalis.jobs j WHERE r.job_id=j.id AND r.status IN ('queued','running') AND (j.status='Killed' OR (j.status='Failed' AND j.attempts>=j.max_attempts))")
        .execute(pool).await?;
    let mut tx = pool.begin().await?;
    let runs: Vec<(i64,i64)> = sqlx::query_as("SELECT id,user_id FROM user_sync_runs WHERE status='queued' AND queued_at IS NULL ORDER BY id FOR UPDATE SKIP LOCKED LIMIT 100")
        .fetch_all(&mut *tx).await?;
    for (run_id, user_id) in &runs {
        let job = SyncJob {
            user_id: *user_id,
            run_id: *run_id,
        };
        let task_id = TaskId::new().to_string();
        sqlx::query("INSERT INTO apalis.jobs (id,job,job_type,max_attempts) VALUES ($1,$2,$3,3)")
            .bind(&task_id)
            .bind(serde_json::to_value(job)?)
            .bind(NAMESPACE)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE user_sync_runs SET queued_at=now(),job_id=$2 WHERE id=$1")
            .bind(run_id)
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(runs.len())
}

async fn cancel(connection: &mut PgConnection, job: &SyncJob) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE user_sync_runs SET status='cancelled',phase='cancelled',error='Sync authorization is no longer valid',finished_at=now() WHERE id=$1 AND user_id=$2 AND status IN ('queued','running')")
        .bind(job.run_id).bind(job.user_id).execute(connection).await?;
    Ok(())
}

async fn execute(job: SyncJob, state: Data<AuthState>, task_id: TaskId) -> Result<(), Error> {
    match execute_inner(&job, &state, &task_id).await {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = sqlx::query(
                "UPDATE apalis.jobs SET run_at=now()+interval '30 seconds' WHERE id=$1",
            )
            .bind(task_id.to_string())
            .execute(&state.pool)
            .await;
            Err(Error::from(BoxDynError::from(
                "collection synchronization failed",
            )))
        }
    }
}

async fn execute_inner(job: &SyncJob, state: &AuthState, task_id: &TaskId) -> anyhow::Result<()> {
    let mut connection = state.pool.acquire().await?.detach();
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(job.user_id)
        .fetch_one(&mut connection)
        .await?;
    anyhow::ensure!(locked, "another sync is running for this user");
    let status: Option<String> =
        sqlx::query_scalar("SELECT status FROM user_sync_runs WHERE id=$1 AND user_id=$2")
            .bind(job.run_id)
            .bind(job.user_id)
            .fetch_optional(&mut connection)
            .await?;
    if !matches!(status.as_deref(), Some("queued" | "running")) {
        return Ok(());
    }
    let owner: Option<SyncOwner> = sqlx::query_as("SELECT u.username,c.updated_at,c.key_id,c.nonce,c.ciphertext,COALESCE(p.price_refresh_hours,24) AS refresh_hours FROM users u JOIN discogs_connections c ON c.user_id=u.id LEFT JOIN user_preferences p ON p.user_id=u.id WHERE u.id=$1 AND u.status='approved'")
        .bind(job.user_id).fetch_optional(&mut connection).await?;
    let Some(owner) = owner else {
        cancel(&mut connection, job).await?;
        return Ok(());
    };
    let attempts: i32=sqlx::query_scalar("UPDATE user_sync_runs SET status='running',started_at=now(),attempts=attempts+1,error=NULL WHERE id=$1 RETURNING attempts")
        .bind(job.run_id).fetch_one(&mut connection).await?;
    if attempts > 3 {
        sqlx::query("UPDATE user_sync_runs SET status='failed',phase='failed',finished_at=now(),error='Sync failed after three attempts' WHERE id=$1")
            .bind(job.run_id).execute(&mut connection).await?;
        return Ok(());
    }
    let sync = UserSync {
        user_id: job.user_id,
        run_id: job.run_id,
        username: owner.username,
        connection_updated_at: owner.updated_at,
        price_refresh_hours: owner.refresh_hours,
    };
    let result = async {
        let credentials = state.vault.decrypt(job.user_id, &owner.credentials)?;
        let client = state.oauth.collection_client(credentials)?;
        waxdemon_sync::user::run_locked(&mut connection, &client, &sync).await
    }
    .await;
    match result {
        Ok(_) => {
            sqlx::query("UPDATE user_sync_runs SET status='completed',phase='completed',finished_at=now(),error=NULL WHERE id=$1 AND user_id=$2 AND status='running'")
                .bind(job.run_id).bind(job.user_id).execute(&mut connection).await?;
        }
        Err(_) => {
            if waxdemon_sync::user::check_access(&mut connection, &sync)
                .await
                .is_err()
            {
                cancel(&mut connection, job).await?;
            } else {
                let mut tx = connection.begin().await?;
                sqlx::query("UPDATE user_sync_runs SET status=$2,phase=$2,error=$3,finished_at=CASE WHEN $2='failed' THEN now() ELSE NULL END WHERE id=$1")
                    .bind(job.run_id).bind(if attempts>=3 {"failed"} else {"queued"})
                    .bind(if attempts>=3 {"Sync failed after three attempts"} else {"Sync failed; retry scheduled"}).execute(&mut *tx).await?;
                sqlx::query("UPDATE apalis.jobs SET run_at=now()+make_interval(secs => $2),max_attempts=CASE WHEN $3 THEN attempts ELSE max_attempts END WHERE id=$1")
                    .bind(task_id.to_string()).bind(f64::from(30*attempts)).bind(attempts>=3).execute(&mut *tx).await?;
                tx.commit().await?;
                if attempts < 3 {
                    anyhow::bail!("retry required");
                }
            }
        }
    }
    connection.close().await?;
    Ok(())
}

pub async fn run(state: AuthState) -> anyhow::Result<()> {
    let storage = PostgresStorage::<SyncJob>::new_with_config(
        state.pool.clone(),
        Config::new(NAMESPACE)
            .set_buffer_size(1)
            .set_poll_interval(Duration::from_secs(1)),
    );
    let worker = WorkerBuilder::new(format!("waxdemon-{}", TaskId::new()))
        .concurrency(2)
        .catch_panic()
        .data(state.clone())
        .backend(storage)
        .build_fn(execute);
    let scheduling = async {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if waxdemon_db::user_sync::enqueue_due(&state.pool)
                .await
                .is_err()
                || dispatch(&state.pool).await.is_err()
            {
                tracing::warn!("sync scheduling failed; retrying on next tick");
            }
        }
    };
    tokio::select! {
        _ = worker.run() => anyhow::bail!("sync worker stopped"),
        _ = scheduling => anyhow::bail!("sync scheduler stopped"),
    }
}
