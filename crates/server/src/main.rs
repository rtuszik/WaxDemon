use anyhow::Context;
use waxdemon_db::{init_pool, run_migrations};
use waxdemon_discogs::client::Client;
use waxdemon_scheduler::{effective_schedule, setup_scheduler};
use waxdemon_server::{config::Config, router, AppState};
use waxdemon_sync::run::SyncConfig;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env().context("loading config")?;
    let pool = init_pool(&cfg.database_url).await.context("init pool")?;
    run_migrations(&pool).await.context("migrate")?;

    let token = cfg.discogs_token.clone().unwrap_or_default();
    let client = Client::new(token.clone());

    // Launch scheduler only if we have credentials; otherwise skip to avoid red herrings.
    let _scheduler =
        if let (Some(username), false) = (cfg.discogs_username.clone(), token.is_empty()) {
            let expr = effective_schedule(cfg.sync_cron_schedule.as_deref());
            Some(
                setup_scheduler(
                    pool.clone(),
                    client.clone(),
                    SyncConfig { username, token },
                    &expr,
                )
                .await?,
            )
        } else {
            tracing::warn!("discogs credentials missing; sync scheduler not started");
            None
        };

    let state = AppState::new(pool, client, cfg.discogs_username.clone());
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!(%cfg.bind_addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
