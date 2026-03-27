-- Migration: 014_create_rate_limits
-- Create rate limits table for database-backed rate limiting

CREATE TABLE IF NOT EXISTS rate_limits (
    id TEXT PRIMARY KEY,
    key TEXT UNIQUE NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    window_start INTEGER NOT NULL,
    window_end INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rate_limits_key ON rate_limits(key);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window ON rate_limits(window_end);
