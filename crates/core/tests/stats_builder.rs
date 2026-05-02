use waxdemon_core::{build_dashboard_stats, DbItem, HistoryRow};

#[allow(clippy::too_many_arguments)]
fn item(
    id: i64,
    release_id: i64,
    artist: &str,
    title: &str,
    year: Option<i32>,
    format: Option<&str>,
    genres_json: Option<&str>,
    suggested: Option<f64>,
    added: &str,
) -> DbItem {
    DbItem {
        id,
        release_id,
        artist: Some(artist.into()),
        title: Some(title.into()),
        year,
        format: format.map(String::from),
        genres: genres_json.map(String::from),
        cover_image_url: None,
        condition: Some("Very Good (VG)".into()),
        suggested_value: suggested,
        added_date: added.into(),
    }
}

#[test]
fn total_items_from_latest_stats_snapshot_when_present() {
    let items = vec![item(
        1,
        101,
        "A",
        "T",
        Some(2020),
        Some("Vinyl"),
        Some("[\"Rock\"]"),
        Some(10.0),
        "2024-01-01T00:00:00Z",
    )];
    let latest = HistoryRow {
        timestamp: "2024-02-01T00:00:00Z".into(),
        total_items: 42,
        value_min: Some(1.0),
        value_mean: Some(8.0),
        value_max: Some(100.0),
    };
    let stats = build_dashboard_stats(&items, Some(&latest), &[]);
    assert_eq!(stats.total_items, 42);
    assert_eq!(stats.latest_value_min, Some(1.0));
    assert_eq!(stats.latest_value_mean, Some(8.0));
    assert_eq!(stats.latest_value_max, Some(100.0));
    // average = mean / total_items
    assert!((stats.average_value_per_item.unwrap() - 8.0 / 42.0).abs() < 1e-9);
}

