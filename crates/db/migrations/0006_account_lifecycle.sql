ALTER TABLE oauth_attempts ADD COLUMN IF NOT EXISTS user_id BIGINT REFERENCES users(id) ON DELETE CASCADE;
CREATE INDEX IF NOT EXISTS oauth_attempts_user_idx ON oauth_attempts(user_id);
ALTER TABLE legacy_import ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE legacy_import DROP CONSTRAINT IF EXISTS legacy_import_user_id_fkey;
ALTER TABLE legacy_import ADD CONSTRAINT legacy_import_user_id_fkey FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;
