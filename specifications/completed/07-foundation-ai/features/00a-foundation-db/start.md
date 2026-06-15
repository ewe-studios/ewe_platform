---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
this_file: "specifications/07-foundation-ai/features/00a-foundation-db/start.md"
created: 2026-03-20
author: "Main Agent"
---

# Start: Foundation DB - Unified Storage Backend

## Feature Overview

Create `foundation_db` crate in `backends/foundation_db/` providing unified storage abstraction with Turso sync backend, Cloudflare D1, R2 object storage, and in-memory fallback. This enables `foundation_auth` to persist credentials, tokens, and auth state.

## Iron Laws (READ FIRST)

**Before doing ANY work, read the Iron Laws in `feature.md`.** Summary:
1. **No tokio, No async-trait** — BANNED. Storage traits are synchronous.
2. **Turso sync backend** — Uses Turso crate with sync API (no feature flags needed)
3. **Valtron Stream for multi-value** — `list_keys`, `query`, `get_blob` return `StorageItemStream` (Valtron `Stream`-based lazy iterator). Single-value ops return `StorageResult<T>` directly.

## Required Reading (Before Implementation)

1. **Read `.agents/skills/rust-valtron-usage/skill.md`** — Valtron execution model, stream-returning patterns, sync boundary helpers. This is MANDATORY before writing any async/I/O code.
2. **Read `feature.md`** — Full requirements, Iron Laws, task list.
3. **Read `../../LEARNINGS.md`** — Spec-level learnings including `from_future` patterns.
4. **Read `./LEARNINGS.md`** — Feature-specific learnings (exec_future, !Send constraints, StorageItemStream).

## Prerequisites

Before starting, understand:
1. `foundation_core::valtron` async patterns (`TaskIterator`, `StreamIterator`, `execute()`)
2. Turso crate sync API and SQL basics
3. Encryption fundamentals for data at rest
4. `Zeroizing` for secure memory handling

## Agent Workflow

### Phase 1: Project Setup

1. **Create foundation_db crate**
   ```bash
   mkdir -p backends/foundation_db/src/{backends,schema,crypto}
   mkdir -p backends/foundation_db/tests
   ```

2. **Create Cargo.toml**
   - Create: `backends/foundation_db/Cargo.toml`
   - Add dependencies: `serde`, `serde_json`, `zeroize`, `thiserror`, `derive_more`, `tracing`
   - Add: `turso = "0.1"` (no feature flag needed - always available)
   - **NO tokio, NO async-trait**

3. **Create lib.rs**
   - Create: `backends/foundation_db/src/lib.rs`
   - Declare modules: `storage_provider`, `backends`, `schema`, `crypto`, `errors`

### Phase 2: Core Traits

4. **Create storage_provider.rs**
   - Create: `backends/foundation_db/src/storage_provider.rs`
   - Define `StorageProvider` struct with `new(backend) -> StorageResult<Self>` (synchronous)
   - Define `StorageBackend` enum: `Turso { url }`, `D1 { binding }`, `R2 { bucket }`, `JsonFile { path }`, `Memory`
   - Define `KeyValueStore` trait — synchronous methods, NO async-trait
   - Define `BlobStore` trait — synchronous single-value ops, streamed `get_blob`
   - Define `QueryStore` trait — with crate-owned `DataValue`/`DataRow` types (NO turso::Value leak)
   - Multi-value ops (`query`, `list_keys`, `get_blob`) return `StorageItemStream` (Valtron `Stream`-based lazy iterator)
   - Single-value ops (`get`, `set`, `delete`, `exists`, `execute`) return `StorageResult<T>` directly

5. **Create errors.rs**
   - Create: `backends/foundation_db/src/errors.rs`
   - Define `StorageError` enum with variants:
     - `ConnectionFailed(String)`
     - `QueryFailed(String)`
     - `NotFound(String)`
     - `EncryptionError(String)`
     - `BackendError(String)`
   - Implement `Display`, `Debug`, `Error`

### Phase 3: In-Memory Backend

6. **Create memory.rs**
   - Create: `backends/foundation_db/src/backends/memory.rs`
   - Implement `MemoryStorage` struct with `Arc<Mutex<HashMap<String, Zeroizing<Vec<u8>>>>>`
   - Implement `KeyValueStore` trait
   - Implement `Drop` to clear all data
   - Test: Basic CRUD, zeroizing on drop

### Phase 4: SQL Backends

7. **Create turso_backend.rs**
   - Create: `backends/foundation_db/src/backends/turso_backend.rs`
   - Implement `TursoStorage` struct using `turso` crate with sync API
   - Implement `KeyValueStore` and `QueryStore` traits (synchronous)
   - Multi-value ops (`query`, `list_keys`) return `StorageItemStream` (Valtron `Stream`-based lazy iterator)

