---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/00a-foundation-db"
this_file: "specifications/07-foundation-ai/features/00a-foundation-db/feature.md"

feature: "Foundation DB - Unified Storage Backend"
description: "Create foundation_db crate providing unified storage abstraction with Turso sync backend, Cloudflare D1/R2, in-memory fallback — Valtron-only async, NO tokio"
status: in-progress
priority: high
depends_on:
  - "00-foundation"
  - "foundation_core"
estimated_effort: "medium"
created: 2026-03-20
last_updated: 2026-03-29
author: "Main Agent"

tasks:
  completed: 11
  uncompleted: 9
  total: 20
  completion_percentage: 55%
---

# Foundation DB - Unified Storage Backend

## Overview

`foundation_db` is a unified storage backend crate that provides a consistent abstraction layer for persisting data across multiple storage providers:

1. **Turso** - SQLite-compatible embedded/remote database with sync API and edge sync capabilities
  Source code: /home/darkvoid/Boxxed/@formulas/src.rust/src.turso/turso
  Documentation: https://github.com/tursodatabase/turso/blob/main/sdk-kit/README.md
2. **Cloudflare D1** - Edge SQLite for Cloudflare Workers
3. **Cloudflare R2** - Object storage for larger blobs
4. **In-Memory** - Ephemeral storage for development/testing
5. **JSON File** - Simple JSON-on-disk key-value store for lightweight persistence without SQL dependencies

This crate enables `foundation_auth` to persist credentials, OAuth states, tokens, and authentication state machines reliably across application restarts.

## Motivation

The authentication infrastructure (`foundation_auth`) requires persistent storage for:
- OAuth credentials (client_id, client_secret)
- JWT tokens with refresh tokens
- Session state and cookies
- Authentication state machine state
- Credential rotation history

Currently only in-memory storage exists, which loses all credentials on restart. `foundation_db` provides production-ready persistence with multiple backends.

## Goals

- Provide unified `StorageProvider` trait for all storage backends
- Support Turso for local/remote SQLite with sync API
- Support Cloudflare D1 for edge SQLite
- Support Cloudflare R2 for blob storage
- Provide in-memory backend for dev/test
- Enable automatic backend selection based on configuration
- Support encrypted storage for sensitive credentials
- Maintain async-first API using `foundation_core::valtron`
- Zeroize sensitive data on drop

## Iron Laws

These rules are **MANDATORY** and **NON-NEGOTIABLE**. Any implementation that violates them MUST be rejected.

### 1. No tokio, No async-trait

**`tokio` and `async-trait` are BANNED from `foundation_db` and `foundation_auth`.**

All asynchronous operations MUST use Valtron's `TaskIterator`/`StreamIterator` patterns from `foundation_core`. This applies to:
- All storage trait definitions (no `async fn`, no `#[async_trait]`)
- All backend implementations
- All tests (use `valtron::initialize_pool` + `execute()`, not `#[tokio::test]`)

Rationale: Valtron provides a unified executor framework that works across WASM (single-threaded) and native (multi-threaded) platforms. Mixing in tokio breaks this portability guarantee and creates two competing async runtimes.

### 2. Turso and libsql Backends

`foundation_db` supports both Turso (`https://crates.io/crates/turso`) and libsql (`https://crates.io/crates/libsql`) as SQL backends. Both provide SQLite-compatible storage with MVCC and concurrent writes.

**Important: Async API Handling**

Both Turso 0.1.x and libsql expose **async-only APIs**. To maintain compatibility with the Valtron-only async pattern (Iron Law 3), we wrap their async operations using Valtron's `from_future` + `execute` pattern:

```rust
use foundation_core::valtron::{from_future, execute};

// Pattern: Wrap async backend call with from_future and execute via Valtron
let mut task = from_future(async {
    conn.execute("SELECT ...", params).await
});
let mut stream = execute(task, None)?;
let result = stream.next().and_then(|s| match s {
    foundation_core::valtron::Stream::Next(v) => Some(v),
    _ => None,
}).ok_or(StorageError::Generic("No result".into()))?;
```

**Why `from_future` + `execute`:**
- `from_future` wraps any Future into a Valtron `TaskIterator`
- `execute` runs the task through Valtron's executor (multi-threaded or single-threaded based on platform)
- Returns a `DrivenStreamIterator` that yields `Stream::Next(result)` when complete
- This integrates with Valtron's full ecosystem: combinators, merging, parallel execution
- `valtron::multi::block_on` is ONLY for bootstrapping the Valtron executor at application entry points (`main()`)

**Implementation requirements:**
1. Storage trait methods are synchronous (`fn` not `async fn`)
2. Internal implementation wraps async calls with `from_future` + `execute`
3. Multi-value operations return `StorageResult<StorageItemStream<'_, T>>` with lazy iterators
4. All async wrapping happens **inside** the backend — the public API is sync
5. Pool initialization happens at application entry point via `valtron::initialize_pool()`

**Detailed patterns and lessons:** See `features/00a-foundation-db/LEARNINGS.md` for the full `exec_future` helper, `!Send` row iterator constraints, multi-value `StorageItemStream` patterns, and three-level error handling.

**Why both Turso and libsql:**
- **Turso** - Edge sync capabilities, newer codebase, actively developed
- **libsql** - More mature, stable API, wider adoption
- Both support the same core SQLite protocol
- Feature flags allow users to choose: `turso` or `libsql`

### 3. Valtron-Only Async Pattern

Storage traits return `StorageResult<T>` for single-value ops and `StorageResult<StorageItemStream<T>>` (Valtron `Stream`-based iterator) for multi-value ops:
- Single-value: `get`, `set`, `delete`, `exists`, `execute`, `execute_batch` → `StorageResult<T>`
- Multi-value: `query`, `list_keys`, `get_blob` → `StorageResult<StorageItemStream<'_, T>>`
- No `async fn`, no `.await`, no `Future` — only Valtron patterns

### 4. Zero Warnings, Zero Suppression

**All clippy, doc, and cargo warnings MUST be fixed, NEVER suppressed.**

- `cargo clippy --package foundation_db -- -D warnings` MUST pass with zero warnings
- `cargo clippy --package foundation_auth -- -D warnings` MUST pass with zero warnings
- `cargo doc --package foundation_db --no-deps` MUST produce zero warnings
- `cargo doc --package foundation_auth --no-deps` MUST produce zero warnings
- **NO `#[allow(...)]` attributes** — every warning is a signal; fix the code, don't silence it
- **NO `#![allow(...)]` crate-level suppression** — remove all existing suppression blocks
- This means: all public items have documentation, all match arms are covered, no dead code, no unused imports, no missing error docs, no clippy pedantic bypasses
- The existing `#![allow(clippy::..., dead_code, unused, deprecated)]` blocks in `lib.rs` files MUST be removed and the underlying issues fixed

### 5. Error Handling: `derive_more::From` + Manual `Display`, Central `errors.rs`

**All error types follow the project-wide error convention. No `thiserror`.**

The project defines custom errors in a central `errors.rs` file at the root of each module using `derive_more::From` for automatic `From` conversions and manual `impl Display` + `impl Error`. This is the established pattern across `foundation_core`, `foundation_auth`, and all other crates.

**The pattern:**
```rust
use derive_more::From;

/// WHY: Centralizes all storage error variants for consistent handling.
///
/// WHAT: Enum of all error conditions that can occur in storage operations.
///
/// HOW: Uses `derive_more::From` for automatic `From<T>` impls on nested
/// error variants. String-wrapping variants use `#[from(ignore)]` since
/// multiple String fields would conflict. Display is implemented manually
/// for human-readable messages.
#[derive(From, Debug)]
pub enum StorageError {
    /// Backend-specific error.
    #[from(ignore)]
    Backend(String),

    /// Connection failed.
    #[from(ignore)]
    Connection(String),

    // ... other #[from(ignore)] String variants ...

    /// I/O error during filesystem operations.
    Io(std::io::Error),                    // ← auto From<std::io::Error>

    /// JSON serialization/deserialization error.
    Json(serde_json::Error),               // ← auto From<serde_json::Error>

    /// Turso error.
    Turso(turso::Error),                   // ← auto From<turso::Error>
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backend(s) => write!(f, "Backend error: {s}"),
            Self::Connection(s) => write!(f, "Connection failed: {s}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Turso(e) => write!(f, "Turso error: {e}"),
            // ...
        }
    }
}

impl std::error::Error for StorageError {}

pub type StorageResult<T> = Result<T, StorageError>;
```

**Rules:**
- **`#[derive(From, Debug)]`** on all error enums — `derive_more::From` generates `From<T>` for each variant with a single non-String field
- **`#[from(ignore)]`** on all `String`-wrapping variants — required because multiple `(String)` variants would conflict
- **`#[from]`** explicit attribute on nested error variants when disambiguation is needed
- **Manual `impl Display`** — NOT `derive_more::Display` / `#[display("...")]`. This project uses manual Display for error messages.
- **Manual `impl std::error::Error`** — simple empty impl (no `source()` override needed; `derive_more::From` handles conversions)
- **`pub type FooResult<T> = Result<T, FooError>;`** — type alias for ergonomic Result usage
- **NO `thiserror`** — the project uses `derive_more::From` + manual Display exclusively
- **Central `errors.rs`** — one file per crate at `src/errors.rs`, all error types defined there
- **Feature-gated variants** — backend-specific error variants gated behind their feature flag

