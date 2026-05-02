//! Server-rendered dashboard shell. Charts are rendered client-side by
//! ApexCharts against `/api/dashboard-stats`; this view only produces the HTML
//! skeleton and the static KPI / list sections.

use crate::state::AppState;
use chrono::Utc;
use waxdemon_core::{
    build_dashboard_stats, time_range_filter, DashboardStats, DbItem, HistoryRow, TimeRange,
};
use leptos::prelude::*;

pub async fn render(st: &AppState) -> String {
    use leptos::prelude::RenderHtml;
    let stats = fetch_stats(st).await.unwrap_or_else(|_| default_stats());
    let body = view! { <Dashboard stats=stats /> }.to_html();
    format!("<!DOCTYPE html>{}", body)
}

async fn fetch_stats(st: &AppState) -> Result<DashboardStats, anyhow::Error> {
    let all_items = waxdemon_db::items::select_all(&st.db).await?;
    let latest = waxdemon_db::stats_history::latest_snapshot(&st.db).await?;
    let start = time_range_filter(TimeRange::ThreeMonths, Utc::now());
    let history = waxdemon_db::stats_history::range_query(&st.db, start.as_deref()).await?;

    let db_items: Vec<DbItem> = all_items
        .into_iter()
        .map(|r| DbItem {
            id: r.id as i64,
            release_id: r.release_id as i64,
            artist: r.artist,
            title: r.title,
            year: r.year,
            format: r.format,
            genres: r.genres,
            cover_image_url: r.cover_image_url,
            condition: r.condition,
            suggested_value: r.suggested_value,
            added_date: r.added_date,
        })
        .collect();
    let latest_row = latest.map(|s| HistoryRow {
        timestamp: s.timestamp,
        total_items: s.total_items as i64,
        value_min: s.value_min,
        value_mean: s.value_mean,
        value_max: s.value_max,
    });
    let history_rows: Vec<HistoryRow> = history
        .into_iter()
        .map(|s| HistoryRow {
            timestamp: s.timestamp,
            total_items: s.total_items as i64,
            value_min: s.value_min,
            value_mean: s.value_mean,
            value_max: s.value_max,
        })
        .collect();

    Ok(build_dashboard_stats(
        &db_items,
        latest_row.as_ref(),
        &history_rows,
    ))
}

fn default_stats() -> DashboardStats {
    DashboardStats {
        total_items: 0,
        latest_value_min: None,
        latest_value_mean: None,
        latest_value_max: None,
        average_value_per_item: None,
        item_count_history: vec![],
        value_history: vec![],
        genre_distribution: Default::default(),
        year_distribution: Default::default(),
        format_distribution: Default::default(),
        top_valuable_items: vec![],
        least_valuable_items: vec![],
        latest_additions: vec![],
    }
}

fn fmt_money(n: Option<f64>) -> String {
    match n {
        Some(v) => format!("${:.2}", v),
        None => "—".to_string(),
    }
}

