-- Migration: 025_create_user_provider_links
-- Links a local user to an upstream identity (Google sub, GitHub id, etc.).
-- A user can have multiple links (account linking); a given upstream identity
-- (provider_id + upstream_subject) maps to exactly one local user.

CREATE TABLE IF NOT EXISTS user_provider_links (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    provider_id TEXT NOT NULL REFERENCES upstream_providers(id),
    upstream_subject TEXT NOT NULL,               -- "sub" from OIDC or provider's user ID
    upstream_email TEXT,
    upstream_username TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    last_used_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_user_provider_links_user ON user_provider_links(user_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_provider_links_provider_subject ON user_provider_links(provider_id, upstream_subject);
CREATE INDEX IF NOT EXISTS idx_user_provider_links_email ON user_provider_links(upstream_email);
