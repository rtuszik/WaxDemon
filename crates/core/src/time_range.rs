use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeRange {
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "1m")]
    OneMonth,
    #[serde(rename = "3m")]
    ThreeMonths,
    #[serde(rename = "6m")]
    SixMonths,
    #[serde(rename = "1y")]
    OneYear,
    #[serde(rename = "all")]
    All,
}

impl TimeRange {
    pub fn parse(s: &str) -> Self {
        match s {
            "7d" => Self::SevenDays,
            "1m" => Self::OneMonth,
            "3m" => Self::ThreeMonths,
            "6m" => Self::SixMonths,
            "1y" => Self::OneYear,
            "all" => Self::All,
            _ => Self::ThreeMonths,
        }
    }
}

/// Return the lower-bound ISO-8601 timestamp for the given time range, or `None` for "all".
pub fn time_range_filter(range: TimeRange, now: DateTime<Utc>) -> Option<String> {
    let days = match range {
        TimeRange::SevenDays => 7,
        TimeRange::OneMonth => 30,
        TimeRange::ThreeMonths => 90,
        TimeRange::SixMonths => 180,
        TimeRange::OneYear => 365,
        TimeRange::All => return None,
    };
    let start = now - Duration::days(days);
    Some(iso_z(start))
}

/// Format like JS `Date#toISOString()`: `YYYY-MM-DDTHH:MM:SS.sssZ`.
pub fn iso_z(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
