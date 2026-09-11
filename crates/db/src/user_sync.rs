use sqlx::{PgConnection, PgPool};

pub async fn enqueue(
    connection: &mut PgConnection,
    user_id: i64,
) -> Result<Option<i64>, sqlx::Error> {
    let user: Option<i64> = sqlx::query_scalar("SELECT u.id FROM users u WHERE u.id=$1 AND u.status='approved' AND EXISTS (SELECT 1 FROM discogs_connections c WHERE c.user_id=u.id) FOR UPDATE")
        .bind(user_id).fetch_optional(&mut *connection).await?;
    if user.is_none() {
        return Ok(None);
    }
    sqlx::query_scalar("INSERT INTO user_sync_runs (user_id) VALUES ($1) ON CONFLICT (user_id) WHERE status IN ('queued','running') DO UPDATE SET user_id=EXCLUDED.user_id RETURNING id")
        .bind(user_id).fetch_optional(connection).await
}

pub async fn enqueue_due(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let users: Vec<i64> = sqlx::query_scalar("SELECT u.id FROM users u LEFT JOIN user_preferences p ON p.user_id=u.id WHERE u.status='approved' AND EXISTS (SELECT 1 FROM discogs_connections c WHERE c.user_id=u.id) AND COALESCE(p.sync_interval_hours,24)>0 AND NOT EXISTS (SELECT 1 FROM user_sync_runs r WHERE r.user_id=u.id AND (r.status IN ('queued','running') OR COALESCE(r.finished_at,r.created_at)>now()-make_interval(hours=>COALESCE(p.sync_interval_hours,24)))) ORDER BY u.id FOR UPDATE OF u SKIP LOCKED LIMIT 100")
        .fetch_all(&mut *tx).await?;
    let mut count = 0;
    for id in users {
        count += u64::from(enqueue(&mut tx, id).await?.is_some());
    }
    tx.commit().await?;
    Ok(count)
}
