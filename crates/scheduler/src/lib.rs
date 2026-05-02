use std::sync::Arc;

use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{error, info, warn};
use waxdemon_db::{get_setting, Db};
use waxdemon_discogs::client::Client;
use waxdemon_sync::run::{run_collection_sync, SyncConfig};

pub const DEFAULT_CRON_SCHEDULE: &str = "0 0 0 * * *";
pub const TIMEZONE: &str = "Europe/Berlin";

pub fn effective_schedule(env: Option<&str>) -> String {
    match env {
        Some(s) if !s.trim().is_empty() => s.trim().to_string(),
        _ => DEFAULT_CRON_SCHEDULE.to_string(),
    }
}

/// Cron tick should skip when the previous sync is still running. Advisory —
/// a process crash mid-sync can leave `sync_status="running"` forever; fixing
/// stale-lock recovery is out of scope here.
pub fn should_skip_sync(status: Option<&str>) -> bool {
    matches!(status, Some("running"))
}

/// Spawn the sync cron job. The returned handle must be kept alive.
pub async fn setup_scheduler(
    pool: Db,
    client: Client,
    cfg: SyncConfig,
    cron_expr: &str,
) -> Result<JobScheduler, anyhow::Error> {
    info!(cron = %cron_expr, "scheduling collection sync");
    let scheduler = JobScheduler::new().await?;
    let pool = Arc::new(pool);
    let client = Arc::new(client);
    let cfg = Arc::new(cfg);

    let job = Job::new_async(cron_expr, move |_uuid, _l| {
        let pool = pool.clone();
        let client = client.clone();
        let cfg = cfg.clone();
        Box::pin(async move {
            match get_setting(&pool, "sync_status").await {
                Ok(status) if should_skip_sync(status.as_deref()) => {
                    info!("skipping scheduled sync — previous run still in progress");
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    warn!(%e, "could not read sync_status; proceeding with sync");
                }
            }
            match run_collection_sync(&pool, &client, &cfg).await {
                Ok(out) => info!("scheduled sync finished: {}", out.message),
                Err(e) => error!(%e, "scheduled sync failed"),
            }
        })
    })?;
    scheduler.add(job).await?;
    scheduler.start().await?;
    Ok(scheduler)
}