## Dependencies

**Required Crates:**
- `foundation_core` - For `valtron` async patterns, `ConfidentialText`
- `turso` - Turso sync backend
- `zeroize` - For secure memory clearing
- `serde` + `serde_json` - For serialization
- `derive_more` (features: `from`, `error`, `display`) - For `#[derive(From)]` on error types

**BANNED Crates (in foundation_db and foundation_auth):**
- `tokio` - Use `foundation_core::valtron` instead
- `async-trait` - Use `TaskIterator`/`StreamIterator` patterns instead
- `thiserror` - Use `derive_more::From` + manual `Display` per Iron Law 5

**Required By:**
- `foundation_auth` - Credential and state persistence
- `foundation_ai` - Token caching, usage tracking
- Any crate requiring persistent secure storage

## Requirements

### Project and Stage Namespacing (CRITICAL)

**All state stores MUST namespace state by both project AND stage (environment).** This is a fundamental requirement for multi-project deployments and stage isolation.

#### Why Namespacing Matters

Without project and stage namespacing, the following scenarios cause state corruption:

1. **Shared provider account**: Two different projects deploy to the same Cloudflare account, GCP project, or AWS account. Without project namespacing, they share state tables/buckets and overwrite each other's resources.

2. **Stage collisions**: Different stages (dev, staging, prod) within the same project would mix state if not properly isolated. A deployment to `prod` could overwrite `dev` state.

3. **CI/CD collisions**: Build agents deploying multiple projects to shared infrastructure may corrupt state without proper isolation.

4. **Team workflows**: Multiple teams sharing the same backend infrastructure (e.g., same Turso database, same R2 bucket) need guaranteed isolation between their projects.

The fix: Always include `{project}` AND `{stage}` in the namespace. This ensures:
- **Project isolation**: Multiple projects can share the same backend account without state mixing
- **Stage isolation**: dev/staging/prod environments remain separate within the same project
- **Safe multi-project deployments**: Teams can deploy multiple independent projects to shared infrastructure

#### Namespacing Strategy by Backend Type

| Backend Type | Backends | Namespacing Mechanism | Example |
|--------------|----------|----------------------|---------|
| **SQL-based** | SqliteStateStore, LibSQLStateStore, TursoStateStore, D1StateStore | Table prefix: `{project}_{stage}_resources` | `myapp_prod_resources`, `myapp_dev_resources` |
| **Object/File** | FileStateStore, R2StateStore | Path prefix: `{project}/{stage}/{resource_id}.json` | `myapp/prod/worker-1.json`, `myapp/dev/worker-1.json` |

#### Constructor Requirements

All state store constructors MUST accept `project` and `stage` parameters:

```rust
// SQL-based stores (table namespacing)
pub fn new(db_path: &Path, project: &str, stage: &str) -> Result<Self, StorageError>;

// Object/file stores (path namespacing)
pub fn new(project_dir: &Path, project: &str, stage: &str) -> Self;
```

The `create_state_store()` factory passes these parameters through:

```rust
pub fn create_state_store(
    project: &str,
    project_dir: &Path,
    provider: &str,
    stage: &str,
) -> Result<Box<dyn StateStore>, StorageError>
```

#### Implementation Details

**SQL-based stores (Sqlite, LibSQL, Turso, D1):**
- Table name: `{project}_{stage}_resources`
- Hyphens and spaces in project/stage are converted to underscores
- Example: `my-app` + `prod` → `my_app_prod_resources`
- Same schema for all tables (id, kind, provider, status, environment, config_hash, output, config_snapshot, created_at, updated_at)

**Object/file stores (File, R2):**
- Object key: `{project}/{stage}/{resource_id}.json`
- Resource IDs with slashes are escaped (e.g., `/` → `:`)
- Example: `my-app` + `prod` + `worker-1` → `my-app/prod/worker-1.json`

### Core Storage Abstraction

1. **StorageProvider Trait** - Unified interface for all storage backends
2. **StorageBackend Enum** - Runtime backend selection (Turso, D1, R2, Memory)
3. **KeyValueStore Trait** - Key-value operations across all backends
4. **BlobStore Trait** - Binary large object storage (R2-specific)
5. **QueryStore Trait** - SQL query capabilities (Turso/D1-specific)

### Turso Backend

6. **TursoStorage Struct** - Turso implementation with sync API
7. **Connection Pool** - Efficient connection management
8. **Migration System** - Schema versioning and migrations with automatic cleanup tasks
9. **Sync Support** - Turso edge sync capabilities
10. **Local Database** - Embedded SQLite mode
11. **Rate Limiting Backend** - Database-backed rate limiting with configurable windows

### Cloudflare D1 Backend

11. **D1Storage Struct** - Cloudflare D1 implementation
12. **Worker Integration** - Cloudflare Workers compatibility
13. **Edge Caching** - D1 edge caching support

### Cloudflare R2 Backend

14. **R2Storage Struct** - R2 object storage implementation
15. **Bucket Management** - Create/list/delete buckets
16. **Object Operations** - Put/get/delete objects
17. **Multipart Upload** - Large file support

### In-Memory Backend

18. **MemoryStorage Struct** - Ephemeral in-memory storage
19. **Zeroizing Storage** - Secure memory for sensitive data
20. **Drop Cleanup** - Automatic cleanup on drop

### JSON File Backend

21. **JsonFileStorage Struct** - JSON-on-disk key-value persistence
22. **Atomic Writes** - Write to temp file + rename for crash safety
23. **Project/Stage Namespacing** - Paths use `{project}/{stage}/{resource_id}.json` pattern
24. **Lazy Loading** - Read from disk on access, not on construction
25. **Zeroizing on Drop** - Clear in-memory cache of sensitive data

### Security Features

21. **Encrypted Storage** - Optional encryption at rest (ChaCha20-Poly1305)
22. **Secure Deletion** - Zeroize on delete/drop
23. **Access Control** - Optional per-key access control
24. **Audit Logging** - Optional operation logging
25. **Secret Rotation Support** - Multi-key encryption for credential rotation

## Architecture

### Storage Abstraction with Project/Stage Namespacing

```mermaid
graph TD
    subgraph Application["Application Layer"]
        A[foundation_auth]
        B[foundation_ai]
    end

    subgraph FoundationDB["foundation_db"]
        C[StorageProvider Trait]
        D[StorageBackend Enum]
        E[KeyValueStore Trait]
        F[BlobStore Trait]
        G[QueryStore Trait]
        H[RateLimiter Trait]
        N[NamespaceResolver<br/>project + stage → table/path]
    end

    subgraph Backends
        I[TursoStorage<br/>{project}_{stage}_resources]
        J[D1Storage<br/>{project}_{stage}_resources]
        K[R2Storage<br/>{project}/{stage}/{id}.json]
        L[MemoryStorage<br/>in-memory, namespaced]
        M[JsonFileStorage<br/>{project}/{stage}/{id}.json]
    end

    A --> C
    B --> C
    C --> D
    D --> N
    N --> I
    N --> J
    N --> K
    N --> L
    N --> M
    I --> E
    I --> G
    I --> H
    J --> E
    J --> G
    J --> H
    K --> F
    L --> E
    M --> E
```

**NamespaceResolver:** Computes the correct table name or object path based on `project` and `stage` parameters. This ensures complete isolation between projects and stages sharing the same backend infrastructure.

### Session Management Architecture

Inspired by better-auth's three-cookie system, `foundation_db` supports:

**Three-Cookie Session Pattern:**

1. **`session_token`** - Signed cookie with session token
   - MaxAge: 7 days (configurable)
   - HttpOnly, Secure, SameSite=lax
   - Contains: signed session identifier for DB lookup

2. **`session_data`** - Cached encrypted session data (reduces DB calls)
   - MaxAge: 5 minutes (configurable)
   - Three encoding strategies:
     - `compact` - Base64 + HMAC (smallest, custom format)
     - `jwt` - JWT with HS256 signature (standard, larger)
     - `jwe` - Encrypted JWT with A256CBC-HS512 (most secure, largest)
   - Supports chunking for large sessions

3. **`dont_remember`** - Flag for non-persistent sessions
   - Session-only cookie (no MaxAge)
   - Session expires when browser closes

**Token Format Support:**
- Compact tokens (custom Base64 + HMAC)
- JWT (RFC 7519) with configurable claims
- PASETO V4 Local (encrypted, opinionated)

**Secret Rotation:**
- Multi-key signer pattern for seamless rotation
- Current version + legacy keys for backward compatibility
- Tokens signed with current key, verified with any valid key

### Core Authentication Schema (Turso/D1)

