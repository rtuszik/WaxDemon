CREATE TABLE IF NOT EXISTS app_sessions (
    id_hash BYTEA PRIMARY KEY CHECK (octet_length(id_hash) = 32),
    data JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS app_sessions_expiry_idx ON app_sessions(expires_at);

CREATE TABLE IF NOT EXISTS oauth_attempts (
    browser_hash BYTEA PRIMARY KEY CHECK (octet_length(browser_hash) = 32),
    token_hash BYTEA NOT NULL CHECK (octet_length(token_hash) = 32),
    key_id TEXT NOT NULL CHECK (key_id ~ '^[A-Za-z0-9_-]{1,64}$'),
    nonce BYTEA NOT NULL CHECK (octet_length(nonce) = 24),
    ciphertext BYTEA NOT NULL CHECK (octet_length(ciphertext) >= 16),
    expires_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS oauth_attempts_expiry_idx ON oauth_attempts(expires_at);
