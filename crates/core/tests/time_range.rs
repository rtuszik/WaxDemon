use chrono::{TimeZone, Utc};
use waxdemon_core::{time_range_filter, TimeRange};

#[test]
fn parse_each_variant() {
    assert_eq!(TimeRange::parse("7d"), TimeRange::SevenDays);
    assert_eq!(TimeRange::parse("1m"), TimeRange::OneMonth);
    assert_eq!(TimeRange::parse("3m"), TimeRange::ThreeMonths);
    assert_eq!(TimeRange::parse("6m"), TimeRange::SixMonths);
    assert_eq!(TimeRange::parse("1y"), TimeRange::OneYear);
    assert_eq!(TimeRange::parse("all"), TimeRange::All);
    assert_eq!(TimeRange::parse("garbage"), TimeRange::ThreeMonths);
}

#[test]
fn all_returns_none() {
    let now = Utc.with_ymd_and_hms(2024, 6, 1, 12, 0, 0).unwrap();
    assert!(time_range_filter(TimeRange::All, now).is_none());
}

#[test]
fn seven_days_subtracts_seven_days() {
    let now = Utc.with_ymd_and_hms(2024, 6, 8, 12, 0, 0).unwrap();
    assert_eq!(
        time_range_filter(TimeRange::SevenDays, now).as_deref(),
        Some("2024-06-01T12:00:00.000Z")
    );
}

#[test]
fn one_month_subtracts_thirty_days() {
    let now = Utc.with_ymd_and_hms(2024, 6, 30, 12, 0, 0).unwrap();
    assert_eq!(
        time_range_filter(TimeRange::OneMonth, now).as_deref(),
        Some("2024-05-31T12:00:00.000Z")
    );
}

#[test]
fn three_months_subtracts_ninety_days() {
    let now = Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap();
    assert_eq!(
        time_range_filter(TimeRange::ThreeMonths, now).as_deref(),
        Some("2024-03-03T00:00:00.000Z")
    );
}
