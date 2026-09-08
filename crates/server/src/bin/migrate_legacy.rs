use anyhow::Context;
use waxdemon_db::legacy_import::{ImportMode, import_legacy};
use waxdemon_discogs::client::Client;
use waxdemon_server::legacy_migration::verify_owner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mode = match args.as_slice() {
        [arg] if arg == "--preview" => ImportMode::Preview,
        [arg] if arg == "--commit" => ImportMode::Commit,
        _ => anyhow::bail!("usage: waxdemon-migrate-legacy --preview|--commit"),
    };
    let username = std::env::var("DISCOGS_USERNAME").context("DISCOGS_USERNAME is required")?;
    let token = std::env::var("DISCOGS_TOKEN").context("DISCOGS_TOKEN is required")?;
    if token.trim().is_empty() {
        anyhow::bail!("DISCOGS_TOKEN must not be empty");
    }
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;

    let identity = verify_owner(&Client::new(token), &username).await?;
    let pool = waxdemon_db::init_pool(&database_url)
        .await
        .map_err(|_| anyhow::anyhow!("could not connect to the migration database"))?;
    waxdemon_db::run_migrations(&pool).await?;
    let report = import_legacy(&pool, identity.id, &identity.username, mode).await?;
    println!(
        "{mode:?}: admin Discogs ID {}; {} copies, {} releases, {} history snapshots, {} settings",
        identity.id,
        report.item_count,
        report.release_count,
        report.history_count,
        report.setting_count,
    );
    if mode == ImportMode::Preview {
        println!("Imported rows rolled back; additive schema migrations remain applied.");
    } else {
        println!(
            "Legacy tables retained. This command does not switch the application to multi-user mode."
        );
    }
    pool.close().await;
    Ok(())
}
