-- Migration: 008_create_jwt_tokens
-- Create JWT tokens table for cached/refresh token storage

CREATE TABLE IF NOT EXISTS jwt_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    access_token_encrypted TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    expires_at INTEGER NOT NULL,
    scope TEXT,
    audience TEXT,
    issuer TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_jwt_tokens_user_id ON jwt_tokens(user_id);
