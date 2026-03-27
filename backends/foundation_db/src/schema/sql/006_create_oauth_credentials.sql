-- Migration: 006_create_oauth_credentials
-- Create OAuth credentials table for provider configuration

CREATE TABLE IF NOT EXISTS oauth_credentials (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT,
    redirect_uri TEXT,
    scopes TEXT,  -- JSON array
    authorization_url TEXT,
    token_url TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    expires_at INTEGER
);
