CREATE TABLE IF NOT EXISTS user_preferences (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    sync_interval_hours INTEGER NOT NULL DEFAULT 24 CHECK (sync_interval_hours BETWEEN 0 AND 720),
    price_refresh_hours INTEGER NOT NULL DEFAULT 24 CHECK (price_refresh_hours BETWEEN 1 AND 720),
    display_currency TEXT CHECK (display_currency ~ '^[A-Z]{3}$')
);

CREATE TABLE IF NOT EXISTS user_sync_runs (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled')),
    phase TEXT NOT NULL DEFAULT 'queued',
    processed INTEGER NOT NULL DEFAULT 0,
    total INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    queued_at TIMESTAMPTZ,
    job_id TEXT UNIQUE
);
CREATE UNIQUE INDEX IF NOT EXISTS user_sync_one_active_idx ON user_sync_runs(user_id) WHERE status IN ('queued', 'running');
CREATE INDEX IF NOT EXISTS user_sync_history_idx ON user_sync_runs(user_id, created_at DESC);

CREATE TABLE IF NOT EXISTS user_collection_metadata (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    folders JSONB NOT NULL,
    fields JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_price_cache (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    release_id BIGINT NOT NULL REFERENCES releases(id),
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, release_id)
);

ALTER TABLE user_collection_history ADD COLUMN IF NOT EXISTS raw_values JSONB;
