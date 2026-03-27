-- Migration: 004_create_accounts
-- Create accounts table for linked OAuth provider accounts

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    provider_account_id TEXT NOT NULL,
    access_token TEXT,
    refresh_token TEXT,
    expires_at INTEGER,
    scope TEXT,
    token_type TEXT,
    id_token TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    UNIQUE(provider_id, provider_account_id)
);

CREATE INDEX IF NOT EXISTS idx_accounts_user_id ON accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_accounts_provider ON accounts(provider_id, provider_account_id);