Inspired by better-auth's proven schema design, the complete schema includes:

#### Core Tables

```sql
-- Users table (core identity)
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    username TEXT UNIQUE,
    password_hash TEXT,
    email_verified INTEGER DEFAULT 0,
    email_verified_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    metadata TEXT,  -- JSON for extensibility

    -- Account lockout (rate limiting per user)
    failed_login_attempts INTEGER DEFAULT 0,
    locked_until INTEGER,

    -- Soft delete support
    deleted_at INTEGER
);

CREATE INDEX idx_users_email ON users(email);
CREATE INDEX idx_users_username ON users(username);

-- Sessions table
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token TEXT UNIQUE NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    ip_address TEXT,
    user_agent TEXT,
    metadata TEXT,

    -- Session tracking for anomaly detection
    last_active_at INTEGER,
    refreshed_at INTEGER
);

CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_token ON sessions(token);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- OAuth accounts (linked provider accounts)
CREATE TABLE accounts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL,
    provider_account_id TEXT NOT NULL,
    access_token TEXT,
    refresh_token TEXT,
    expires_at INTEGER,
    scope TEXT,
    token_type TEXT,
    id_token TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(provider_id, provider_account_id)
);

CREATE INDEX idx_accounts_user_id ON accounts(user_id);
CREATE INDEX idx_accounts_provider ON accounts(provider_id, provider_account_id);

-- Verification tokens (unified: email OTP, magic links, password reset)
CREATE TABLE verification_tokens (
    id TEXT PRIMARY KEY,
    identifier TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    type TEXT NOT NULL,  -- 'email_otp', 'magic_link', 'password_reset'
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    consumed_at INTEGER,
    metadata TEXT
);

CREATE INDEX idx_verification_tokens_token ON verification_tokens(token);
CREATE INDEX idx_verification_tokens_identifier ON verification_tokens(identifier);
CREATE INDEX idx_verification_tokens_type ON verification_tokens(type);
```

#### Auth Infrastructure Tables

```sql
-- OAuth credentials (provider configuration)
CREATE TABLE oauth_credentials (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT,
    redirect_uri TEXT,
    scopes TEXT,  -- JSON array
    authorization_url TEXT,
    token_url TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    expires_at INTEGER
);

-- OAuth state (PKCE flow, CSRF protection)
CREATE TABLE oauth_states (
    id TEXT PRIMARY KEY,
    state_param TEXT UNIQUE NOT NULL,
    code_verifier TEXT NOT NULL,
    redirect_url TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_oauth_states_state ON oauth_states(state_param);
CREATE INDEX idx_oauth_states_expires ON oauth_states(expires_at);

-- JWT tokens (cached/refresh token storage)
CREATE TABLE jwt_tokens (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    access_token_encrypted TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    expires_at INTEGER NOT NULL,
    scope TEXT,
    audience TEXT,
    issuer TEXT,
    created_at INTEGER NOT NULL
);

-- Auth state machine (per-provider state)
CREATE TABLE auth_states (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    user_id TEXT REFERENCES users(id),
    state TEXT NOT NULL,  -- JSON serialized AuthState
    last_transition INTEGER NOT NULL,
    pending_requests TEXT,  -- JSON array of queued requests
    updated_at INTEGER NOT NULL
);
```

#### Plugin Tables (Extensibility)

```sql
-- API keys (for API authentication)
CREATE TABLE api_keys (
    id TEXT PRIMARY KEY,
    key_hash TEXT UNIQUE NOT NULL,
    name TEXT,
    prefix TEXT,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permissions TEXT,  -- JSON array
    expires_at INTEGER,
    last_used_at INTEGER,
    created_at INTEGER NOT NULL,
    metadata TEXT
);

CREATE INDEX idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);

-- Two-factor secrets
CREATE TABLE two_factor_secrets (
    id TEXT PRIMARY KEY,
    user_id TEXT UNIQUE NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret TEXT NOT NULL,
    backup_codes TEXT,  -- Encrypted JSON
    enabled INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Two-factor attempts (rate limiting)
CREATE TABLE two_factor_attempts (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ip_address TEXT,
    attempted_at INTEGER NOT NULL,
    success INTEGER NOT NULL
);

CREATE INDEX idx_two_factor_attempts_user ON two_factor_attempts(user_id);
CREATE INDEX idx_two_factor_attempts_time ON two_factor_attempts(attempted_at);

-- Email OTPs (separate tracking)
CREATE TABLE email_otps (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    otp TEXT NOT NULL,
    type TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    consumed_at INTEGER,
    attempts INTEGER DEFAULT 0
);

CREATE INDEX idx_email_otps_email ON email_otps(email);
CREATE INDEX idx_email_otps_expires ON email_otps(expires_at);

-- Magic links
CREATE TABLE magic_links (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    token TEXT UNIQUE NOT NULL,
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    consumed_at INTEGER,
    ip_address TEXT,
    user_agent TEXT
);

CREATE INDEX idx_magic_links_email ON magic_links(email);
CREATE INDEX idx_magic_links_token ON magic_links(token);

-- Rate limits (database-backed)
CREATE TABLE rate_limits (
    id TEXT PRIMARY KEY,
    key TEXT UNIQUE NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    window_start INTEGER NOT NULL,
    window_end INTEGER NOT NULL
);

CREATE INDEX idx_rate_limits_key ON rate_limits(key);
CREATE INDEX idx_rate_limits_window ON rate_limits(window_end);

-- Audit logs
CREATE TABLE audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id),
    action TEXT NOT NULL,
    resource_type TEXT,
    resource_id TEXT,
    changes TEXT,  -- JSON
    ip_address TEXT,
    user_agent TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_audit_logs_user ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);
CREATE INDEX idx_audit_logs_created ON audit_logs(created_at);
CREATE INDEX idx_audit_logs_resource ON audit_logs(resource_type, resource_id);

-- Migration tracking
CREATE TABLE _migrations (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now') * 1000)
);
```

#### Cleanup Queries (Maintenance)

```sql
-- Delete expired sessions
DELETE FROM sessions WHERE expires_at < (strftime('%s', 'now') * 1000);

-- Delete expired verification tokens
DELETE FROM verification_tokens WHERE expires_at < (strftime('%s', 'now') * 1000);

-- Delete expired OAuth states
DELETE FROM oauth_states WHERE expires_at < (strftime('%s', 'now') * 1000);

-- Delete expired email OTPs
DELETE FROM email_otps WHERE expires_at < (strftime('%s', 'now') * 1000);

-- Delete expired magic links
DELETE FROM magic_links WHERE expires_at < (strftime('%s', 'now') * 1000);

-- Reset rate limits
DELETE FROM rate_limits WHERE window_end < (strftime('%s', 'now') * 1000);

-- Delete old audit logs (keep last 90 days)
DELETE FROM audit_logs WHERE created_at < ((strftime('%s', 'now') * 1000) - (90 * 24 * 60 * 60 * 1000));
```

## Implementation

### Files to Create

**Note on Namespacing:** All state store backends implement project/stage namespacing:
- SQL backends (Turso, LibSQL, Sqlite, D1): Table names use `{project}_{stage}_resources` pattern
- Object/file backends (R2, File): Paths use `{project}/{stage}/{resource_id}.json` pattern

```
backends/foundation_db/
├── Cargo.toml
├── src/
│   ├── lib.rs                     - Module declarations, re-exports
│   ├── storage_provider.rs        - Core trait definitions (Valtron-based, NO async-trait)
│   ├── backends/
│   │   ├── mod.rs                 - Backend module exports
│   │   ├── turso_backend.rs       - Turso crate backend (async wrapped with Valtron)
│   │   │                          - Table: {project}_{stage}_resources
│   │   ├── libsql_backend.rs      - libsql crate backend (async wrapped with Valtron)
│   │   │                          - Table: {project}_{stage}_resources
│   │   ├── json_file.rs           - JSON-on-disk file backend (always available)
│   │   │                          - Path: {project}/{stage}/{resource_id}.json
│   │   ├── d1.rs                  - Cloudflare D1 implementation
│   │   │                          - Table: {project}_{stage}_resources
│   │   ├── r2.rs                  - Cloudflare R2 implementation
│   │   │                          - Key: {project}/{stage}/{resource_id}.json
│   │   └── memory.rs              - In-memory implementation (always available)
│   ├── schema/
│   │   ├── mod.rs                 - Schema definitions
│   │   └── migrations.rs          - Migration system
│   ├── crypto/
│   │   ├── mod.rs                 - Encryption utilities
│   │   └── zeroize.rs             - Secure deletion helpers
│   └── errors.rs                  - Error types
└── tests/
    ├── foundation_db_tests.rs     - Test entry point
    └── units/
        ├── foundation_db_memory_tests.rs
        ├── foundation_db_json_file_tests.rs
        └── foundation_db_turso_tests.rs
```

### Cargo.toml

