---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/00a-foundation-db"
this_file: "specifications/07-foundation-ai/features/00a-foundation-db/feature.md"

feature: "Foundation DB - Unified Storage Backend"
description: "Create foundation_db crate providing unified storage abstraction wrapping Turso (libsql), Cloudflare D1, and R2 object storage with in-memory fallback for credential and state persistence"
status: pending
priority: high
depends_on:
  - "00-foundation"
  - "foundation_core"
estimated_effort: "medium"
created: 2026-03-20
last_updated: 2026-03-20
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 24
  total: 24
  completion_percentage: 0%
---

# Foundation DB - Unified Storage Backend

## Overview

`foundation_db` is a unified storage backend crate that provides a consistent abstraction layer for persisting data across multiple storage providers:

1. **Turso (libsql)** - Local/remote SQLite with edge sync
2. **Cloudflare D1** - Edge SQLite for Cloudflare Workers
3. **Cloudflare R2** - Object storage for larger blobs
4. **In-Memory** - Ephemeral storage for development/testing

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
- Support Turso (libsql) for local/remote SQLite
- Support Cloudflare D1 for edge SQLite
- Support Cloudflare R2 for blob storage
- Provide in-memory backend for dev/test
- Enable automatic backend selection based on configuration
- Support encrypted storage for sensitive credentials
- Maintain async-first API using `foundation_core::valtron`
- Zeroize sensitive data on drop

## Dependencies

**Required Crates:**
- `foundation_core` - For `valtron` async patterns, `ConfidentialText`
- `turso` - Turso/libsql client (add to Cargo.toml)
- `zeroize` - For secure memory clearing
- `serde` + `serde_json` - For serialization
- `thiserror` - For error handling
- `tokio` or `valtron` runtime - Async execution

**Required By:**
- `foundation_auth` - Credential and state persistence
- `foundation_ai` - Token caching, usage tracking
- Any crate requiring persistent secure storage

## Requirements

### Core Storage Abstraction

1. **StorageProvider Trait** - Unified interface for all storage backends
2. **StorageBackend Enum** - Runtime backend selection (Turso, D1, R2, Memory)
3. **KeyValueStore Trait** - Key-value operations across all backends
4. **BlobStore Trait** - Binary large object storage (R2-specific)
5. **QueryStore Trait** - SQL query capabilities (Turso/D1-specific)

### Turso Backend

6. **TursoStorage Struct** - libsql implementation
7. **Connection Pool** - Efficient connection management
8. **Migration System** - Schema versioning and migrations
9. **Sync Support** - Turso edge sync capabilities
10. **Local Database** - Embedded SQLite mode

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

### Security Features

21. **Encrypted Storage** - Optional encryption at rest
22. **Secure Deletion** - Zeroize on delete/drop
23. **Access Control** - Optional per-key access control
24. **Audit Logging** - Optional operation logging

## Architecture

### Storage Abstraction

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
    end

    subgraph Backends
        H[TursoStorage]
        I[D1Storage]
        J[R2Storage]
        K[MemoryStorage]
    end

    A --> C
    B --> C
    C --> D
    D --> H
    D --> I
    D --> J
    D --> K
    H --> E
    H --> G
    I --> E
    I --> G
    J --> F
    K --> E
```

### Credential Storage Schema (Turso/D1)

```sql
-- OAuth credentials table
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

-- JWT tokens table
CREATE TABLE jwt_tokens (
    id TEXT PRIMARY KEY,
    access_token_encrypted TEXT NOT NULL,
    refresh_token_encrypted TEXT,
    expires_at INTEGER NOT NULL,
    scope TEXT,
    audience TEXT,
    issuer TEXT,
    created_at INTEGER NOT NULL
);

