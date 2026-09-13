ALTER TABLE user_sync_runs ADD COLUMN warnings TEXT[] NOT NULL DEFAULT '{}';