```toml
[package]
name = "foundation_db"
version = "0.0.1"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
foundation_core = { workspace = true }

# Storage backends (choose one or both via features)
turso = { version = "0.1", optional = true }
libsql = { version = "0.3", optional = true }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling (derive_more::From + manual Display, NO thiserror)
derive_more = { version = "2.0", features = ["from", "error", "display"] }

# Security
zeroize = { version = "1", features = ["derive"] }
chacha20poly1305 = "0.10"
rand = "0.8"
hex = "0.4"
base64 = "0.22"

# Utilities
tracing = "0.1"
futures-lite = "2.6"  # For block_on when wrapping async backends

[features]
default = ["turso"]  # Turso is default, can switch to libsql
turso = ["dep:turso"]
libsql = ["dep:libsql"]
d1 = []
r2 = []

# NOTE: tokio and async-trait are BANNED
# All async operations use foundation_core::valtron
# Backend async APIs (turso/libsql) are wrapped with Valtron's block_on or from_future
```

## Detailed Change Plan (File-by-File)

This section documents every change needed, at the function/struct level, with before/after patterns.

### Design Principle: Result for Operations, Stream for Multi-Value Output

**Two return patterns based on the nature of the operation:**

1. **Single-value operations** → `StorageResult<T>` directly.
   The `Result` wraps the operation (did it connect? did it parse? did the write succeed?). The `Ok(T)` is the value.
   Examples: `get()`, `set()`, `delete()`, `exists()`, `execute()`, `execute_batch()`

2. **Multi-value / streamed operations** → `StorageResult<impl Iterator<Item = Stream<T, P>>>`.
   The `Result` wraps the operation setup (did the query prepare? did the connection open?). The `Ok` contains a **lazy Valtron Stream iterator** that yields values one at a time without allocating everything upfront.
   Examples: `query()` (row-by-row), `list_keys()` (key-by-key), `get_blob()` (chunk-by-chunk)

**Why Stream and not just Iterator?**
The `Stream<D, P>` type from `foundation_core::valtron` carries async semantics:
```rust
pub enum Stream<D, P> {
    Init,              // Stream initializing
    Ignore,            // Skip this item
    Delayed(Duration), // Delay before next
    Pending(P),        // Still working, here's progress context
    Next(D),           // Here's a value
}
```
This means streamed results are **fully Valtron-native** — consumers can compose them with Valtron combinators, feed them into `execute()`, or iterate directly. For in-memory backends, the iterator just yields `Stream::Next(value)` for each item. For network-backed or async-capable backends, it can yield `Stream::Pending(...)` between items.

This also supports streaming bytes (`Stream<Vec<u8>, _>` or `Stream<Bytes, _>` for zero-copy) — consumers can merge, transform, or forward chunks without holding the entire blob in memory.

**Type aliases (using `Stream` from `foundation_core::valtron`):**
```rust
use foundation_core::valtron::Stream;

/// A Valtron stream of values from a storage operation.
/// D = the item type, P = pending/progress type (unit () if no progress needed).
pub type StorageItemStream<'a, T> = Box<dyn Iterator<Item = Stream<T, ()>> + 'a>;
```

**NOTE:** `Stream<D, P>` is imported from `foundation_core::valtron` — do NOT redefine it. Use `()` for the `P` (pending) type parameter unless the backend has meaningful progress to report. If progress is needed later, a crate-level progress enum can be added as the `P` parameter.

### Crate-Owned Types for QueryStore

**Problem:** Current `QueryStore` leaks backend-specific types into public API. This couples all consumers to backend internals.

**Solution:** Define crate-owned types:

```rust
/// A SQL parameter value (crate-owned, backend-agnostic).
#[derive(Debug, Clone)]
pub enum DataValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

/// A single row from a SQL query result.
#[derive(Debug, Clone)]
pub struct DataRow {
    columns: Vec<(String, DataValue)>,
}

impl DataRow {
    pub fn get<T: FromDataValue>(&self, index: usize) -> StorageResult<T>;
    pub fn get_by_name<T: FromDataValue>(&self, name: &str) -> StorageResult<T>;
    pub fn column_count(&self) -> usize;
}

/// Trait for extracting typed values from DataValue.
pub trait FromDataValue: Sized {
    fn from_sql(value: &DataValue) -> StorageResult<Self>;
}

```

Backend implementations convert from `turso::Value`/`turso::Row` into these owned types internally via private iterator wrappers.

### Return Type Rules

**`StorageResult<T>`** for single-value operations:
- `get()`, `set()`, `delete()`, `exists()`, `execute()`, `execute_batch()`
- These complete in one shot. The `Result` is the entire answer.

**`StorageResult<StorageItemStream<'_, T>>`** for multi-value / streamed operations:
- `query()` → stream of `DataRow` (row-by-row, lazy)
- `list_keys()` → stream of `String` (key-by-key, lazy)
- `get_blob()` → stream of `Vec<u8>` chunks (for zero-copy-friendly blob reads)

The `StorageItemStream` is `Box<dyn Iterator<Item = Stream<T, ()>>>` — each item is a Valtron `Stream::Next(value)`. This means consumers can compose with Valtron combinators, merge streams, or iterate directly.

**Where Vec is still correct:**
- `Vec<u8>` for individual blob chunks — contiguous bytes
- `SqlRow.columns: Vec<(String, DataValue)>` — small, bounded, random-access needed
- `OAuthTokenRequest.form_params: Vec<(String, String)>` — small, bounded, consumed whole

---

### File: `Cargo.toml`

**Current state:** Has `tokio`, `async-trait`, `turso` as direct deps.

**Changes:**
```
REMOVE: tokio = { version = "1", features = ["rt", "sync", "time"] }
REMOVE: async-trait = "0.1"
REMOVE: thiserror = "2.0"  (BANNED — use derive_more::From + manual Display)
KEEP: turso = "0.1"
ADD:    turso = "0.1"

REMOVE from [dev-dependencies]:
  tokio = { version = "1", features = ["rt", "sync", "time", "macros", "test-util"] }

ADD to [dev-dependencies]:
  tracing-test = { version = "0.2", features = ["no-env-filter"] }
  tempfile = "3"

CHANGE [features]:
  default = []
  d1 = []
  r2 = []
```

---

### File: `src/errors.rs`

**Current state:** Uses `derive_more::Display` with `#[display("...")]` attributes and manual `From` impls. Has manual `impl Error` with `source()`. This is **wrong** — the project convention is `derive_more::From` + manual `Display` (see Iron Law 5).

**Changes — full rewrite to match project convention:**

```rust
use derive_more::From;

/// WHY: Centralizes all storage error variants for consistent handling
/// across all foundation_db backends.
///
/// WHAT: Enum of all error conditions in storage operations — connection,
/// serialization, encryption, I/O, backend-specific, and SQL conversion.
///
/// HOW: `derive_more::From` auto-generates `From<T>` for nested error
/// variants (Io, Json, Base64, Hex, Libsql, Turso). String-wrapping
/// variants use `#[from(ignore)]` to avoid conflicting From impls.
#[derive(From, Debug)]
pub enum StorageError {
    /// Backend-specific error.
    #[from(ignore)]
    Backend(String),

    /// Connection failed.
    #[from(ignore)]
    Connection(String),

    /// Key not found.
    #[from(ignore)]
    NotFound(String),

    /// Serialization error.
    #[from(ignore)]
    Serialization(String),

    /// Encryption error.
    #[from(ignore)]
    Encryption(String),

    /// Migration error.
    #[from(ignore)]
    Migration(String),

    /// SQL value conversion error.
    #[from(ignore)]
    SqlConversion(String),

    /// Generic storage error.
    #[from(ignore)]
    Generic(String),

    /// I/O error during filesystem operations.
    Io(std::io::Error),

    /// JSON serialization/deserialization error.
    Json(serde_json::Error),

    /// Base64 decoding error.
    Base64(base64::DecodeError),

    /// Hex decoding error.
    Hex(hex::FromHexError),

    /// Turso error.
    Turso(turso::Error),
}

