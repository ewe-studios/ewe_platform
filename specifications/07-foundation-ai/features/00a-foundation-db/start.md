---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
this_file: "specifications/07-foundation-ai/features/00a-foundation-db/start.md"
created: 2026-03-20
author: "Main Agent"
---

# Start: Foundation DB - Unified Storage Backend

## Feature Overview

Create `foundation_db` crate in `backends/foundation_db/` providing unified storage abstraction for Turso (libsql), Cloudflare D1, R2 object storage, and in-memory backend. This enables `foundation_auth` to persist credentials, tokens, and auth state.

## Prerequisites

Before starting, understand:
1. `foundation_core::valtron` async patterns
2. SQLite basics and libsql/Turso
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
   - Add dependencies: `libsql`, `serde`, `serde_json`, `zeroize`, `thiserror`, `tokio`

3. **Create lib.rs**
   - Create: `backends/foundation_db/src/lib.rs`
   - Declare modules: `storage_provider`, `backends`, `schema`, `crypto`, `errors`

### Phase 2: Core Traits

4. **Create storage_provider.rs**
   - Create: `backends/foundation_db/src/storage_provider.rs`
   - Define `StorageProvider` trait with `new(backend) -> Self`
   - Define `StorageBackend` enum: `Turso { url }`, `D1 { binding }`, `R2 { bucket }`, `Memory`
   - Define `KeyValueStore` trait: `get()`, `set()`, `delete()`, `exists()`
   - Define `BlobStore` trait: `put_blob()`, `get_blob()`, `delete_blob()`
   - Define `QueryStore` trait: `query()`, `execute()`

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

### Phase 4: Turso Backend

7. **Create turso.rs**
   - Create: `backends/foundation_db/src/backends/turso.rs`
   - Implement `TursoStorage` struct with `libsql::Connection`
   - Implement connection function: `TursoStorage::connect(url) -> Result<Self>`
   - Implement `KeyValueStore` trait using SQLite key-value table
   - Implement `QueryStore` trait passthrough to libsql

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

- [ ] `backends/foundation_db/Cargo.toml`
- [ ] `backends/foundation_db/src/lib.rs`
- [ ] `backends/foundation_db/src/storage_provider.rs`
- [ ] `backends/foundation_db/src/errors.rs`
- [ ] `backends/foundation_db/src/backends/mod.rs`
- [ ] `backends/foundation_db/src/backends/memory.rs`
- [ ] `backends/foundation_db/src/backends/turso.rs`
- [ ] `backends/foundation_db/src/schema/mod.rs`
- [ ] `backends/foundation_db/src/schema/migrations.rs`
- [ ] `backends/foundation_db/src/crypto/mod.rs`
- [ ] `backends/foundation_db/tests/integration_tests.rs`

## Key Implementation Details

### StorageProvider Trait
```rust
pub trait StorageProvider: Send + Sync {
    type Config;

    fn new(config: Self::Config) -> impl Future<Output = Result<Self>>;
    fn backend(&self) -> &StorageBackend;
}

pub enum StorageBackend {
    Turso { url: String },
    D1 { binding: String },
    R2 { bucket: String },
    Memory,
}
```

### KeyValueStore Trait
```rust
#[async_trait]
pub trait KeyValueStore {
    async fn get(&self, key: &str) -> Result<Option<Value>>;
    async fn set(&self, key: &str, value: Value) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
}

pub enum Value {
    Bytes(Vec<u8>),
    Text(String),
    Json(serde_json::Value),
}
```

### Turso Connection
```rust
use libsql::{Connection, Database};

pub struct TursoStorage {
    conn: Connection,
    encryption_key: Option<Zeroizing<Vec<u8>>>,
}

impl TursoStorage {
    pub async fn connect(url: &str, auth_token: Option<&str>) -> Result<Self> {
        let db = Database::open(url, auth_token)?;
        let conn = db.connect()?;
        Ok(Self { conn, encryption_key: None })
    }
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

Add to `Cargo.toml`:
```toml
[dependencies]
foundation_core = { workspace = true }
libsql = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
zeroize = { version = "1" }
thiserror = "2.0"
tokio = { version = "1", features = ["rt", "sync"] }
async-trait = "0.1"
chacha20poly1305 = "0.10"
rand = "0.8"
```

---

_Created: 2026-03-20_
