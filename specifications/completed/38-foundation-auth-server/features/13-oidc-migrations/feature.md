---
feature: "OIDC Migrations"
description: "foundation_db migrations 016-019: oauth_clients, authorization_codes, refresh_tokens, device_codes"
status: "pending"
priority: "high"
depends_on: ["10-idp-models"]
estimated_effort: "medium"
created: 2026-06-05
last_updated: 2026-06-07
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 1
  total: 1
  completion_percentage: 0%
---

# Feature 13: OIDC Migrations

## Migration Grouping Design

Not all tables may be needed by every consumer. Instead of one monolithic list,
migrations are organized into **groups** that can be selectively applied:

```rust
// Migration groups are composable:
let migrations = Migrations::new()
    .add_group("auth", AUTH_MIGRATIONS)        // 001-015: users, sessions, etc.
    .add_group("oidc", OIDC_MIGRATIONS)         // 016-019: OAuth clients, codes, tokens
    .add_group("custom", CUSTOM_MIGRATIONS);    // user-defined

// Apply all groups:
migrations.migrate_all(&store)?;

// Apply specific groups:
migrations.migrate_groups(&["auth", "oidc"], &store)?;
```

This allows:
1. **All migrations** — for full self-hosted IdP deployments
2. **OIDC migrations only** — for client-only deployments that need OAuth tables
3. **Other groups** — as defined by consumers

If all groups are interconnected (shared FK references), applying `migrate_all()`
ensures they are applied in the correct order.

## Description

Add foundation_db migrations 016-019 for OIDC server tables: OAuth clients, authorization codes, refresh tokens, and device codes. These are added to the centralized migration system in `foundation_db`.

## Files Modified

- `backends/foundation_db/src/core/schema/migrations.rs` — add 4 new Migration entries
- `backends/foundation_db/src/core/schema/sql/016_create_oauth_clients.sql` — NEW
- `backends/foundation_db/src/core/schema/sql/017_create_authorization_codes.sql` — NEW
- `backends/foundation_db/src/core/schema/sql/018_create_refresh_tokens.sql` — NEW
- `backends/foundation_db/src/core/schema/sql/019_create_device_codes.sql` — NEW

## Migration 016: OAuth Clients

```sql
-- Migration: 016_create_oauth_clients
-- Create OAuth clients table for registered applications

CREATE TABLE IF NOT EXISTS oauth_clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    client_secret_hash TEXT NOT NULL,
    redirect_uris TEXT NOT NULL,  -- JSON array
    grant_types TEXT NOT NULL,    -- JSON array
    scopes TEXT NOT NULL,         -- JSON array
    is_public INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_oauth_clients_id ON oauth_clients(id);
```

## Migration 017: Authorization Codes

```sql
-- Migration: 017_create_authorization_codes
-- Create authorization codes table for OIDC auth code flow

CREATE TABLE IF NOT EXISTS authorization_codes (
    code TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT,
    scope TEXT NOT NULL,
    nonce TEXT,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_auth_codes_code ON authorization_codes(code);
CREATE INDEX IF NOT EXISTS idx_auth_codes_expires_at ON authorization_codes(expires_at);
CREATE INDEX IF NOT EXISTS idx_auth_codes_client_id ON authorization_codes(client_id);
```

## Migration 018: Refresh Tokens

```sql
-- Migration: 018_create_refresh_tokens
-- Create refresh tokens table for token rotation

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    token_hash TEXT UNIQUE NOT NULL,
    expires_at INTEGER NOT NULL,
    rotated_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires_at ON refresh_tokens(expires_at);
```

## Migration 019: Device Codes

```sql
-- Migration: 019_create_device_codes
-- Create device codes table for device authorization grant (RFC 8628)

CREATE TABLE IF NOT EXISTS device_codes (
    device_code TEXT PRIMARY KEY,
    user_code TEXT UNIQUE NOT NULL,
    client_id TEXT NOT NULL,
    scope TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    interval_seconds INTEGER NOT NULL DEFAULT 5,
    user_id TEXT,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);

CREATE INDEX IF NOT EXISTS idx_device_codes_device ON device_codes(device_code);
CREATE INDEX IF NOT EXISTS idx_device_codes_user_code ON device_codes(user_code);
CREATE INDEX IF NOT EXISTS idx_device_codes_expires_at ON device_codes(expires_at);
```

## Migration Runner Update

Add entries to `MIGRATIONS` array in `migrations.rs`:

```rust
pub static MIGRATIONS: &[Migration] = &[
    // ... existing 001-015 ...
    Migration {
        id: "016_create_oauth_clients",
        name: "Create OAuth clients table",
        sql: include_str!("sql/016_create_oauth_clients.sql"),
    },
    Migration {
        id: "017_create_authorization_codes",
        name: "Create authorization codes table",
        sql: include_str!("sql/017_create_authorization_codes.sql"),
    },
    Migration {
        id: "018_create_refresh_tokens",
        name: "Create refresh tokens table",
        sql: include_str!("sql/018_create_refresh_tokens.sql"),
    },
    Migration {
        id: "019_create_device_codes",
        name: "Create device codes table",
        sql: include_str!("sql/019_create_device_codes.sql"),
    },
];
```

## Testing

- Migration count updated to 19
- Migration IDs remain unique
- Each SQL file is valid SQLite syntax
- MigrationRunner applies all 4 new migrations cleanly on fresh database
- Idempotent: running again on existing database does nothing (all tables IF NOT EXISTS)