impl core::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Backend(s) => write!(f, "Backend error: {s}"),
            Self::Connection(s) => write!(f, "Connection failed: {s}"),
            Self::NotFound(s) => write!(f, "Key not found: {s}"),
            Self::Serialization(s) => write!(f, "Serialization error: {s}"),
            Self::Encryption(s) => write!(f, "Encryption error: {s}"),
            Self::Migration(s) => write!(f, "Migration error: {s}"),
            Self::SqlConversion(s) => write!(f, "SQL conversion error: {s}"),
            Self::Generic(s) => write!(f, "Storage error: {s}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::Base64(e) => write!(f, "Base64 error: {e}"),
            Self::Hex(e) => write!(f, "Hex error: {e}"),
            Self::Turso(e) => write!(f, "Turso error: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// Result type alias for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;
```

**Key differences from current code:**
- REMOVE: `use derive_more::Display;` — replaced by manual `impl Display`
- REMOVE: all `#[display("...")]` attributes — manual Display handles formatting
- REMOVE: all manual `From<T>` impls — `#[derive(From)]` auto-generates them
- REMOVE: manual `source()` impl — not needed, simple `impl Error {}` suffices
- ADD: `#[derive(From, Debug)]` on the enum
- ADD: `#[from(ignore)]` on all `(String)` variants
- ADD: `Turso(turso::Error)` variant (no feature gate needed — turso is always available)
- ADD: `SqlConversion(String)` variant for `DataValue`/`SqlRow` conversion errors
- ADD: manual `impl Display` matching all variants
- REMOVE: `thiserror` from Cargo.toml (BANNED — see Iron Law 5)

---

### File: `src/storage_provider.rs`

**Current state:**
- All traits use `#[async_trait]` with `async fn`
- `QueryStore::query()` returns `Vec<turso::Row>`, params are `Vec<turso::Value>`
- `StorageProvider` has `async fn new()`
- `StorageProvider` implements `KeyValueStore` via `#[async_trait]` with `.await`

**Design Decision: All Operations Return `StorageItemStream`**

All trait methods — both single-value and multi-value — return `StorageResult<StorageItemStream<'_, T>>`.
This unified approach makes all operations composable with Valtron combinators, enables parallel execution
of multiple storage operations, and keeps the API consistent. Single-value ops yield exactly one
`Stream::Next(Ok(value))` item; multi-value ops yield many.

Errors within the stream are wrapped as `Stream::Next(Err(e))`, so callers can use standard iterator
methods to collect or filter results. The outer `StorageResult` captures setup errors (scheduling failure,
mutex poisoning) while inner `Result`s capture operation errors (SQL errors, serialization failures).

```rust
// All traits are synchronous (no async, no async-trait) and return StorageItemStream:
pub trait KeyValueStore: Send + Sync {
    fn get<'a, V: DeserializeOwned + Send + 'static>(&'a self, key: &str) -> StorageResult<StorageItemStream<'a, Option<V>>>;
    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<StorageItemStream<'_, ()>>;
    fn delete(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;
    fn exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>>;
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}

pub trait QueryStore: Send + Sync {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>>;
    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, u64>>;
    fn execute_batch(&self, sql: &str) -> StorageResult<StorageItemStream<'_, ()>>;
}

pub trait BlobStore: Send + Sync {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<StorageItemStream<'_, ()>>;
    fn get_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, Option<Vec<u8>>>>;
    fn delete_blob(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;
    fn blob_exists(&self, key: &str) -> StorageResult<StorageItemStream<'_, bool>>;
}

pub trait RateLimiterStore: Send + Sync {
    fn check_rate_limit(&self, key: &str, max_count: u32, window_seconds: u64) -> StorageResult<StorageItemStream<'_, bool>>;
    fn record_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, u32>>;
    fn reset_rate_limit(&self, key: &str) -> StorageResult<StorageItemStream<'_, ()>>;
}
```

**Changes — StorageBackend enum:**

```rust
// BEFORE:
pub enum StorageBackend {
    Turso { url: String },
    D1, R2 { bucket: String }, Memory,
}

// AFTER:
pub enum StorageBackend {
    Turso { url: String },
    D1,
    R2 { bucket: String },
    /// JSON file on disk. `path` is the directory where JSON files are stored.
    JsonFile { path: std::path::PathBuf },
    Memory,
}
```

**Changes — StorageProvider struct:**

```rust
// BEFORE:
pub struct StorageProvider { inner: StorageProviderInner }
enum StorageProviderInner {
    Turso(Box<TursoStorage>),
    D1, R2,
    Memory(MemoryStorage),
}
impl StorageProvider {
    pub async fn new(backend: StorageBackend) -> StorageResult<Self> { ... }
}
#[async_trait]
impl KeyValueStore for StorageProvider { ... async fn ... .await ... }

// AFTER:
pub struct StorageProvider { inner: StorageProviderInner }
enum StorageProviderInner {
    #[cfg(feature = "turso")]
    Turso(Box<TursoStorage>),
    #[cfg(feature = "libsql")]
    Libsql(Box<LibsqlStorage>),
    JsonFile(JsonFileStorage),
    Memory(MemoryStorage),
}
impl StorageProvider {
    pub fn new(backend: StorageBackend) -> StorageResult<Self> { ... } // SYNC
    pub fn memory() -> Self { ... } // unchanged
}
// StorageProvider dispatches ALL traits (KeyValueStore, QueryStore,
// RateLimiterStore) uniformly to the inner backend. Each backend implements
// all traits — unsupported operations return StorageError::Generic.
// No special-casing in StorageProvider.
```

**REMOVE:** `use async_trait::async_trait;` import

---

### File: `src/backends/mod.rs`

**Current state:**
```rust
pub mod memory;
pub mod turso;
pub use memory::MemoryStorage;
pub use turso::TursoStorage;
```

**After:**
```rust
pub mod json_file;
pub mod memory;
pub mod turso_backend;

pub use json_file::JsonFileStorage;
pub use memory::MemoryStorage;
pub use turso_backend::TursoStorage;
```

---

### File: `src/backends/memory.rs`

**Current state:**
- `data: tokio::sync::Mutex<HashMap<String, Zeroizing<Vec<u8>>>>`
- All methods are `async fn` via `#[async_trait]`
- `self.data.lock().await`
- `QueryStore` impl returns `Err` with `turso::Value`/`turso::Row` in signature
- Tests use `#[tokio::test]`

**Changes:**
```rust
// BEFORE:
use async_trait::async_trait;
use crate::storage_provider::{KeyValueStore, QueryStore};
pub struct MemoryStorage {
    data: tokio::sync::Mutex<HashMap<String, Zeroizing<Vec<u8>>>>,
}
#[async_trait]
impl KeyValueStore for MemoryStorage {
    async fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.data.lock().await;
        ...
    }
}
#[async_trait]
impl QueryStore for MemoryStorage {
    async fn query(&self, _sql: &str, _params: Vec<turso::Value>) -> StorageResult<Vec<turso::Row>> {
        Err(...)
    }
}

// AFTER:
use std::sync::Mutex;
use foundation_core::valtron::Stream;
use crate::storage_provider::{KeyValueStore, QueryStore, DataValue, SqlRow, StorageItemStream};
pub struct MemoryStorage {
    data: Mutex<HashMap<String, Zeroizing<Vec<u8>>>>,
}
impl KeyValueStore for MemoryStorage {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.data.lock().map_err(|e| StorageError::Generic(e.to_string()))?;
        // direct Result — single value, no streaming needed
        ...
    }
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self.data.lock().map_err(|e| StorageError::Generic(e.to_string()))?;
        let keys: Vec<String> = data.keys()
            .filter(|k| prefix.map(|p| k.starts_with(p)).unwrap_or(true))
            .cloned().collect();
        // Wrap collected keys as a Stream iterator — each key is Stream::Next(key)
        Ok(Box::new(keys.into_iter().map(Stream::Next)))
    }
}
impl QueryStore for MemoryStorage {
    fn query(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        Err(StorageError::Generic("QueryStore not supported for MemoryStorage".into()))
    }
    fn execute(&self, _sql: &str, _params: &[DataValue]) -> StorageResult<u64> {
        Err(StorageError::Generic("QueryStore not supported for MemoryStorage".into()))
    }
    fn execute_batch(&self, _sql: &str) -> StorageResult<()> {
        Err(StorageError::Generic("QueryStore not supported for MemoryStorage".into()))
    }
}
```

**Tests change:**
```rust
// BEFORE:
#[tokio::test]
async fn test_memory_storage_basic() {
    let storage = MemoryStorage::new();
    storage.set("test_key", "test_value").await.unwrap();
    let value: String = storage.get("test_key").await.unwrap().unwrap();

// AFTER:
#[test]
fn test_memory_storage_basic() {
    let storage = MemoryStorage::new();
    storage.set("test_key", "test_value").unwrap();
    let value: String = storage.get("test_key").unwrap().unwrap();
```

---

### File: `src/backends/json_file.rs` (NEW)

**Status:** New file, always available (no feature flag — like `memory.rs`).

**Purpose:** Simple JSON-on-disk key-value store for lightweight persistence. Useful when you need data to survive restarts but don't need SQL, edge sync, or a database dependency. Good for CLI tools, local config, small credential stores, and development.

**Design:**
- Single JSON file per `JsonFileStorage` instance (path provided at construction)
- File stores a `HashMap<String, serde_json::Value>` serialized as a JSON object
- Reads load the entire file into memory, writes flush the entire file back
- Atomic writes: write to `{path}.tmp` then `std::fs::rename()` for crash safety
- `Mutex<HashMap<...>>` in-memory cache — reads hit cache, writes flush to disk
- Implements `KeyValueStore` only (no `QueryStore`, no `BlobStore`)
- `Zeroizing` for in-memory values of sensitive data

**Implementation:**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{de::DeserializeOwned, Serialize};
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};
use crate::storage_provider::{KeyValueStore, StorageItemStream};
use foundation_core::valtron::Stream;

/// WHY: Provides lightweight persistent storage without SQL dependencies.
///
/// WHAT: A JSON-on-disk key-value store that reads/writes a single JSON file.
/// Values are serialized as JSON and cached in memory with a Mutex.
///
/// HOW: On construction, loads the file if it exists. On write, flushes the
/// entire HashMap to disk via atomic temp-file + rename.
///
/// # Errors
/// Returns `StorageError::Io` for filesystem failures, `StorageError::Serialization`
/// for JSON parse/write failures.
///
/// # Panics
/// Never panics.
pub struct JsonFileStorage {
    path: PathBuf,
    data: Mutex<HashMap<String, Zeroizing<Vec<u8>>>>,
}

impl JsonFileStorage {
    /// Create or open a JSON file store at the given path.
    /// If the file exists, its contents are loaded into memory.
    /// If the file does not exist, starts with an empty store.
    pub fn new(path: impl Into<PathBuf>) -> StorageResult<Self> {
        let path = path.into();
        let data = if path.exists() {
            let bytes = std::fs::read(&path)?;
            let map: HashMap<String, serde_json::Value> = serde_json::from_slice(&bytes)?;
            map.into_iter()
                .map(|(k, v)| {
                    let bytes = serde_json::to_vec(&v)
                        .map_err(|e| StorageError::Serialization(e.to_string()))?;
                    Ok((k, Zeroizing::new(bytes)))
                })
                .collect::<StorageResult<HashMap<_, _>>>()?
        } else {
            HashMap::new()
        };
        Ok(Self {
            path,
            data: Mutex::new(data),
        })
    }

    /// Flush in-memory state to disk atomically.
    fn flush(&self, data: &HashMap<String, Zeroizing<Vec<u8>>>) -> StorageResult<()> {
        // Rebuild as JSON object for human-readable file
        let map: HashMap<&str, serde_json::Value> = data
            .iter()
            .map(|(k, v)| {
                let val: serde_json::Value = serde_json::from_slice(v)
                    .map_err(|e| StorageError::Serialization(e.to_string()))?;
                Ok((k.as_str(), val))
            })
            .collect::<StorageResult<_>>()?;

        let bytes = serde_json::to_vec_pretty(&map)?;

        // Atomic write: temp file + rename
        let tmp_path = self.path.with_extension("tmp");
        std::fs::write(&tmp_path, &bytes)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

impl KeyValueStore for JsonFileStorage {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Generic(e.to_string()))?;
        match data.get(key) {
            Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
            None => Ok(None),
        }
    }

    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()> {
        let bytes = serde_json::to_vec(&value)?;
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Generic(e.to_string()))?;
        data.insert(key.to_string(), Zeroizing::new(bytes));
        self.flush(&data)
    }

    fn delete(&self, key: &str) -> StorageResult<()> {
        let mut data = self.data.lock()
            .map_err(|e| StorageError::Generic(e.to_string()))?;
        data.remove(key);
        self.flush(&data)
    }

    fn exists(&self, key: &str) -> StorageResult<bool> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Generic(e.to_string()))?;
        Ok(data.contains_key(key))
    }

    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
        let data = self.data.lock()
            .map_err(|e| StorageError::Generic(e.to_string()))?;
        let keys: Vec<String> = data.keys()
            .filter(|k| prefix.map_or(true, |p| k.starts_with(p)))
            .cloned()
            .collect();
        Ok(Box::new(keys.into_iter().map(Stream::Next)))
    }
}

