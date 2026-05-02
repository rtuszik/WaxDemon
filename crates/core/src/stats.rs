use crate::distribution::classify_format;
use crate::types::{
    DashboardStats, ItemCountPoint, LatestAddition, OrderedDist, ValuableItem, ValuePoint,
};

/// Row shape matching the `SELECT ... FROM collection_items` query.
#[derive(Debug, Clone)]
pub struct DbItem {
    pub id: i64,
    pub release_id: i64,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub year: Option<i32>,
    pub format: Option<String>,
    /// JSON array string, e.g. `["Rock","Pop"]`
    pub genres: Option<String>,
    pub cover_image_url: Option<String>,
    pub condition: Option<String>,
    pub suggested_value: Option<f64>,
    pub added_date: String,
}

/// Row shape matching `collection_stats_history` for the history query.
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub timestamp: String,
    pub total_items: i64,
    pub value_min: Option<f64>,
    pub value_mean: Option<f64>,
    pub value_max: Option<f64>,
}

/// Pure assembly function that turns raw DB rows into a `DashboardStats`.
pub fn build_dashboard_stats(
    all_items: &[DbItem],
    latest_stats: Option<&HistoryRow>,
    history: &[HistoryRow],
) -> DashboardStats {
    let total_items = latest_stats
        .map(|s| s.total_items)
        .unwrap_or(all_items.len() as i64);
    let latest_value_min = latest_stats.and_then(|s| s.value_min);
    let latest_value_mean = latest_stats.and_then(|s| s.value_mean);
    let latest_value_max = latest_stats.and_then(|s| s.value_max);

    let average_value_per_item = match (total_items, latest_value_mean) {
        (n, Some(mean)) if n > 0 => Some(mean / n as f64),
        _ => None,
    };

    let item_count_history = history
        .iter()
        .map(|r| ItemCountPoint {
            timestamp: r.timestamp.clone(),
            count: r.total_items,
        })
        .collect();

    let value_history = history
        .iter()
        .map(|r| ValuePoint {
            timestamp: r.timestamp.clone(),
            min: r.value_min,
            mean: r.value_mean,
            max: r.value_max,
        })
        .collect();

    // Items with a positive suggested value, sorted descending & ascending.
    let mut with_value: Vec<&DbItem> = all_items
        .iter()
        .filter(|i| i.suggested_value.map(|v| v > 0.0).unwrap_or(false))
        .collect();

    with_value.sort_by(|a, b| {
        b.suggested_value
            .unwrap()
            .partial_cmp(&a.suggested_value.unwrap())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let top_valuable_items: Vec<ValuableItem> =
        with_value.iter().take(10).map(|i| to_valuable(i)).collect();

    with_value.sort_by(|a, b| {
        a.suggested_value
            .unwrap()
            .partial_cmp(&b.suggested_value.unwrap())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let least_valuable_items: Vec<ValuableItem> =
        with_value.iter().take(10).map(|i| to_valuable(i)).collect();

    // Latest additions: all items sorted by added_date desc, top 10.
    let mut by_added: Vec<&DbItem> = all_items.iter().collect();
    by_added.sort_by(|a, b| b.added_date.cmp(&a.added_date));
    let latest_additions: Vec<LatestAddition> =
        by_added.iter().take(10).map(|i| to_latest(i)).collect();

    // Distributions.
    let mut genre_counts: Vec<(String, i64)> = Vec::new();
    let mut year_counts: Vec<(String, i64)> = Vec::new();
    let mut format_counts: Vec<(String, i64)> = Vec::new();

    for item in all_items {
        if let Some(genres_json) = &item.genres {
            if let Ok(genres) = serde_json::from_str::<Vec<String>>(genres_json) {
                for g in genres {
                    let g = g.trim().to_string();
                    if g.is_empty() {
                        continue;
                    }
                    bump(&mut genre_counts, &g);
                }
            }
        }

        match item.year {
            Some(y) if y > 0 => bump(&mut year_counts, &y.to_string()),
            _ => bump(&mut year_counts, "Unknown"),
        }

        match &item.format {
            Some(_) => {
                let bucket = classify_format(item.format.as_deref());
                bump(&mut format_counts, bucket.as_str());
            }
            None => bump(&mut format_counts, "Unknown"),
        }
    }

    // Sort descending by count, preserve insertion-ordered dict for JSON.
    genre_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    year_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    format_counts.sort_by_key(|(_, c)| std::cmp::Reverse(*c));

    DashboardStats {
        total_items,
        latest_value_min,
        latest_value_mean,
        latest_value_max,
        average_value_per_item,
        item_count_history,
        value_history,
        genre_distribution: OrderedDist::from_sorted(genre_counts),
        year_distribution: OrderedDist::from_sorted(year_counts),
        format_distribution: OrderedDist::from_sorted(format_counts),
        top_valuable_items,
        least_valuable_items,
        latest_additions,
    }
}

fn to_valuable(i: &DbItem) -> ValuableItem {
    ValuableItem {
        id: i.id,
        release_id: i.release_id,
        artist: i.artist.clone(),
        title: i.title.clone(),
        cover_image_url: i.cover_image_url.clone(),
        condition: i.condition.clone(),
        suggested_value: i.suggested_value,
    }
}

fn to_latest(i: &DbItem) -> LatestAddition {
    LatestAddition {
        id: i.id,
        release_id: i.release_id,
        artist: i.artist.clone(),
        title: i.title.clone(),
        cover_image_url: i.cover_image_url.clone(),
        condition: i.condition.clone(),
        suggested_value: i.suggested_value,
        added_date: i.added_date.clone(),
        format: i.format.clone(),
        year: i.year,
    }
}

fn bump(entries: &mut Vec<(String, i64)>, key: &str) {
    if let Some(e) = entries.iter_mut().find(|(k, _)| k == key) {
        e.1 += 1;
    } else {
        entries.push((key.to_string(), 1));
    }
}
