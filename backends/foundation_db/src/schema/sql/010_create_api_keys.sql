-- Migration: 010_create_api_keys
-- Create API keys table for API authentication

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    key_hash TEXT UNIQUE NOT NULL,
    name TEXT,
    prefix TEXT,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permissions TEXT,  -- JSON array
    expires_at INTEGER,
    last_used_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    metadata TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);