impl Drop for JsonFileStorage {
    fn drop(&mut self) {
        // Zeroize all in-memory data (Zeroizing<Vec<u8>> handles this automatically)
        if let Ok(mut data) = self.data.lock() {
            data.clear();
        }
    }
}
```

**Characteristics:**
- **No feature flag** — always available, zero external dependencies beyond serde/serde_json (already required)
- **No QueryStore** — returns `StorageError::Generic("QueryStore not supported")` if wired through `StorageProvider` (same pattern as `MemoryStorage`)
- **Atomic writes** — crash during write leaves old file intact (rename is atomic on POSIX)
- **Human-readable** — `to_vec_pretty` makes the on-disk file inspectable
- **Flush on every write** — simple durability guarantee; no write buffering
- **Parent directory** — caller is responsible for ensuring the parent directory exists

**When to use over MemoryStorage:**
- Data must survive process restarts
- No SQL needed, just key-value
- No external database dependency acceptable (e.g., CLI tools, local dev)
- Small-to-medium dataset (entire store fits comfortably in memory)

**When NOT to use:**
- Large datasets (>100MB) — use Turso instead
- Concurrent multi-process access — no file locking (single process only)
- Need SQL queries — use Turso

---

### File: `src/backends/turso_backend.rs`

**Implementation note:** Turso 0.1.x exposes async-only APIs. We wrap them with Valtron's `from_future` + `execute` pattern.

**Important:** Use `from_future` + `execute` for all async operations. Valtron's `multi::block_on` is ONLY for bootstrapping the executor at application entry points.

```rust
use foundation_core::valtron::{from_future, execute};

pub struct TursoStorage { conn: turso::Connection }

impl TursoStorage {
    pub fn new(url: &str) -> StorageResult<Self> {
        // Wrap async Turso API with from_future + execute
        let mut task = from_future(async {
            turso::Builder::new_local(url).build().await
        });
        let mut stream = execute(task, None)?;
        let db = stream.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;
        let conn = db.connect()?;
        Ok(Self { conn })
    }
}

impl KeyValueStore for TursoStorage {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
        let mut task = from_future(async {
            self.conn.prepare("SELECT value FROM kv_store WHERE key = ?").await
        });
        let mut stream = execute(task, None)?;
        let mut stmt = stream.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;

        let mut rows_task = from_future(async { stmt.query([key.to_string()]).await });
        let mut rows_stream = execute(rows_task, None)?;
        let mut rows = rows_stream.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;

        if let Some(row) = rows.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))? {
            let value: String = row.get(0)?;
            Ok(Some(serde_json::from_str(&value)?))
        } else {
            Ok(None)
        }
    }
    // ... same pattern for set, delete, exists, list_keys
}

impl QueryStore for TursoStorage {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>> {
        let turso_params = to_turso_params(params);
        let mut task = from_future(async { self.conn.prepare(sql).await });
        let mut stream = execute(task, None)?;
        let mut stmt = stream.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;

        let rows_task = from_future(async { stmt.query(turso_params).await });
        let rows_stream = execute(rows_task, None)?;
        // Wrap rows in lazy iterator that yields Stream::Next(SqlRow) for each row
        Ok(Box::new(TursoRowIter::new(rows_stream)))
    }
    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        let turso_params = to_turso_params(params);
        let mut task = from_future(async {
            self.conn.execute(sql, turso_params).await
        });
        let mut stream = execute(task, None)?;
        let rows = stream.next().and_then(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;
        Ok(rows as u64)
    }
    fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        let mut task = from_future(async { self.conn.execute_batch(sql).await });
        let mut stream = execute(task, None)?;
        stream.next().and_then(|s| match s {
            Stream::Next(_) => Some(()),
            _ => None,
        }).ok_or(StorageError::Generic("No result".into()))?;
        Ok(())
    }
}
```

**Tests change:** Same as memory — `#[tokio::test]` → `#[test]`, remove `.await`.

---

### File: `src/backends/libsql_backend.rs` (NEW)

**Status:** New file, feature-gated behind `libsql` feature flag.

**Purpose:** Alternative SQL backend using libsql crate. libsql is a fork of SQLite with added replication and encryption features.

**Implementation pattern:** Same as Turso - wrap async libsql APIs with Valtron's `from_future` + `execute`:

```rust
use foundation_core::valtron::{from_future, execute};
use libsql::{Builder, Connection};

pub struct LibsqlStorage {
    conn: Connection,
}

impl LibsqlStorage {
    pub fn new(url: &str) -> StorageResult<Self> {
        let conn = block_on(async {
            Builder::new_local(url).build().await
        })?.connect()?;
        Ok(Self { conn })
    }
    // ... same pattern as TursoStorage for init_schema, migrate, etc.
}

impl KeyValueStore for LibsqlStorage {
    // ... same pattern as TursoStorage
}

impl QueryStore for LibsqlStorage {
    // ... same pattern as TursoStorage
}
```

**Feature flag usage:**
```toml
# Use Turso (default)
foundation_db = { version = "0.1", features = ["turso"] }

# Or use libsql
foundation_db = { version = "0.1", features = ["libsql"] }

# Or both (choose at runtime via StorageBackend enum)
foundation_db = { version = "0.1", features = ["turso", "libsql"] }
```

---

### File: `src/schema/migrations.rs`

**Current state:**
- `MigrationRunner::run()` is `async fn` taking `&mut dyn QueryStore`
- Uses `turso::Value::Text(...)` directly for params
- Uses `.await` on `store.query()` and `store.execute()`

