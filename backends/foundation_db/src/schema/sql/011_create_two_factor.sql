-- Migration: 011_create_two_factor
-- Create two-factor authentication tables

CREATE TABLE IF NOT EXISTS two_factor_secrets (
    id TEXT PRIMARY KEY,
    user_id TEXT UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret TEXT NOT NULL,
    backup_codes TEXT,  -- Encrypted JSON
    enabled INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE TABLE IF NOT EXISTS two_factor_attempts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ip_address TEXT,
    attempted_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    success INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_two_factor_attempts_user ON two_factor_attempts(user_id);
CREATE INDEX IF NOT EXISTS idx_two_factor_attempts_time ON two_factor_attempts(attempted_at);
