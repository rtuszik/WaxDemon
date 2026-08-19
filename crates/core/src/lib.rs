pub mod currency;
pub mod distribution;
pub mod stats;
pub mod sync_status;
pub mod time_range;
pub mod types;

pub use currency::parse_currency;
pub use distribution::{FormatBucket, classify_format};
pub use stats::{DbItem, HistoryRow, build_dashboard_stats};
pub use sync_status::{SyncStatus, SyncStatusResponse};
pub use time_range::{TimeRange, time_range_filter};
pub use types::{DashboardStats, ItemCountPoint, LatestAddition, ValuableItem, ValuePoint};

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