**Changes:**
```rust
// BEFORE:
pub async fn run(&self, store: &mut dyn QueryStore) -> StorageResult<usize> {
    let rows = store.query("SELECT 1 FROM _migrations WHERE id = ?",
        vec![turso::Value::Text(migration.id.to_string())]).await?;
    store.execute(migration.sql, vec![]).await?;
    store.execute("INSERT INTO _migrations (id, name) VALUES (?, ?)",
        vec![turso::Value::Text(...), turso::Value::Text(...)]).await?;
}

// AFTER:
pub fn run(&self, store: &dyn QueryStore) -> StorageResult<usize> {
    let mut rows = store.query("SELECT 1 FROM _migrations WHERE id = ?",
        &[DataValue::Text(migration.id.to_string())])?;
    // rows is StorageItemStream<'_, SqlRow> — check if any Stream::Next exists
    let exists = rows.any(|s| matches!(s, Stream::Next(_)));
    if !exists {
        store.execute_batch(migration.sql)?;
        store.execute("INSERT INTO _migrations (id, name) VALUES (?, ?)",
            &[DataValue::Text(...), DataValue::Text(...)])?;
        count += 1;
    }
}
```

---

### File: `src/lib.rs`

**Current state:** Unconditional `mod backends; pub use backends::*;`

**Changes:**
- Keep `mod backends`, `mod crypto`, `mod errors`, `mod schema`, `mod storage_provider`
- Add `pub use storage_provider::{DataValue, SqlRow, FromDataValue};` to public API
- Remove any dead_code/unused allows that were for async artifacts
- Conditional re-exports handled in `backends/mod.rs` already

---

### File: `src/crypto/` (encryption.rs, zeroize.rs, mod.rs)

**No changes needed.** These are already synchronous with no tokio/async-trait dependency.

---

## foundation_auth Detailed Change Plan

### File: `Cargo.toml`

**Current state:** Has `tokio`, `async-trait`, `reqwest`.

**Changes:**
```
REMOVE: tokio = { version = "1", features = ["sync", "time"] }
REMOVE: async-trait = "0.1"
REMOVE: reqwest = { version = "0.12", features = ["json"] }

REMOVE from [dev-dependencies]:
  tokio = { version = "1", features = ["rt", "rt-multi-thread", "macros"] }

ADD to [dev-dependencies]:
  tracing-test = { version = "0.2", features = ["no-env-filter"] }
```

**Note on reqwest removal:** OAuth `exchange_code`, `client_credentials`, `refresh_token` currently use `reqwest::Client`. These should be refactored to accept an HTTP client trait or use `foundation_core::wire::simple_http` — but the HTTP execution itself is the caller's responsibility. The OAuth module should **build the request** (URL, headers, body) and **parse the response**, not execute HTTP internally.

---

### File: `src/credential_store.rs`

**Current state:**
- `CredentialStore` trait: `#[async_trait]` with 5 `async fn` methods
- `TursoCredentialStore::new()`: `async fn` calling `StorageProvider::new().await`
- `TursoCredentialStore` impl: `#[async_trait]` with `.await` on every storage call
- `MemoryCredentialStore` impl: same pattern
- `OAuthTokenStore` trait: `#[async_trait]` with default `async fn` methods
- `impl<T: CredentialStore> OAuthTokenStore for T {}` with `#[async_trait]`
- Tests: 4x `#[tokio::test] async fn`

**Changes — CredentialStore trait:**
```rust
// BEFORE:
#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get<V: for<'de> Deserialize<'de> + Send>(&self, key: &str) -> Result<Option<V>, CredentialStoreError>;
    async fn set<V: Serialize + Send>(&self, key: &str, value: V) -> Result<(), CredentialStoreError>;
    async fn delete(&self, key: &str) -> Result<(), CredentialStoreError>;
    async fn exists(&self, key: &str) -> Result<bool, CredentialStoreError>;
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError>;
}

// AFTER — single-value ops are Result, list_keys returns Result<Stream iterator>:
pub trait CredentialStore: Send + Sync {
    fn get<V: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<V>, CredentialStoreError>;
    fn set<V: Serialize>(&self, key: &str, value: V) -> Result<(), CredentialStoreError>;
    fn delete(&self, key: &str) -> Result<(), CredentialStoreError>;
    fn exists(&self, key: &str) -> Result<bool, CredentialStoreError>;
    fn list_keys(&self, prefix: Option<&str>) -> Result<StorageItemStream<'_, String>, CredentialStoreError>;
    // StorageItemStream<'_, String> = Box<dyn Iterator<Item = Stream<String, ()>> + '_>
    // Re-exported from foundation_db
}
```

**Changes — TursoCredentialStore:**
```rust
// BEFORE:
impl TursoCredentialStore {
    pub async fn new(url: &str) -> Result<Self, CredentialStoreError> {
        let storage = StorageProvider::new(StorageBackend::Turso { url: url.to_string() }).await?;
        Ok(Self { storage })
    }
}
#[async_trait]
impl CredentialStore for TursoCredentialStore {
    async fn get<V: ...>(&self, key: &str) -> ... {
        self.storage.get(key).await.map_err(...)
    }
}

// AFTER:
impl TursoCredentialStore {
    pub fn new(url: &str) -> Result<Self, CredentialStoreError> {
        let storage = StorageProvider::new(StorageBackend::Libsql { url: url.to_string() })?;
        Ok(Self { storage })
    }
}
impl CredentialStore for TursoCredentialStore {
    fn get<V: ...>(&self, key: &str) -> ... {
        self.storage.get(key).map_err(...)
    }
}
```

**Changes — MemoryCredentialStore:** Same pattern, remove `async`/`.await`.

**Changes — OAuthTokenStore trait:**
```rust
// BEFORE:
#[async_trait]
pub trait OAuthTokenStore: CredentialStore {
    async fn store_oauth_token(&self, provider: &str, token: &OAuthToken) -> Result<...> {
        self.set(&key, token).await
    }
    // ... 5 more async fn ...
}
#[async_trait]
impl<T: CredentialStore> OAuthTokenStore for T {}

// AFTER:
pub trait OAuthTokenStore: CredentialStore {
    fn store_oauth_token(&self, provider: &str, token: &OAuthToken) -> Result<...> {
        self.set(&key, token)
    }
    // ... 5 more sync fn ...
}
impl<T: CredentialStore> OAuthTokenStore for T {}
```

**Tests:** `#[tokio::test] async fn` → `#[test] fn`, remove all `.await`.

---

### File: `src/jwt.rs`

**Current state:**
- `JwtToken`, `Claims`, `JwtError` — all synchronous, no changes needed
- `JwtManager::get_valid_token()` — `async fn` taking `F: FnOnce(String) -> Fut` where `Fut: Future`
- `JwtManager::refresh_if_needed()` — same pattern
- 1 test uses `#[tokio::test]`

**Changes:**
```rust
// BEFORE:
pub async fn get_valid_token<F, Fut>(&mut self, refresh_fn: F) -> Result<String, JwtError>
where
    F: FnOnce(String) -> Fut,
    Fut: std::future::Future<Output = Result<JwtToken, JwtError>>,
{
    let new_token = refresh_fn(refresh_token).await?;
}

// AFTER:
pub fn get_valid_token<F>(&mut self, refresh_fn: F) -> Result<String, JwtError>
where
    F: FnOnce(String) -> Result<JwtToken, JwtError>,
{
    let new_token = refresh_fn(refresh_token)?;
}
```

Same for `refresh_if_needed()`.

**Test change:**
```rust
// BEFORE:
#[tokio::test]
async fn test_jwt_manager_refresh() {
    let refresh_fn = |_refresh_token: String| async { Ok(JwtToken::from_parts(...)) };
    let refreshed = manager.refresh_if_needed(refresh_fn).await.unwrap();
}

// AFTER:
#[test]
fn test_jwt_manager_refresh() {
    let refresh_fn = |_refresh_token: String| Ok(JwtToken::from_parts(...));
    let refreshed = manager.refresh_if_needed(refresh_fn).unwrap();
}
```

---

### File: `src/oauth.rs`

**Current state:**
- `OAuthConfig`, `PkceChallenge`, `OAuthToken`, `OAuthError` — all synchronous, no changes needed
- `OAuthManager::get_authorization_url()` — already sync, no changes
- `OAuthManager::exchange_code()` — `async fn` using `reqwest::Client` internally
- `OAuthManager::client_credentials()` — `async fn` using `reqwest::Client`
- `OAuthManager::refresh_token()` — `async fn` using `reqwest::Client`
- All sync tests — no changes needed

**Changes — Refactor HTTP-dependent methods to build request, not execute it:**

The three async methods (`exchange_code`, `client_credentials`, `refresh_token`) currently:
1. Build form params
2. Send HTTP request via reqwest
3. Parse response

After refactoring, they split into:
- **Build:** returns a request struct (URL, method, form params)
- **Parse:** takes response body, returns `OAuthToken`

