use chrono::Utc;
use tracing::{error, info, warn};
use waxdemon_core::{parse_currency, time_range::iso_z, CONDITION_ORDER};
use waxdemon_db::{
    items::{self, UpsertItem},
    set_setting,
    stats_history::{self, StatsSnapshot},
    Db,
};
use waxdemon_discogs::{
    client::{extract_next_path, Client},
    fetch_collection_page, fetch_collection_value, fetch_price_suggestions,
    types::DiscogsReleaseBasic,
};

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub item_count: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub username: String,
    pub token: String,
}

/// Build the initial collection-page endpoint.
fn collection_endpoint(username: &str) -> String {
    format!(
        "/users/{}/collection/folders/0/releases?per_page=100",
        username
    )
}

/// Fetch every release by following pagination links.
pub async fn fetch_all_releases(
    client: &Client,
    username: &str,
) -> Result<Vec<DiscogsReleaseBasic>, anyhow::Error> {
    let mut all = Vec::new();
    let mut next: Option<String> = Some(collection_endpoint(username));
    while let Some(endpoint) = next.take() {
        let page = fetch_collection_page(client, &endpoint).await?;
        info!(
            page = page.pagination.page,
            pages = page.pagination.pages,
            releases_in_page = page.releases.len(),
            total_so_far = all.len() + page.releases.len(),
            "fetched collection page"
        );
        all.extend(page.releases);
        next = page
            .pagination
            .urls
            .next
            .as_deref()
            .and_then(extract_next_path);
    }
    Ok(all)
}

pub fn pick_suggested_value(
    suggestions: &waxdemon_discogs::PriceSuggestionsResponse,
) -> Option<f64> {
    for condition in CONDITION_ORDER {
        if let Some(s) = suggestions.get(*condition) {
            return Some(s.value);
        }
    }
    None
}

pub async fn run_collection_sync(
    pool: &Db,
    client: &Client,
    cfg: &SyncConfig,
) -> Result<SyncOutcome, anyhow::Error> {
    let started = std::time::Instant::now();

    set_setting(pool, "sync_status", "running").await?;
    set_setting(pool, "sync_current_item", "0").await?;
    set_setting(pool, "sync_total_items", "0").await?;
    set_setting(pool, "sync_last_error", "").await?;

    let result: Result<SyncOutcome, anyhow::Error> = async {
        info!("syncing for user {}", cfg.username);

        let releases = fetch_all_releases(client, &cfg.username).await?;
        let total = releases.len();
        if total == 0 {
            anyhow::bail!(
                "Discogs returned 0 releases — refusing to wipe items and write empty snapshot"
            );
        }
        set_setting(pool, "sync_total_items", &total.to_string()).await?;
        info!(total, "collection fetched; starting per-release sync");

        // Best-effort overall value.
        let overall = fetch_collection_value(client, &cfg.username).await.ok();

        let mut tx = pool.begin().await?;
        items::delete_all(&mut *tx).await?;

        let now_iso = iso_z(Utc::now());
        let mut processed = 0usize;
        // Heartbeat every N items so the log shows steady progress without
        // one line per release. Cadence scales with total: ~20 lines max.
        let heartbeat = (total / 20).clamp(1, 50);

        for release in &releases {
            processed += 1;
            // Update progress in a separate connection since we hold tx.
            set_setting(pool, "sync_current_item", &processed.to_string()).await?;

            // Fetch price suggestion (404 → None, other errors logged + skipped).
            let suggestions = match fetch_price_suggestions(client, release.id).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(release_id = release.id, %e, "price suggestions failed");
                    None
                }
            };

            if processed == 1 || processed == total || processed % heartbeat == 0 {
                let pacer = client.pacer_snapshot().await;
                let pct = if total > 0 {
                    (processed as f64 / total as f64) * 100.0
                } else {
                    0.0
                };
                let elapsed = started.elapsed().as_secs_f64();
                info!(
                    processed,
                    total,
                    pct = format!("{:.0}%", pct),
                    elapsed_s = format!("{:.1}", elapsed),
                    rate_remaining = pacer.remaining,
                    rate_quota = pacer.quota,
                    "sync progress"
                );
            }
            let (suggested_value, last_check) = match suggestions {
                Some(s) => (pick_suggested_value(&s), Some(iso_z(Utc::now()))),
                None => (None, None),
            };

            let bi = &release.basic_information;
            let artist = if bi.artists.is_empty() {
                Some("Unknown Artist".to_string())
            } else {
                Some(
                    bi.artists
                        .iter()
                        .map(|a| a.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            };
            let title = Some(if bi.title.is_empty() {
                "Unknown Title".to_string()
            } else {
                bi.title.clone()
            });
            let format = if bi.formats.is_empty() {
                None
            } else {
                Some(
                    bi.formats
                        .iter()
                        .map(|f| {
                            let desc = f
                                .descriptions
                                .as_ref()
                                .map(|d| format!(" ({})", d.join(", ")))
                                .unwrap_or_default();
                            format!("{} x {}{}", f.qty, f.name, desc)
                        })
                        .collect::<Vec<_>>()
                        .join("; "),
                )
            };
            let genres_json = serde_json::to_string(bi.genres.as_ref().unwrap_or(&Vec::new()))?;
            let styles_json = serde_json::to_string(bi.styles.as_ref().unwrap_or(&Vec::new()))?;
            let cover = if bi.cover_image.is_empty() {
                None
            } else {
                Some(bi.cover_image.clone())
            };

            items::upsert(
                &mut *tx,
                &UpsertItem {
                    id: release.instance_id as i32,
                    release_id: release.id as i32,
                    artist,
                    title,
                    year: if bi.year > 0 { Some(bi.year) } else { None },
                    format,
                    genres_json,
                    styles_json,
                    cover_image_url: cover,
                    added_date: release.date_added.clone(),
                    folder_id: Some(release.folder_id as i32),
                    rating: Some(release.rating as i32),
                    notes: None,
                    condition: None,
                    suggested_value,
                    last_value_check: last_check,
                },
            )
            .await?;
        }

        let (min_v, mean_v, max_v) = match &overall {
            Some(v) => (
                parse_currency(Some(&v.minimum)),
                parse_currency(Some(&v.median)),
                parse_currency(Some(&v.maximum)),
            ),
            None => (None, None, None),
        };

        stats_history::insert_snapshot(
            &mut *tx,
            &StatsSnapshot {
                timestamp: now_iso.clone(),
                total_items: total as i32,
                value_min: min_v,
                value_mean: mean_v,
                value_max: max_v,
            },
        )
        .await?;

        tx.commit().await?;

        let secs = started.elapsed().as_secs_f64();
        Ok(SyncOutcome {
            item_count: total,
            message: format!(
                "Sync complete. Processed {total} items in {:.1} seconds.",
                secs
            ),
        })
    }
    .await;

    match result {
        Ok(outcome) => {
            set_setting(pool, "sync_status", "idle").await.ok();
            Ok(outcome)
        }
        Err(e) => {
            error!(%e, "sync failed");
            set_setting(pool, "sync_status", "error").await.ok();
            set_setting(pool, "sync_last_error", &e.to_string())
                .await
                .ok();
            Err(e)
        }
    }
}
