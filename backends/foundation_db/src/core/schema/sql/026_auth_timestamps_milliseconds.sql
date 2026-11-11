-- Migration: 026_auth_timestamps_milliseconds
-- Convert legacy whole-second timestamps in the auth tables to Unix
-- milliseconds, so every timestamp column uses one unit.
--
-- The documented unit for these columns is Unix milliseconds. Rows written by
-- older IdP builds or by hand used whole seconds (values under 10000000000).
-- A millisecond value for the current era is far above that threshold, so the
-- same guard makes the migration idempotent: a converted row is never matched
-- again.

-- sessions: expires_at, created_at, last_active_at, refreshed_at
UPDATE sessions
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

UPDATE sessions
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

UPDATE sessions
SET last_active_at = last_active_at * 1000
WHERE last_active_at > 0 AND last_active_at < 10000000000;

UPDATE sessions
SET refreshed_at = refreshed_at * 1000
WHERE refreshed_at > 0 AND refreshed_at < 10000000000;

-- verification_tokens: expires_at, created_at, consumed_at
UPDATE verification_tokens
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

UPDATE verification_tokens
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

UPDATE verification_tokens
SET consumed_at = consumed_at * 1000
WHERE consumed_at > 0 AND consumed_at < 10000000000;

-- oauth_states: created_at, expires_at
UPDATE oauth_states
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

UPDATE oauth_states
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

-- jwt_tokens: expires_at, created_at
UPDATE jwt_tokens
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

UPDATE jwt_tokens
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

-- email_otps: expires_at, created_at, consumed_at
UPDATE email_otps
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

UPDATE email_otps
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

UPDATE email_otps
SET consumed_at = consumed_at * 1000
WHERE consumed_at > 0 AND consumed_at < 10000000000;

-- magic_links: expires_at, created_at, consumed_at
UPDATE magic_links
SET expires_at = expires_at * 1000
WHERE expires_at > 0 AND expires_at < 10000000000;

UPDATE magic_links
SET created_at = created_at * 1000
WHERE created_at > 0 AND created_at < 10000000000;

UPDATE magic_links
SET consumed_at = consumed_at * 1000
WHERE consumed_at > 0 AND consumed_at < 10000000000;
