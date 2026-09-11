use anyhow::Context;
use tracing_subscriber::EnvFilter;
use waxdemon_db::{init_pool, run_migrations};
use waxdemon_server::{auth, config::Config};

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
    let auth = auth::config::from_env(pool.clone()).await?;
    waxdemon_server::jobs::setup(&pool).await?;
    let store = auth::store::PgSessionStore::new(pool);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if store.delete_expired().await.is_err() {
                tracing::warn!("authentication expiry cleanup failed");
            }
        }
    });
    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!(%cfg.bind_addr, "listening");
    tokio::select! {
        result = axum::serve(listener, auth.clone().router()).into_future() => result?,
        result = waxdemon_server::jobs::run(auth) => result?,
    }
    Ok(())
}
