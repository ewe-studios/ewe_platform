---
feature: "02-provider-migrations"
spec: "57-foundation-social-and-keychain"
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

## ✅ Status: COMPLETE (2026-07-18)

### What shipped

- `backends/foundation_db/src/core/schema/sql/024_create_upstream_providers.sql`
- `backends/foundation_db/src/core/schema/sql/025_create_user_provider_links.sql`
- Both registered in `foundation_db/src/core/schema/migrations.rs` (`MIGRATIONS` static)
- `client_secret_ciphertext` is `BLOB` storing `nonce || ciphertext` (see F001 ProviderCrypto)

### Tests (all green, real Turso DB — no mocks)

- `tests/schema_migrations.rs`: `test_migrations_defined` (count 22→24), `test_social_login_migrations_present`
- `tests/turso_storage_tests.rs::test_social_login_tables_functional`: real INSERT/SELECT round-trip on both tables, verifies the unique `(provider_id, upstream_subject)` index rejects duplicates, and the account-linking lookup returns the correct user.

### Learnings / insights

- **Migration 023 (vectors) is NOT in the `MIGRATIONS` registry** — it's registered/gated elsewhere. New migrations append after `022`, so we used `024`/`025` (the spec's chosen numbers) and they slot in after 022 without gaps in the registry. The numeric id is just a string label; ordering in the `MIGRATIONS` slice is what matters, not contiguity.
- **`init_schema()` on TursoStorage runs the whole `MIGRATIONS` set** via `MigrationRunner`; each migration is `IF NOT EXISTS` + tracked in `_migrations`, so re-running is idempotent. Integration tests just call `TursoStorage::new(url).init_schema()`.
- **`test_migrations_defined` hard-codes the count** — adding migrations requires bumping it (22→24). Left it as a count assertion (rather than removing) because it's a cheap guard against accidental migration loss.
- **Unique index on `(provider_id, upstream_subject)`** (not just a plain index as the original SQL sketch had) — this enforces D05's "one upstream identity → one local user" invariant at the DB level, verified by the duplicate-insert-rejected test.

_Created: 2026-07-17 · Completed: 2026-07-18_
