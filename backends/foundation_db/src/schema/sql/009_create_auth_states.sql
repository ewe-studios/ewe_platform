-- Migration: 009_create_auth_states
-- Create auth states table for per-provider state machine

CREATE TABLE IF NOT EXISTS auth_states (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    user_id TEXT REFERENCES users(id),
    state TEXT NOT NULL,  -- JSON serialized AuthState
    last_transition INTEGER NOT NULL,
    pending_requests TEXT,  -- JSON array of queued requests
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_auth_states_provider ON auth_states(provider_id);
CREATE INDEX IF NOT EXISTS idx_auth_states_user ON auth_states(user_id);
