-- Ported verbatim from src/lib/db/index.ts:39-91

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS collection_items (
    id INTEGER PRIMARY KEY,
    release_id INTEGER NOT NULL,
    artist TEXT,
    title TEXT,
    year INTEGER,
    format TEXT,
    genres TEXT,
    styles TEXT,
    cover_image_url TEXT,
    added_date TEXT NOT NULL,
    folder_id INTEGER,
    rating INTEGER,
    notes TEXT,
    condition TEXT,
    suggested_value REAL,
    last_value_check TEXT
);

CREATE INDEX IF NOT EXISTS idx_collection_items_release_id ON collection_items(release_id);
CREATE INDEX IF NOT EXISTS idx_collection_items_artist ON collection_items(artist);
CREATE INDEX IF NOT EXISTS idx_collection_items_year ON collection_items(year);
CREATE INDEX IF NOT EXISTS idx_collection_items_added_date ON collection_items(added_date);

CREATE TABLE IF NOT EXISTS collection_stats_history (
    timestamp TEXT PRIMARY KEY,
    total_items INTEGER NOT NULL,
    value_min REAL,
    value_mean REAL,
    value_max REAL
);