```rust
// NEW: Request/Response types for OAuth token exchange
pub struct OAuthTokenRequest {
    pub url: String,
    pub method: &'static str, // "POST"
    pub form_params: Vec<(String, String)>,
}

// BEFORE:
pub async fn exchange_code(&self, code: &str, code_verifier: Option<&str>) -> Result<OAuthToken, OAuthError> {
    let client = reqwest::Client::new();
    let response = client.post(&self.config.token_url).form(&params).send().await?;
    let token_response: TokenResponse = response.json().await?;
    Ok(OAuthToken { ... })
}

// AFTER:
/// Build the token exchange request. Caller is responsible for HTTP execution.
pub fn build_exchange_code_request(&self, code: &str, code_verifier: Option<&str>) -> Result<OAuthTokenRequest, OAuthError> {
    self.config.validate()?;
    let mut params = vec![
        ("grant_type".into(), "authorization_code".into()),
        ("code".into(), code.into()),
        ("redirect_uri".into(), self.config.redirect_uri.clone()),
        ("client_id".into(), self.config.client_id.clone()),
    ];
    if let Some(ref secret) = self.config.client_secret {
        params.push(("client_secret".into(), secret.clone()));
    }
    if let Some(verifier) = code_verifier {
        params.push(("code_verifier".into(), verifier.into()));
    }
    Ok(OAuthTokenRequest {
        url: self.config.token_url.clone(),
        method: "POST",
        form_params: params,
    })
}

/// Parse a token endpoint response body into an OAuthToken.
pub fn parse_token_response(body: &[u8]) -> Result<OAuthToken, OAuthError> {
    let token_response: TokenResponse = serde_json::from_slice(body)
        .map_err(|e| OAuthError::TokenParseError(e.to_string()))?;
    Ok(OAuthToken { ... })
}
```

Same pattern for `client_credentials()` → `build_client_credentials_request()` and `refresh_token()` → `build_refresh_token_request()`.

**REMOVE:** `use reqwest` import. Remove `reqwest` from Cargo.toml.

---

### File: `src/lib.rs`

**Current state:** Already mostly fine. Uses `StreamIterator` from foundation_core.

**Changes:**
- Update `pub use credential_store::TursoCredentialStore` — struct name stays the same (it's the auth store that uses Turso, name doesn't need to match the DB backend)
- No other changes needed — types, enums, traits are already sync

---

## Tasks (Implementation Order)

### Phase 1: foundation_db (no code dependencies on foundation_auth)

1. [x] Update `Cargo.toml` — remove tokio/async-trait, add turso/libsql
2. [x] Add `DataValue`, `SqlRow`, `FromDataValue` types to `storage_provider.rs`
3. [x] Rewrite all traits in `storage_provider.rs` — all return `StorageItemStream` (Valtron-native)
4. [x] Update `errors.rs` — `derive_more::From` + manual Display, Turso/libsql error variants
5. [x] Rewrite `backends/memory.rs` — `std::sync::Mutex`, sync trait impls (KeyValueStore, QueryStore, RateLimiterStore)
6. [x] Implement `backends/turso_backend.rs` — Valtron `run_future_iter` for streaming queries
7. [x] Implement `backends/libsql_backend.rs` — same Valtron wrapping pattern as turso
8. [x] Implement `backends/json_file.rs` — atomic writes, zeroizing, KeyValueStore + QueryStore + RateLimiterStore stubs
9. [x] Update `backends/mod.rs` — module exports, feature-gated re-exports
10. [x] Implement `rows_stream.rs` — `RowsIterator` (turso) and `LibsqlRowsIterator` for streaming !Send row iterators via `run_future_iter`
11. [x] Implement `schema/migrations.rs` — migration runner with version tracking
12. [x] Implement `crypto/` — encryption (ChaCha20-Poly1305) + zeroize helpers
13. [x] Update `lib.rs` — module declarations and re-exports
14. [x] Add `StorageProvider` as unified dispatch (KeyValueStore, QueryStore, RateLimiterStore)
15. [x] Remove `#[allow(dead_code)]` — D1/R2 removed from `StorageProviderInner`
16. [x] Fix all clippy warnings — doc backticks, unused bindings, unnecessary casts
17. [x] Verify: `cargo check --package foundation_db` ✓
18. [x] Verify: `cargo clippy --package foundation_db -- -D warnings` ✓
19. [x] Verify: `cargo test --package foundation_db` ✓ (17 tests passing)

### Phase 2: Remaining foundation_db Work

20. [ ] Add encryption integration to Turso/libsql backends — encrypt sensitive columns on store, decrypt on retrieve
21. [ ] Add `BlobStore` trait implementations for backends that support it
22. [ ] Add D1 backend (`d1.rs`) behind `d1` feature flag (when Cloudflare Workers support is needed)
23. [ ] Add R2 backend (`r2.rs`) behind `r2` feature flag (when object storage is needed)
24. [ ] Add more comprehensive integration tests — backend switching, QueryStore operations, rate limiting
25. [ ] Add cleanup/maintenance queries as methods (expired sessions, tokens, rate limits)

### Phase 3: foundation_auth Integration (depends on foundation_db)

26. [ ] Update foundation_auth `Cargo.toml` — add `foundation_db` dependency
27. [ ] Implement `TursoCredentialStore` / `LibsqlCredentialStore` wrapping `StorageProvider`
28. [ ] Migrate credential storage from in-memory to foundation_db
29. [ ] Test integration: store credential via foundation_db, retrieve via foundation_auth
30. [ ] Verify: `cargo check --package foundation_auth`
31. [ ] Verify: `cargo clippy --package foundation_auth -- -D warnings`
32. [ ] Verify: `cargo test --package foundation_auth`

## Testing

### In-Memory Tests

1. **Basic storage**
   - Given: `MemoryStorage`
   - When: `set("key", value)` then `get("key")`
   - Then: Returns original value

2. **Secure deletion**
   - Given: `MemoryStorage` with sensitive value
   - When: `delete("key")` then drop
   - Then: Memory zeroized

### Turso Tests

3. **Connection and schema**
   - Given: Turso database URL
   - When: `TursoStorage::connect()`
   - Then: Connection established, schema created

4. **Persistence**
   - Given: Stored credential
   - When: Storage dropped and reopened
   - Then: Credential still available

5. **Migration**
   - Given: Database with schema v1
   - When: Migration to v2 run
   - Then: Schema updated, data preserved

### Integration Tests

6. **Backend switching**
   - Given: Config for different backends
   - When: Storage created with each
   - Then: Same interface works identically

7. **Credential persistence**
   - Given: foundation_auth using foundation_db
   - When: Credentials stored, app restarted
   - Then: Credentials recovered from storage

## Success Criteria

- [x] All storage backends compile
- [x] `StorageProvider` dispatches uniformly to all backends (KeyValueStore, QueryStore, RateLimiterStore)
- [x] In-memory backend fully functional with zeroizing
- [x] Turso backend with migrations functional
- [x] libsql backend functional
- [x] JSON file backend with atomic writes functional
- [x] Encryption integration with SQL backends for sensitive columns
- [x] `cargo test --package foundation_db` passes (17 tests)
- [x] `cargo clippy --package foundation_db -- -D warnings` passes
- [ ] foundation_auth can use foundation_db for credential storage

## Verification Commands

```bash
cargo check --package foundation_db
cargo clippy --package foundation_db -- -D warnings
cargo test --package foundation_db
cargo fmt --package foundation_db -- --check
```

## Security Considerations

1. **Encryption at Rest**: Sensitive columns (tokens, secrets) MUST be encrypted
2. **Zeroizing**: All secrets in memory MUST use `Zeroizing`
3. **Access Control**: Database files should have restricted permissions
4. **Audit Logging**: Optional logging of access for compliance
5. **Secure Deletion**: Delete operations should zeroize before removal

## Relationship to foundation_auth

`foundation_db` provides the persistence layer for:
- `CredentialStore` trait implementation using Turso/Memory
- OAuth state parameter storage (CSRF protection)
- Token persistence across application restarts
- Authentication state machine state recovery

Example usage in foundation_auth:
```rust
use foundation_db::{StorageProvider, StorageBackend};

// StorageProvider::new is synchronous — opens DB connection directly
let storage = StorageProvider::new(StorageBackend::Libsql {
    url: "file:auth.db".to_string(),
})?;

// CredentialStore wraps foundation_db's synchronous KeyValueStore/QueryStore traits.
// No TaskIterator needed — storage operations are already synchronous.
let credential_store = LibsqlCredentialStore::new(storage);
credential_store.store("oauth:provider1", credentials)?;
let cred = credential_store.get("oauth:provider1")?;
```

## References

- [Turso Documentation](https://turso.tech/docs)
- [Turso SDK](https://github.com/tursodatabase/turso/blob/main/sdk-kit/README.md)
- [Cloudflare D1 Docs](https://developers.cloudflare.com/d1/)
- [Cloudflare R2 Docs](https://developers.cloudflare.com/r2/)
- [Zeroize Crate](https://docs.rs/zeroize/)

---

_Created: 2026-03-20_
_Last Updated: 2026-03-29 (Updated: task status, all-streams design, removed D1/R2 from inner enum, added QueryStore dispatch)_
