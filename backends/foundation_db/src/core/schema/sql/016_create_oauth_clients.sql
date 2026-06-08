-- Migration: 016_create_oauth_clients
-- Create OAuth clients table for registered applications

CREATE TABLE IF NOT EXISTS oauth_clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    client_secret_hash TEXT NOT NULL,
    redirect_uris TEXT NOT NULL,  -- JSON array
    grant_types TEXT NOT NULL,    -- JSON array
    scopes TEXT NOT NULL,         -- JSON array
    is_public INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_oauth_clients_id ON oauth_clients(id);