-- Session credentials table
CREATE TABLE session_credentials (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    token_encrypted TEXT NOT NULL,
    cookie_data TEXT,  -- JSON serialized
    expires_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

-- Auth state machine table
CREATE TABLE auth_states (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    state TEXT NOT NULL,  -- JSON serialized AuthState
    last_transition INTEGER NOT NULL,
    pending_requests TEXT  -- JSON array of queued requests
);

-- OAuth state parameter tracking (CSRF protection)
CREATE TABLE oauth_states (
    state_param TEXT PRIMARY KEY,
    original_url TEXT NOT NULL,
    code_verifier TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    used BOOLEAN DEFAULT FALSE
);
```

## Implementation

### Files to Create

```
backends/foundation_db/
├── Cargo.toml
├── src/
│   ├── lib.rs                     - Module declarations, StorageProvider trait
│   ├── storage_provider.rs        - Core trait definitions
│   ├── backends/
│   │   ├── mod.rs                 - Backend module exports
│   │   ├── turso.rs               - Turso/libsql implementation
│   │   ├── d1.rs                  - Cloudflare D1 implementation
│   │   ├── r2.rs                  - Cloudflare R2 implementation
│   │   └── memory.rs              - In-memory implementation
│   ├── schema/
│   │   ├── mod.rs                 - Schema definitions
│   │   └── migrations.rs          - Migration system
│   ├── crypto/
│   │   ├── mod.rs                 - Encryption utilities
│   │   └── zeroize.rs             - Secure deletion helpers
│   └── errors.rs                  - Error types
└── tests/
    ├── turso_tests.rs
    ├── memory_tests.rs
    └── integration_tests.rs
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

# Storage backends
libsql = "0.4"           # Turso/libsql
# d1 = "..."             # Cloudflare D1 (workers-rs)
# r2 = "..."             # Cloudflare R2 (workers-rs)

# Async
tokio = { version = "1", features = ["rt", "sync"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Error handling
thiserror = "2.0"

# Security
zeroize = { version = "1" }
argon2 = "0.5"           # Password hashing
chacha20poly1305 = "0.10" # Encryption

# Utilities
async-trait = "0.1"
```

## Tasks

### Task Group 1: Core Traits

- [ ] Create `backends/foundation_db/Cargo.toml`
- [ ] Create `src/lib.rs` with module declarations
- [ ] Create `src/storage_provider.rs` with `StorageProvider` trait
- [ ] Define `StorageBackend` enum (Turso, D1, R2, Memory)
- [ ] Define `KeyValueStore` trait: `get()`, `set()`, `delete()`, `exists()`
- [ ] Define `BlobStore` trait: `put_blob()`, `get_blob()`, `delete_blob()`
- [ ] Define `QueryStore` trait: `query()`, `execute()`
- [ ] Create `src/errors.rs` with `StorageError` enum

### Task Group 2: In-Memory Backend

- [ ] Create `src/backends/memory.rs`
- [ ] Implement `MemoryStorage` struct with `HashMap`
- [ ] Wrap values in `Zeroizing` for sensitive data
- [ ] Implement `KeyValueStore` trait
- [ ] Implement `Drop` for secure cleanup
- [ ] Test: Basic CRUD operations
- [ ] Test: Zeroizing on drop

### Task Group 3: Turso Backend

- [ ] Create `src/backends/turso.rs`
- [ ] Implement `TursoStorage` struct with `libsql::Connection`
- [ ] Implement connection pool
- [ ] Create `src/schema/migrations.rs` with migration system
- [ ] Implement schema initialization
- [ ] Implement `KeyValueStore` trait
- [ ] Implement `QueryStore` trait
- [ ] Add encryption layer for sensitive columns
- [ ] Test: Basic CRUD operations
- [ ] Test: Migration execution

### Task Group 4: Cloudflare Backends (Optional/Phase 2)

- [ ] Create `src/backends/d1.rs`
- [ ] Implement `D1Storage` for Cloudflare Workers
- [ ] Create `src/backends/r2.rs`
- [ ] Implement `R2Storage` for object storage
- [ ] Implement `BlobStore` trait for R2

### Task Group 5: Security

- [ ] Create `src/crypto/mod.rs`
- [ ] Implement encryption wrapper for sensitive values
- [ ] Implement `src/crypto/zeroize.rs` helpers
- [ ] Add optional encryption to all backends
- [ ] Test: Encrypted storage and retrieval

### Task Group 6: Integration

- [ ] Create `tests/integration_tests.rs`
- [ ] Test: Backend selection and switching
- [ ] Test: Credential persistence across restarts
- [ ] Run `cargo test --package foundation_db`
- [ ] Run `cargo clippy --package foundation_db -- -D warnings`

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

- [ ] All storage backends compile
- [ ] `StorageProvider` trait implemented consistently
- [ ] In-memory backend fully functional with zeroizing
- [ ] Turso backend with migrations functional
- [ ] Encryption wrapper for sensitive data working
- [ ] `cargo test --package foundation_db` passes
- [ ] `cargo clippy --package foundation_db -- -D warnings` passes
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

let storage = StorageProvider::new(StorageBackend::Turso {
    url: "file:auth.db".to_string(),
}).await?;

let mut credential_store = TursoCredentialStore::new(storage);
credential_store.store("oauth:provider1", credentials).await?;
```

## References

- [Turso Documentation](https://turso.tech/docs)
- [libsql GitHub](https://github.com/libsql/libsql)
- [Cloudflare D1 Docs](https://developers.cloudflare.com/d1/)
- [Cloudflare R2 Docs](https://developers.cloudflare.com/r2/)
- [Zeroize Crate](https://docs.rs/zeroize/)

---

_Created: 2026-03-20_
_Last Updated: 2026-03-20_