#[component]
fn Dashboard(stats: DashboardStats) -> impl IntoView {
    let total = stats.total_items;
    let value_min_str = fmt_money(stats.latest_value_min);
    let value_mean_str = fmt_money(stats.latest_value_mean);
    let value_max_str = fmt_money(stats.latest_value_max);
    let avg_str = fmt_money(stats.average_value_per_item);

    let top_items_view: Vec<_> = stats
        .top_valuable_items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let img = it
                .cover_image_url
                .clone()
                .unwrap_or_else(|| "/placeholder.svg".into());
            let img_src = if img.starts_with("http") {
                format!("/api/image-proxy?url={}", urlencoding::encode(&img))
            } else {
                img.clone()
            };
            view! {
                <li class="flex items-center gap-3 py-1">
                    <span class="w-6 text-neutral-500">{i + 1}</span>
                    <img src=img_src class="w-10 h-10 object-cover rounded" />
                    <div class="flex-1">
                        <div class="text-neutral-100">{it.title.clone().unwrap_or_default()}</div>
                        <div class="text-neutral-400 text-xs">{it.artist.clone().unwrap_or_default()}</div>
                    </div>
                    <div class="text-right text-neutral-200">{fmt_money(it.suggested_value)}</div>
                </li>
            }
        })
        .collect();

    let genre_items: Vec<_> = stats
        .genre_distribution
        .iter()
        .take(10)
        .map(|(k, v)| {
            view! {
                <li class="flex justify-between text-neutral-300 py-0.5">
                    <span>{k.clone()}</span>
                    <span class="text-neutral-500">{*v}</span>
                </li>
            }
        })
        .collect();

    view! {
        <html lang="en" class="bg-neutral-950 text-neutral-100">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1" />
                <title>"WaxDemon"</title>
                <link rel="icon" type="image/x-icon" href="/favicon.ico" />
                <link rel="icon" type="image/png" sizes="32x32" href="/assets/favicon-32x32.png" />
                <link rel="icon" type="image/png" sizes="16x16" href="/assets/favicon-16x16.png" />
                <link rel="apple-touch-icon" sizes="180x180" href="/assets/apple-touch-icon.png" />
                <link rel="manifest" href="/assets/site.webmanifest" />
                <link rel="stylesheet" href="/assets/styles.css" />
            </head>
            <body class="bg-neutral-950 min-h-screen p-6">
                <header class="flex items-center justify-between mb-6">
                    <h1 class="text-2xl font-semibold text-neutral-100">"WaxDemon"</h1>
                    <div class="flex items-center gap-3">
                        <span id="sync-progress" class="text-sm text-neutral-400"></span>
                        <button id="sync-btn" class="px-3 py-1.5 rounded bg-neutral-800 hover:bg-neutral-700 text-neutral-100">
                            "Sync"
                        </button>
                    </div>
                </header>

                <section class="grid grid-cols-2 md:grid-cols-5 gap-4 mb-6">
                    <KpiCard label="Items".to_string() value=total.to_string() />
                    <KpiCard label="Value (min)".to_string() value=value_min_str />
                    <KpiCard label="Value (mean)".to_string() value=value_mean_str />
                    <KpiCard label="Value (max)".to_string() value=value_max_str />
                    <KpiCard label="Avg / item".to_string() value=avg_str />
                </section>

                <section class="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
                    <div class="bg-neutral-900 rounded-xl p-4">
                        <div id="value-chart"></div>
                    </div>
                    <div class="bg-neutral-900 rounded-xl p-4">
                        <div id="count-chart"></div>
                    </div>
                </section>

                <section class="grid grid-cols-1 lg:grid-cols-2 gap-4 mb-6">
                    <div class="bg-neutral-900 rounded-xl p-4">
                        <h2 class="text-sm uppercase text-neutral-400 mb-2">"Top valuable"</h2>
                        <ul>{top_items_view}</ul>
                    </div>
                    <div class="bg-neutral-900 rounded-xl p-4">
                        <h2 class="text-sm uppercase text-neutral-400 mb-2">"Year distribution"</h2>
                        <div id="year-chart"></div>
                    </div>
                </section>

                <section class="bg-neutral-900 rounded-xl p-4">
                    <h2 class="text-sm uppercase text-neutral-400 mb-2">"Top genres"</h2>
                    <ul>{genre_items}</ul>
                </section>

                <script src="/assets/apexcharts.min.js"></script>
                <script src="/assets/app.js"></script>
            </body>
        </html>
    }
}

#[component]
fn KpiCard(label: String, value: String) -> impl IntoView {
    view! {
        <div class="bg-neutral-900 rounded-xl p-4">
            <div class="text-xs uppercase text-neutral-500">{label}</div>
            <div class="text-2xl font-semibold text-neutral-100 mt-1">{value}</div>
        </div>
    }
}
