CREATE TABLE IF NOT EXISTS users (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    discogs_id BIGINT NOT NULL UNIQUE CHECK (discogs_id > 0),
    username TEXT NOT NULL CHECK (length(btrim(username)) > 0),
    role TEXT NOT NULL DEFAULT 'user' CHECK (role IN ('user', 'admin')),
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'disabled')),
    session_revocation UUID NOT NULL DEFAULT gen_random_uuid(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS releases (
    id BIGINT PRIMARY KEY,
    artist TEXT,
    title TEXT,
    year INTEGER,
    format TEXT,
    genres TEXT,
    styles TEXT,
    cover_image_url TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS user_collection_items (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    instance_id BIGINT NOT NULL,
    release_id BIGINT NOT NULL REFERENCES releases(id),
    added_date TEXT NOT NULL,
    folder_id BIGINT,
    rating INTEGER,
    notes TEXT,
    condition TEXT,
    suggested_value NUMERIC,
    currency TEXT CHECK (currency ~ '^[A-Z]{3}$'),
    last_value_check TEXT,
    PRIMARY KEY (user_id, instance_id)
);
CREATE INDEX IF NOT EXISTS user_collection_items_release_idx ON user_collection_items(release_id);
CREATE INDEX IF NOT EXISTS user_collection_items_added_idx ON user_collection_items(user_id, added_date);

CREATE TABLE IF NOT EXISTS user_collection_history (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL,
    total_items INTEGER NOT NULL,
    value_min NUMERIC,
    value_median NUMERIC,
    value_max NUMERIC,
    currency TEXT CHECK (currency ~ '^[A-Z]{3}$'),
    PRIMARY KEY (user_id, timestamp)
);

CREATE TABLE IF NOT EXISTS user_settings (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    value TEXT,
    PRIMARY KEY (user_id, key)
);

CREATE TABLE IF NOT EXISTS user_price_suggestions (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    release_id BIGINT NOT NULL REFERENCES releases(id),
    currency TEXT NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
    condition TEXT NOT NULL,
    amount NUMERIC NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, release_id, currency, condition)
);

CREATE TABLE IF NOT EXISTS legacy_import (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    user_id BIGINT NOT NULL REFERENCES users(id),
    item_count BIGINT NOT NULL,
    release_count BIGINT NOT NULL,
    history_count BIGINT NOT NULL,
    setting_count BIGINT NOT NULL,
    imported_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
