pub mod currency;
pub mod distribution;
pub mod stats;
pub mod sync_status;
pub mod time_range;
pub mod types;

pub use currency::parse_currency;
pub use distribution::{classify_format, FormatBucket};
pub use stats::{build_dashboard_stats, DbItem, HistoryRow};
pub use sync_status::{SyncStatus, SyncStatusResponse};
pub use time_range::{time_range_filter, TimeRange};
pub use types::{DashboardStats, ItemCountPoint, LatestAddition, ValuableItem, ValuePoint};

/// Preferred condition order when picking a price from Discogs price suggestions.
pub const CONDITION_ORDER: &[&str] = &[
    "Mint (M)",
    "Near Mint (NM or M-)",
    "Very Good Plus (VG+)",
    "Very Good (VG)",
    "Good Plus (G+)",
    "Good (G)",
    "Fair (F)",
    "Poor (P)",
];
