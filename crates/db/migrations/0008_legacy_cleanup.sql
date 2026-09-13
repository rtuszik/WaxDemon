SET LOCAL lock_timeout = '10s';

DO $$
BEGIN
    LOCK TABLE legacy_import IN EXCLUSIVE MODE;
    IF EXISTS (SELECT 1 FROM legacy_import) THEN
        DROP TABLE IF EXISTS collection_items, collection_stats_history, settings, user_settings;
    ELSIF to_regclass('collection_items') IS NOT NULL THEN
        LOCK TABLE collection_items, collection_stats_history, settings, user_settings IN ACCESS EXCLUSIVE MODE;
        IF NOT EXISTS (
            SELECT 1 FROM collection_items
            UNION ALL SELECT 1 FROM collection_stats_history
            UNION ALL SELECT 1 FROM settings
            UNION ALL SELECT 1 FROM user_settings
        ) THEN
            DROP TABLE collection_items, collection_stats_history, settings, user_settings;
        END IF;
    END IF;
END
$$;