8. **Create schema system**
   - Create: `backends/foundation_db/src/schema/mod.rs`
   - Create: `backends/foundation_db/src/schema/migrations.rs`
   - Define auth schema tables (oauth_credentials, jwt_tokens, session_credentials, auth_states, oauth_states)
   - Implement migration runner with version tracking

9. **Add encryption layer**
   - Create: `backends/foundation_db/src/crypto/mod.rs`
   - Implement `encrypt(value: &[u8], key: &[u8]) -> Vec<u8>`
   - Implement `decrypt(encrypted: &[u8], key: &[u8]) -> Result<Vec<u8>>`
   - Use `chacha20poly1305` or `aes-gcm`
   - Wrap sensitive values in encrypt/decrypt on store/retrieve

### Phase 5: Integration with foundation_auth

10. **Update foundation_auth Cargo.toml**
    - Add: `foundation_db = { workspace = true }`

11. **Extend foundation_auth credential_store**
    - Update: `backends/foundation_auth/src/credential_store.rs` (if exists)
    - Or create new module using `foundation_db::StorageProvider`
    - Implement `TursoCredentialStore` wrapping `TursoStorage`

12. **Test integration**
    - Create: `backends/foundation_db/tests/integration_tests.rs`
    - Test: Store credential via foundation_db, retrieve via foundation_auth

### Phase 6: Verification

13. **Run verification**
    ```bash
    # foundation_db
    cargo check --package foundation_db
    cargo clippy --package foundation_db -- -D warnings
    cargo test --package foundation_db

    # foundation_auth (verify integration)
    cargo check --package foundation_auth
    ```

14. **Update LEARNINGS.md**
    - Document Turso setup and usage patterns
    - Document encryption approach
    - Document schema migrations design

## File Checklist

- [ ] `backends/foundation_db/Cargo.toml` (NO tokio, NO async-trait)
- [ ] `backends/foundation_db/src/lib.rs`
- [ ] `backends/foundation_db/src/storage_provider.rs` (Valtron-based traits)
- [ ] `backends/foundation_db/src/errors.rs`
- [ ] `backends/foundation_db/src/backends/mod.rs`
- [ ] `backends/foundation_db/src/backends/memory.rs` (std::sync::Mutex)
- [ ] `backends/foundation_db/src/backends/json_file.rs` (JSON-on-disk KV store)
- [ ] `backends/foundation_db/src/backends/turso_backend.rs` (Turso sync backend)
- [ ] `backends/foundation_db/src/schema/mod.rs`
- [ ] `backends/foundation_db/src/schema/migrations.rs`
- [ ] `backends/foundation_db/src/crypto/mod.rs`
- [ ] `backends/foundation_db/tests/foundation_db_tests.rs` (external tests)

## Key Implementation Details

### StorageBackend Enum
```rust
pub enum StorageBackend {
    Turso { url: String },
    D1 { binding: String },
    R2 { bucket: String },
    JsonFile { path: std::path::PathBuf },
    Memory,
}
```

### KeyValueStore Trait (Synchronous, NO async-trait)
```rust
// All backends implement synchronous methods. Multi-value ops return
// StorageItemStream (Valtron Stream-based lazy iterator). See feature.md
// for full trait definitions including QueryStore and BlobStore.
pub trait KeyValueStore: Send + Sync {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>>;
    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()>;
    fn delete(&self, key: &str) -> StorageResult<()>;
    fn exists(&self, key: &str) -> StorageResult<bool>;
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}
```

### Turso Backend
```rust
pub struct TursoStorage {
    conn: turso::Connection,
    encryption_key: Option<Zeroizing<Vec<u8>>>,
}
```

### Encryption Wrapper
```rust
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce, KeyInit};
use rand::RngCore;

pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Vec<u8> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);

    let ciphertext = cipher.encrypt(Nonce::from_slice(&nonce), plaintext)
        .expect("encryption failed");

    // Prepend nonce to ciphertext
    let mut result = nonce.to_vec();
    result.extend(ciphertext);
    result
}
```

## Success Criteria

- [ ] `foundation_db` crate compiles without errors
- [ ] In-memory backend fully functional
- [ ] Turso backend with migrations functional
- [ ] Encryption working for sensitive columns
- [ ] `foundation_auth` can store/retrieve credentials
- [ ] `cargo clippy -- -D warnings` passes
- [ ] All tests pass

## Dependencies

Add to `Cargo.toml` (see feature.md Cargo.toml section for full details):
```toml
[dependencies]
foundation_core = { workspace = true }
turso = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
zeroize = { version = "1", features = ["derive"] }
derive_more = { version = "2.0", features = ["from", "error", "display"] }
chacha20poly1305 = "0.10"
rand = "0.8"
tracing = "0.1"
# NOTE: tokio, async-trait, thiserror are BANNED
# Use foundation_core::valtron for async, derive_more::From + manual Display for errors

[features]
default = []
d1 = []
r2 = []
```

---

_Created: 2026-03-20_
