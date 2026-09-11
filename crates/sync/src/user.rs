use anyhow::{Context, ensure};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use sqlx::{Connection, PgConnection, PgPool};
use std::collections::{BTreeSet, HashSet};
use waxdemon_discogs::{
    Client,
    collection::{Entry, Page, Suggestions},
};

pub struct UserSync {
    pub user_id: i64,
    pub run_id: i64,
    pub username: String,
    pub connection_updated_at: DateTime<Utc>,
    pub price_refresh_hours: i32,
}

pub async fn check_access(connection: &mut PgConnection, sync: &UserSync) -> anyhow::Result<()> {
    let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u JOIN discogs_connections c ON c.user_id = u.id JOIN user_sync_runs r ON r.user_id = u.id WHERE u.id = $1 AND u.status = 'approved' AND c.updated_at = $2 AND r.id = $3 AND r.status = 'running')")
        .bind(sync.user_id).bind(sync.connection_updated_at).bind(sync.run_id).fetch_one(connection).await?;
    ensure!(allowed, "sync authorization revoked");
    Ok(())
}

async fn progress(
    connection: &mut PgConnection,
    sync: &UserSync,
    phase: &str,
    processed: usize,
    total: usize,
) -> anyhow::Result<()> {
    check_access(connection, sync).await?;
    sqlx::query("UPDATE user_sync_runs SET phase = $2, processed = $3, total = $4 WHERE id = $1 AND user_id = $5")
        .bind(sync.run_id).bind(phase).bind(i32::try_from(processed)?).bind(i32::try_from(total)?).bind(sync.user_id).execute(connection).await?;
    Ok(())
}

pub fn parse_money(value: &str) -> Option<(Decimal, Option<String>)> {
    let value = value.trim();
    let (number, currency) = if let Some(v) = value.strip_prefix('€') {
        (v, Some("EUR".into()))
    } else if let Some(v) = value.strip_prefix('£') {
        (v, Some("GBP".into()))
    } else if let Some(v) = value.strip_prefix('$') {
        (v, None)
    } else if value.len() >= 3 && value.as_bytes()[..3].iter().all(u8::is_ascii_uppercase) {
        (&value[3..], Some(value[..3].into()))
    } else {
        (value, None)
    };
    let number = number.trim();
    if number
        .split_once('.')
        .is_some_and(|(_, fractional)| fractional.contains(','))
    {
        return None;
    }
    let integer = number.split('.').next()?;
    let groups: Vec<_> = integer.split(',').collect();
    if groups.len() > 1
        && (groups[0].is_empty() || groups[0].len() > 3 || groups[1..].iter().any(|v| v.len() != 3))
    {
        return None;
    }
    let amount: Decimal = number.replace(',', "").parse().ok()?;
    (amount >= Decimal::ZERO).then_some((amount, currency))
}

fn endpoint(username: &str, suffix: &str) -> String {
    let mut url = url::Url::parse("https://api.discogs.com").unwrap();
    url.path_segments_mut().unwrap().extend(["users", username]);
    format!("{}{suffix}", url.path())
}

async fn fetch_entries(
    connection: &mut PgConnection,
    client: &Client,
    sync: &UserSync,
) -> anyhow::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut ids = HashSet::new();
    let mut expected = None;
    let mut page_number = 1;
    loop {
        check_access(connection, sync).await?;
        let page: Page = client
            .request_json(&endpoint(
                &sync.username,
                &format!("/collection/folders/0/releases?per_page=100&page={page_number}"),
            ))
            .await
            .context("collection fetch failed")?;
        ensure!(
            page.pagination.page == page_number
                && page.pagination.items >= 0
                && page.pagination.pages >= 0,
            "invalid collection pagination"
        );
        let count = usize::try_from(page.pagination.items)?;
        if let Some((items, pages)) = expected {
            ensure!(
                (count, page.pagination.pages) == (items, pages),
                "collection changed during pagination; retry required"
            );
        } else {
            expected = Some((count, page.pagination.pages));
        }
        for entry in page.releases {
            ensure!(
                entry.release.id > 0
                    && entry.release.instance_id > 0
                    && ids.insert(entry.release.instance_id),
                "invalid or duplicate collection instance"
            );
            entries.push(entry);
        }
        ensure!(
            entries.len() <= count,
            "collection pagination exceeded expected total"
        );
        progress(connection, sync, "collection", entries.len(), count).await?;
        if page_number >= page.pagination.pages {
            ensure!(entries.len() == count, "incomplete collection pagination");
            return Ok(entries);
        }
        ensure!(
            !entries.is_empty() && page_number < 100_000,
            "invalid collection pagination"
        );
        page_number += 1;
    }
}

