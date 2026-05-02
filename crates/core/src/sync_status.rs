use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Idle,
    Running,
    Error,
    Unknown,
}

impl SyncStatus {
    /// Parse from the value stored in the `settings` table.
    /// Anything not in {idle,running,error} becomes `Unknown`.
    pub fn parse(s: Option<&str>) -> Self {
        match s.unwrap_or("") {
            "idle" => Self::Idle,
            "running" => Self::Running,
            "error" => Self::Error,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncStatusResponse {
    pub status: SyncStatus,
    #[serde(rename = "currentItem")]
    pub current_item: i64,
    #[serde(rename = "totalItems")]
    pub total_items: i64,
    #[serde(rename = "lastError")]
    pub last_error: Option<String>,
}
