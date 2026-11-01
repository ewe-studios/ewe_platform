-- Migration: 027_rebuild_rate_limits
-- Rebuild the rate_limits table with the documented shape.
--
-- Earlier deployments created the table with a different column set. Rate-limit
-- windows are disposable state (a rebuild simply loses in-flight windows), so
-- the migration drops any old table and recreates the documented one instead of
-- trying to reconcile shapes. The runtime also uses CREATE TABLE IF NOT EXISTS
-- with this same shape, so the table stays correct afterwards.

DROP TABLE IF EXISTS rate_limits;

CREATE TABLE rate_limits (
    id TEXT PRIMARY KEY,
    key TEXT UNIQUE NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    window_start INTEGER NOT NULL,
    window_end INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rate_limits_key ON rate_limits(key);
CREATE INDEX IF NOT EXISTS idx_rate_limits_window ON rate_limits(window_end);