pub async fn run(pool: &PgPool, client: &Client, sync: &UserSync) -> anyhow::Result<usize> {
    let mut connection = pool.acquire().await?.detach();
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(sync.user_id)
        .fetch_one(&mut connection)
        .await?;
    ensure!(locked, "another sync is running for this user");
    let result = run_locked(&mut connection, client, sync).await;
    let _ = connection.close().await;
    result
}

pub async fn run_locked(
    connection: &mut PgConnection,
    client: &Client,
    sync: &UserSync,
) -> anyhow::Result<usize> {
    let mut entries = fetch_entries(connection, client, sync).await?;
    check_access(connection, sync).await?;
    let fields: Value = client
        .request_json(&endpoint(&sync.username, "/collection/fields"))
        .await?;
    check_access(connection, sync).await?;
    let folders: Value = client
        .request_json(&endpoint(&sync.username, "/collection/folders"))
        .await?;
    ensure!(
        fields["fields"].is_array() && folders["folders"].is_array(),
        "invalid collection metadata"
    );
    let media_field = fields["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| {
            matches!(
                f["name"].as_str().map(str::to_lowercase).as_deref(),
                Some("media" | "media condition")
            )
        })
        .and_then(|f| f["id"].as_i64());
    check_access(connection, sync).await?;
    let overall = match client
        .request_json::<Value>(&endpoint(&sync.username, "/collection/value"))
        .await
    {
        Ok(value) => Some(value),
        Err(waxdemon_discogs::DiscogsError::Http { status: 401, .. }) => {
            anyhow::bail!("Discogs authorization revoked")
        }
        Err(_) => None,
    };
    check_access(connection, sync).await?;
    let mut tx = connection.begin().await?;
    lock_authorization(&mut tx, sync).await?;
    entries.sort_by_key(|e| (e.release.id, e.release.instance_id));
    for entry in &entries {
        let release = &entry.release;
        let basic = &release.basic_information;
        let artist = basic
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let format = basic
            .formats
            .iter()
            .map(|f| {
                format!(
                    "{} x {}{}",
                    f.qty,
                    f.name,
                    f.descriptions
                        .as_ref()
                        .map(|v| format!(" ({})", v.join(", ")))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        sqlx::query("INSERT INTO releases (id, artist, title, year, format, genres, styles, cover_image_url) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (id) DO UPDATE SET artist=EXCLUDED.artist, title=EXCLUDED.title, year=EXCLUDED.year, format=EXCLUDED.format, genres=EXCLUDED.genres, styles=EXCLUDED.styles, cover_image_url=EXCLUDED.cover_image_url, updated_at=now()")
            .bind(release.id).bind(artist).bind(&basic.title).bind(basic.year).bind(format)
            .bind(serde_json::to_string(&basic.genres.clone().unwrap_or_default())?).bind(serde_json::to_string(&basic.styles.clone().unwrap_or_default())?).bind(&basic.cover_image)
            .execute(&mut *tx).await?;
        let condition = entry
            .notes
            .iter()
            .find(|n| Some(n.field_id) == media_field)
            .map(|n| n.value.as_str());
        sqlx::query("INSERT INTO user_collection_items (user_id, instance_id, release_id, added_date, folder_id, rating, notes, condition) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) ON CONFLICT (user_id,instance_id) DO UPDATE SET release_id=EXCLUDED.release_id, added_date=EXCLUDED.added_date, folder_id=EXCLUDED.folder_id, rating=EXCLUDED.rating, notes=EXCLUDED.notes, condition=EXCLUDED.condition")
            .bind(sync.user_id).bind(release.instance_id).bind(release.id).bind(&release.date_added).bind(release.folder_id).bind(i32::try_from(release.rating)?)
            .bind(serde_json::to_string(&entry.notes)?).bind(condition).execute(&mut *tx).await?;
    }
    let instances: Vec<i64> = entries.iter().map(|e| e.release.instance_id).collect();
    sqlx::query(
        "DELETE FROM user_collection_items WHERE user_id=$1 AND NOT (instance_id = ANY($2))",
    )
    .bind(sync.user_id)
    .bind(instances)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO user_collection_metadata (user_id, fields, folders) VALUES ($1,$2,$3) ON CONFLICT (user_id) DO UPDATE SET fields=EXCLUDED.fields, folders=EXCLUDED.folders, updated_at=now()")
        .bind(sync.user_id).bind(fields).bind(folders).execute(&mut *tx).await?;
    let values: Vec<_> = ["minimum", "median", "maximum"]
        .map(|key| {
            overall
                .as_ref()
                .and_then(|v| v[key].as_str())
                .and_then(parse_money)
        })
        .into();
    let currency = values[0].as_ref().and_then(|v| v.1.clone()).filter(|c| {
        values
            .iter()
            .all(|v| v.as_ref().and_then(|v| v.1.as_ref()) == Some(c))
    });
    sqlx::query("INSERT INTO user_collection_history (user_id, timestamp, total_items, value_min, value_median, value_max, currency, raw_values) SELECT $1, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'), $3, $4::text::numeric, $5::text::numeric, $6::text::numeric, $7, $8 FROM user_sync_runs WHERE id=$2 AND user_id=$1 ON CONFLICT (user_id,timestamp) DO UPDATE SET total_items=EXCLUDED.total_items, value_min=EXCLUDED.value_min, value_median=EXCLUDED.value_median, value_max=EXCLUDED.value_max, currency=EXCLUDED.currency, raw_values=EXCLUDED.raw_values")
        .bind(sync.user_id).bind(sync.run_id).bind(i32::try_from(entries.len())?)
        .bind(values[0].as_ref().map(|v| v.0.to_string())).bind(values[1].as_ref().map(|v| v.0.to_string())).bind(values[2].as_ref().map(|v| v.0.to_string())).bind(currency).bind(overall).execute(&mut *tx).await?;
    tx.commit().await?;
    let releases: BTreeSet<i64> = entries.iter().map(|e| e.release.id).collect();
    for (index, release) in releases.iter().enumerate() {
        progress(connection, sync, "prices", index, releases.len()).await?;
        let fresh: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM user_price_cache WHERE user_id=$1 AND release_id=$2 AND fetched_at > now() - make_interval(hours => $3))")
            .bind(sync.user_id).bind(release).bind(sync.price_refresh_hours).fetch_one(&mut *connection).await?;
        if !fresh {
            let suggestions = match client
                .request_json::<Suggestions>(&format!("/marketplace/price_suggestions/{release}"))
                .await
            {
                Ok(s) => Some(s),
                Err(waxdemon_discogs::DiscogsError::Http {
                    status: 403 | 404, ..
                }) => Some(Suggestions::new()),
                Err(e) => return Err(e.into()),
            };
            if let Some(suggestions) = suggestions {
                ensure!(
                    suggestions.values().all(|v| v.currency.len() == 3
                        && v.currency.bytes().all(|c| c.is_ascii_uppercase())
                        && v.value >= Decimal::ZERO),
                    "invalid price suggestion"
                );
                let mut tx = connection.begin().await?;
                lock_authorization(&mut tx, sync).await?;
                sqlx::query(
                    "DELETE FROM user_price_suggestions WHERE user_id=$1 AND release_id=$2",
                )
                .bind(sync.user_id)
                .bind(release)
                .execute(&mut *tx)
                .await?;
                for (condition, price) in suggestions {
                    sqlx::query("INSERT INTO user_price_suggestions (user_id,release_id,currency,condition,amount,fetched_at) VALUES ($1,$2,$3,$4,$5::text::numeric,now())")
                        .bind(sync.user_id).bind(release).bind(price.currency).bind(condition).bind(price.value.to_string()).execute(&mut *tx).await?;
                }
                sqlx::query("INSERT INTO user_price_cache (user_id,release_id) VALUES ($1,$2) ON CONFLICT (user_id,release_id) DO UPDATE SET fetched_at=now()")
                    .bind(sync.user_id).bind(release).execute(&mut *tx).await?;
                tx.commit().await?;
            }
        }
        let mut tx = connection.begin().await?;
        lock_authorization(&mut tx, sync).await?;
        sqlx::query("UPDATE user_collection_items i SET suggested_value = p.amount, currency = p.currency, last_value_check=p.fetched_at::text FROM user_price_suggestions p WHERE i.user_id=$1 AND i.release_id=$2 AND p.user_id=i.user_id AND p.release_id=i.release_id AND p.condition=i.condition")
            .bind(sync.user_id).bind(release).execute(&mut *tx).await?;
        sqlx::query("UPDATE user_collection_items i SET suggested_value=NULL, currency=NULL, last_value_check=NULL WHERE i.user_id=$1 AND i.release_id=$2 AND NOT EXISTS (SELECT 1 FROM user_price_suggestions p WHERE p.user_id=i.user_id AND p.release_id=i.release_id AND p.condition=i.condition)")
            .bind(sync.user_id).bind(release).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    progress(
        connection,
        sync,
        "completed",
        releases.len(),
        releases.len(),
    )
    .await?;
    Ok(entries.len())
}

async fn lock_authorization(connection: &mut PgConnection, sync: &UserSync) -> anyhow::Result<()> {
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(sync.user_id)
        .execute(&mut *connection)
        .await?;
    check_access(connection, sync).await
}
