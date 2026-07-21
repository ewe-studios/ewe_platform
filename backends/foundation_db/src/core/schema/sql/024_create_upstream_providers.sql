-- Migration: 024_create_upstream_providers
-- Create upstream identity provider configuration table for social login.
-- Each row is a configured upstream OIDC/OAuth2 provider (Google, GitHub, etc.)
-- that the IdP brokers authentication through. Secrets are encrypted at rest.

CREATE TABLE IF NOT EXISTS upstream_providers (
    id TEXT PRIMARY KEY,                          -- slug: "google", "github", etc.
    name TEXT NOT NULL,                           -- display name
    provider_type TEXT NOT NULL DEFAULT 'oidc',   -- 'oidc' or 'oauth2'
    client_id TEXT NOT NULL,
    client_secret_ciphertext BLOB,                -- encrypted secret (nonce || ciphertext)
    encryption_key_id TEXT NOT NULL DEFAULT 'default',
    authorization_url TEXT,                       -- for non-OIDC providers
    token_url TEXT,
    userinfo_url TEXT,
    discovery_url TEXT,                           -- OIDC .well-known URL
    scopes TEXT NOT NULL DEFAULT '["openid","email","profile"]',  -- JSON array
    is_active INTEGER NOT NULL DEFAULT 1,
    mapping_config TEXT NOT NULL DEFAULT '{"email_field":"email","email_verified_field":"email_verified","name_field":"name","subject_field":"sub"}',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_upstream_providers_active ON upstream_providers(is_active);
