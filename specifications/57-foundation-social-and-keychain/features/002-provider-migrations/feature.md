# Feature 02: Provider Migrations

**Status:** Complete

**Depends on:** `01-provider-model`

## Summary

Adds two SQL migrations to the `foundation_db` schema: `upstream_providers` (024) for storing provider configurations and `user_provider_links` (025) for linking local users to upstream identities. Both are registered in the `MIGRATIONS` static and run via `MigrationRunner` during `init_schema()`.

## What was built

### Files created

- `backends/foundation_db/src/core/schema/sql/024_create_upstream_providers.sql`
- `backends/foundation_db/src/core/schema/sql/025_create_user_provider_links.sql`

### Files modified

- `backends/foundation_db/src/core/schema/migrations.rs` -- both migrations registered in the `MIGRATIONS` slice

### Schema details

**`upstream_providers` (024):**
- `id TEXT PRIMARY KEY` -- provider slug (e.g. "google", "github")
- `provider_type TEXT NOT NULL DEFAULT 'oidc'` -- `oidc` or `oauth2`
- `client_id TEXT NOT NULL`, `client_secret_ciphertext BLOB` -- encrypted secret
- `encryption_key_id TEXT NOT NULL DEFAULT 'default'`
- `authorization_url`, `token_url`, `userinfo_url`, `discovery_url` -- nullable for partial configs
- `scopes TEXT NOT NULL DEFAULT '["openid","email","profile"]'` -- JSON array
- `is_active INTEGER NOT NULL DEFAULT 1`
- `mapping_config TEXT NOT NULL DEFAULT '{"email_field":"email",...}'` -- claim mapping JSON
- `created_at`, `updated_at` -- millisecond timestamps

**`user_provider_links` (025):**
- `id TEXT PRIMARY KEY`
- `user_id TEXT NOT NULL REFERENCES users(id)`
- `provider_id TEXT NOT NULL REFERENCES upstream_providers(id)`
- `upstream_subject TEXT NOT NULL` -- stable ID from the provider
- `upstream_email`, `upstream_username`
- `created_at`, `last_used_at`
- **Unique index** on `(provider_id, upstream_subject)` -- enforces one-upstream-identity-to-one-local-user
- Indexes on `user_id` and `upstream_email` for lookup performance

## Tests

- `tests/schema_migrations.rs` -- **3 tests**: `test_migrations_defined` (count assertion, bumped 22 to 24), `test_social_login_migrations_present`
- `tests/turso_storage_tests.rs::test_social_login_tables_functional` -- real INSERT/SELECT round-trip on both tables, verifies unique constraint rejects duplicate `(provider_id, upstream_subject)`, verifies account-linking lookup returns correct user

All tests use real Turso/LibSQL database -- no mocks.

## Related decisions

- `decisions/00-identity-broker-pattern.md` -- storage schema for upstream providers and user linking

## Implementation notes

- **Migration 023 (vectors) is NOT in the `MIGRATIONS` registry** -- it is registered/gated elsewhere. New migrations append after 022, so 024/025 slot in without gaps in the registry. Numeric IDs are labels; ordering in the `MIGRATIONS` slice is what matters.
- **`init_schema()` on TursoStorage runs the whole `MIGRATIONS` set** via `MigrationRunner`. Each migration is `IF NOT EXISTS` and tracked in `_migrations`, so re-running is idempotent.
- **`test_migrations_defined` hard-codes the count** -- adding migrations requires bumping it. Left as a count assertion (a cheap guard against accidental migration loss).
- **Unique index on `(provider_id, upstream_subject)`** (not just a plain index) enforces the D05 "one upstream identity to one local user" invariant at the DB level.
- Migrations 006/007 (client-side outbound OAuth) are **left untouched** -- these serve a different purpose.
