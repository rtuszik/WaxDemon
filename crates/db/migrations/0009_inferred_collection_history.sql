ALTER TABLE user_collection_history
    ADD COLUMN source TEXT NOT NULL DEFAULT 'observed'
        CHECK (source IN ('observed', 'inferred'));

ALTER TABLE user_collection_metadata
    ADD COLUMN history_backfilled_at TIMESTAMPTZ;