#[test]
fn total_items_falls_back_to_item_count_when_no_stats() {
    let items = vec![
        item(
            1,
            1,
            "A",
            "T",
            Some(2020),
            Some("Vinyl"),
            None,
            None,
            "2024-01-01",
        ),
        item(
            2,
            2,
            "B",
            "U",
            Some(2021),
            Some("CD"),
            None,
            None,
            "2024-02-01",
        ),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    assert_eq!(stats.total_items, 2);
    assert_eq!(stats.latest_value_mean, None);
    assert_eq!(stats.average_value_per_item, None);
}

#[test]
fn top_and_least_valuable_sorted_and_capped_to_ten() {
    // 12 items with escalating values; only top 10 and bottom 10 (of 12) should come back.
    let items: Vec<DbItem> = (1..=12)
        .map(|n| {
            item(
                n,
                n,
                "A",
                "T",
                Some(2020),
                Some("Vinyl"),
                None,
                Some(n as f64),
                "2024-01-01",
            )
        })
        .collect();
    let stats = build_dashboard_stats(&items, None, &[]);
    assert_eq!(stats.top_valuable_items.len(), 10);
    assert_eq!(stats.least_valuable_items.len(), 10);
    assert_eq!(stats.top_valuable_items[0].suggested_value, Some(12.0));
    assert_eq!(stats.least_valuable_items[0].suggested_value, Some(1.0));
}

#[test]
fn items_with_null_or_zero_value_excluded_from_valuable_lists() {
    let items = vec![
        item(1, 1, "A", "T", Some(2020), None, None, None, "2024-01-01"),
        item(
            2,
            2,
            "B",
            "T",
            Some(2020),
            None,
            None,
            Some(0.0),
            "2024-01-01",
        ),
        item(
            3,
            3,
            "C",
            "T",
            Some(2020),
            None,
            None,
            Some(5.0),
            "2024-01-01",
        ),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    assert_eq!(stats.top_valuable_items.len(), 1);
    assert_eq!(stats.top_valuable_items[0].id, 3);
}

#[test]
fn genre_distribution_counts_and_sorts_desc() {
    let items = vec![
        item(
            1,
            1,
            "A",
            "T",
            Some(2020),
            None,
            Some("[\"Rock\",\"Pop\"]"),
            None,
            "2024-01-01",
        ),
        item(
            2,
            2,
            "B",
            "T",
            Some(2020),
            None,
            Some("[\"Rock\"]"),
            None,
            "2024-01-01",
        ),
        item(
            3,
            3,
            "C",
            "T",
            Some(2020),
            None,
            Some("[\"Electronic\"]"),
            None,
            "2024-01-01",
        ),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    let entries: Vec<(String, i64)> = stats.genre_distribution.iter().cloned().collect();
    // Rock=2 must appear first, then Pop=1 and Electronic=1
    assert_eq!(entries[0], ("Rock".into(), 2));
    assert_eq!(entries.len(), 3);
    let tail: std::collections::HashSet<(String, i64)> = entries[1..].iter().cloned().collect();
    assert!(tail.contains(&("Pop".into(), 1)));
    assert!(tail.contains(&("Electronic".into(), 1)));
}

#[test]
fn year_distribution_unknown_bucket_for_zero_or_missing() {
    let items = vec![
        item(1, 1, "A", "T", Some(2020), None, None, None, "2024-01-01"),
        item(2, 2, "B", "T", Some(0), None, None, None, "2024-01-01"),
        item(3, 3, "C", "T", None, None, None, None, "2024-01-01"),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    let map: std::collections::HashMap<String, i64> =
        stats.year_distribution.iter().cloned().collect();
    assert_eq!(map.get("2020").copied(), Some(1));
    assert_eq!(map.get("Unknown").copied(), Some(2));
}

#[test]
fn format_distribution_primary_bucket() {
    let items = vec![
        item(
            1,
            1,
            "A",
            "T",
            Some(2020),
            Some("1 x Vinyl"),
            None,
            None,
            "2024-01-01",
        ),
        item(
            2,
            2,
            "B",
            "T",
            Some(2020),
            Some("1 x CD"),
            None,
            None,
            "2024-01-01",
        ),
        item(
            3,
            3,
            "C",
            "T",
            Some(2020),
            Some("1 x Cassette"),
            None,
            None,
            "2024-01-01",
        ),
        item(4, 4, "D", "T", Some(2020), None, None, None, "2024-01-01"),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    let map: std::collections::HashMap<String, i64> =
        stats.format_distribution.iter().cloned().collect();
    assert_eq!(map.get("Vinyl").copied(), Some(1));
    assert_eq!(map.get("CD").copied(), Some(1));
    assert_eq!(map.get("Cassette").copied(), Some(1));
    assert_eq!(map.get("Unknown").copied(), Some(1));
}

#[test]
fn latest_additions_sorted_by_added_date_desc() {
    let items = vec![
        item(
            1,
            1,
            "A",
            "T",
            Some(2020),
            None,
            None,
            None,
            "2024-01-01T00:00:00Z",
        ),
        item(
            2,
            2,
            "B",
            "T",
            Some(2020),
            None,
            None,
            None,
            "2024-03-01T00:00:00Z",
        ),
        item(
            3,
            3,
            "C",
            "T",
            Some(2020),
            None,
            None,
            None,
            "2024-02-01T00:00:00Z",
        ),
    ];
    let stats = build_dashboard_stats(&items, None, &[]);
    let ids: Vec<i64> = stats.latest_additions.iter().map(|x| x.id).collect();
    assert_eq!(ids, vec![2, 3, 1]);
}

#[test]
fn history_maps_to_count_and_value_points() {
    let history = vec![
        HistoryRow {
            timestamp: "2024-01-01T00:00:00Z".into(),
            total_items: 5,
            value_min: Some(1.0),
            value_mean: Some(2.0),
            value_max: Some(3.0),
        },
        HistoryRow {
            timestamp: "2024-02-01T00:00:00Z".into(),
            total_items: 7,
            value_min: None,
            value_mean: None,
            value_max: None,
        },
    ];
    let stats = build_dashboard_stats(&[], None, &history);
    assert_eq!(stats.item_count_history.len(), 2);
    assert_eq!(stats.item_count_history[0].count, 5);
    assert_eq!(stats.value_history[1].mean, None);
}
