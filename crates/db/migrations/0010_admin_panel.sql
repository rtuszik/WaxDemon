-- squawk-ignore-file ban-drop-not-null, ban-drop-default
SET LOCAL lock_timeout = '10s';
SET LOCAL statement_timeout = '30s';

CREATE TABLE IF NOT EXISTS app_settings (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    sync_interval_hours BIGINT NOT NULL DEFAULT 24 CHECK (sync_interval_hours BETWEEN 0 AND 720),
    price_refresh_hours BIGINT NOT NULL DEFAULT 24 CHECK (price_refresh_hours BETWEEN 1 AND 720)
);
INSERT INTO app_settings DEFAULT VALUES ON CONFLICT DO NOTHING;

ALTER TABLE user_preferences
    ALTER COLUMN sync_interval_hours DROP NOT NULL,
    ALTER COLUMN sync_interval_hours DROP DEFAULT,
    ALTER COLUMN price_refresh_hours DROP NOT NULL,
    ALTER COLUMN price_refresh_hours DROP DEFAULT;
UPDATE user_preferences SET sync_interval_hours = NULL, price_refresh_hours = NULL;

ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;
