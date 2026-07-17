---
feature: "02-provider-migrations"
spec: "58-foundation-auth-social-login"
depends: "01-provider-model"
status: "pending"
---

# Feature 02: Provider Migrations

## Goal

Add SQL migrations for upstream provider storage and user-provider linking.

## WHAT

### Migration 024: `upstream_providers`

```sql
CREATE TABLE IF NOT EXISTS upstream_providers (
    id TEXT PRIMARY KEY,                          -- slug: "google", "github", etc.
    name TEXT NOT NULL,                           -- display name
    provider_type TEXT NOT NULL DEFAULT 'oidc',   -- 'oidc' or 'oauth2'
    client_id TEXT NOT NULL,
    client_secret_ciphertext BLOB,                -- encrypted secret
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
```

### Migration 025: `user_provider_links`

```sql
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
CREATE INDEX IF NOT EXISTS idx_user_provider_links_provider_subject ON user_provider_links(provider_id, upstream_subject);
CREATE INDEX IF NOT EXISTS idx_user_provider_links_email ON user_provider_links(upstream_email);
```

## HOW

### Files to create

- `backends/foundation_db/src/core/schema/sql/024_create_upstream_providers.sql`
- `backends/foundation_db/src/core/schema/sql/025_create_user_provider_links.sql`

### Notes

- Migrations 006/007 are **left untouched** — they serve client-side outbound OAuth
- The `upstream_email` index on `user_provider_links` enables the auto-link-by-email lookup (D05)
- `upstream_subject` is the unique identifier from the provider (Google's `sub`, GitHub's numeric `id`)

---

_Created: 2026-07-17_
