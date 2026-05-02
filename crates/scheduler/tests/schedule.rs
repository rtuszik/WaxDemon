use waxdemon_scheduler::{effective_schedule, should_skip_sync, DEFAULT_CRON_SCHEDULE};

#[test]
fn default_when_env_missing_or_blank() {
    assert_eq!(effective_schedule(None), DEFAULT_CRON_SCHEDULE);
    assert_eq!(effective_schedule(Some("")), DEFAULT_CRON_SCHEDULE);
    assert_eq!(effective_schedule(Some("   ")), DEFAULT_CRON_SCHEDULE);
}

#[test]
fn custom_expression_passes_through() {
    assert_eq!(effective_schedule(Some("0 0 3 * * *")), "0 0 3 * * *");
    assert_eq!(
        effective_schedule(Some("  0 */15 * * * *  ")),
        "0 */15 * * * *"
    );
}

#[test]
fn should_skip_sync_only_when_running() {
    assert!(!should_skip_sync(None));
    assert!(!should_skip_sync(Some("idle")));
    assert!(!should_skip_sync(Some("error")));
    assert!(!should_skip_sync(Some("")));
    assert!(should_skip_sync(Some("running")));
}
