use waxdemon_core::{SyncStatus, SyncStatusResponse};

#[test]
fn parse_known_statuses() {
    assert_eq!(SyncStatus::parse(Some("idle")), SyncStatus::Idle);
    assert_eq!(SyncStatus::parse(Some("running")), SyncStatus::Running);
    assert_eq!(SyncStatus::parse(Some("error")), SyncStatus::Error);
}

#[test]
fn parse_falls_back_to_unknown() {
    assert_eq!(SyncStatus::parse(None), SyncStatus::Unknown);
    assert_eq!(SyncStatus::parse(Some("garbage")), SyncStatus::Unknown);
    assert_eq!(SyncStatus::parse(Some("")), SyncStatus::Unknown);
}

#[test]
fn response_json_shape_matches_ts_contract() {
    let resp = SyncStatusResponse {
        status: SyncStatus::Running,
        current_item: 7,
        total_items: 42,
        last_error: None,
    };
    let v: serde_json::Value = serde_json::to_value(&resp).unwrap();
    assert_eq!(v["status"], "running");
    assert_eq!(v["currentItem"], 7);
    assert_eq!(v["totalItems"], 42);
    assert!(v["lastError"].is_null());
}
