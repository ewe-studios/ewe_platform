-- Migration: 007_create_oauth_states
-- Create OAuth states table for PKCE flow and CSRF protection

CREATE TABLE IF NOT EXISTS oauth_states (
    id TEXT PRIMARY KEY,
    state_param TEXT UNIQUE NOT NULL,
    code_verifier TEXT NOT NULL,
    redirect_url TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    expires_at INTEGER NOT NULL,
    used INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_oauth_states_state ON oauth_states(state_param);
CREATE INDEX IF NOT EXISTS idx_oauth_states_expires ON oauth_states(expires_at);
