# Learnings: Foundation DB (Feature 00a)

## Overview

Lessons learned from implementing foundation_db's unified storage backend with Turso, libsql, and in-memory backends using Valtron-only async patterns.

## Wrapping Async Libraries with Valtron's `from_future` + `execute`

**IMPORTANT:** Read `.agents/skills/rust-valtron-usage/skill.md` for the full Valtron usage guidance.

### The Core Problem

Both Turso and libsql expose **async-only APIs**, but Iron Law 1 bans tokio/async-trait. The solution is Valtron's `from_future` which converts any `Future` into a `TaskIterator`, then `execute` runs it through Valtron's executor. **Methods should return the stream to the caller, not block immediately.**

### Primary Pattern: Stream-Returning Methods

Storage methods schedule work via `from_future` + `execute` and return the stream:

```rust
fn get<V>(&self, key: &str) -> StorageResult<impl Iterator<Item = Stream<Option<V>, ()>>> {
    let task = from_future(async move { /* ... */ });
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron scheduling failed: {e}")))?;
    Ok(stream)
}
```

Callers decide when to block using `collect_result()` (drains all Next values) or `sync_one()`/`sync_all()` at boundaries.

### Legacy Pattern: `exec_future` (Blocking — Deprecated for Trait Methods)

The original `exec_future` helper blocks immediately at the leaf. It is acceptable for one-shot initialization (DB connection, migrations) but **should not be the default for storage trait methods** as it kills composability and parallelism.

Original helper for reference:

```rust
use foundation_core::valtron::{execute, from_future, Stream};

pub fn exec_future<T, E, F>(future: F) -> Result<T, StorageError>
where
    F: std::future::Future<Output = Result<T, E>> + Send + 'static,
    F::Output: Send + 'static,
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| StorageError::Backend(format!("Valtron execution failed: {e}")))?;
    let result: Result<T, E> = stream
        .into_iter()
        .find_map(|s| match s {
            Stream::Next(v) => Some(v),
            _ => None,
        })
        .ok_or_else(|| StorageError::Generic("No result from future execution".into()))?;
    result.map_err(|e| StorageError::Backend(format!("SQL error: {e}")))
}
```

**Key insight:** This is a generic bridge — any crate with an async API can be used with Valtron through this pattern. The helper handles three error levels: Valtron execution failure, empty stream, and backend error.

### `Send + 'static` Requirement

All types captured by the async block must be `Send + 'static`:
- Clone `Arc<Connection>` before moving into the async block
- Convert `&str` parameters to owned `String` before the async block
- The future itself must be `Send + 'static`

```rust
fn get(&self, key: &str) -> StorageResult<Option<V>> {
    let key = key.to_string();        // Own the data
    let conn = Arc::clone(&self.conn); // Clone the Arc

    let result: Option<String> = exec_future(async move {
        // key and conn are moved in, both Send + 'static
        let mut stmt = conn.prepare("SELECT ...").await?;
        // ...
    })?;
}
```

### `!Send` Row Iterators Must Be Consumed Inside the Async Block

**Critical discovery:** Both `turso::Rows` and `libsql::Rows` are `!Send`. They cannot cross the Valtron execution boundary. All row iteration must happen inside the async block, collecting results into a `Vec<T>` before returning.

```rust
// CORRECT: Collect inside async block
let keys: Vec<String> = exec_future(async move {
    let mut rows = stmt.query([]).await?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next().await? {
        keys.push(row.get(0)?);
    }
    Ok::<_, turso::Error>(keys)  // Vec<String> is Send
})?;

// WRONG: Cannot return Rows from async block
// turso::Rows is !Send and will fail to compile
```

### Turbo-fish Type Annotations for `Ok` in Async Blocks

When the async block's error type can't be inferred, use turbo-fish on `Ok`:

```rust
exec_future(async move {
    conn.execute_batch(&sql).await?;
    Ok::<_, turso::Error>(true)  // Explicit error type needed
})?;
```

## Multi-Value Returns with `StorageItemStream`

### The Pattern

```rust
pub type StorageItemStream<'a, T> = Box<dyn Iterator<Item = Stream<T, ()>> + Send + 'a>;
```

All multi-value operations (list_keys, query) return `StorageResult<StorageItemStream<'_, T>>`. Items are wrapped as `Stream::Next(item)`.

### Building a StorageItemStream

After collecting results from the async block into a Vec:

```rust
fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
    let keys: Vec<String> = exec_future(async move { /* collect all keys */ })?;
    Ok(Box::new(keys.into_iter().map(Stream::Next)))
}
```

### Consuming a StorageItemStream

```rust
let items: Vec<T> = stream
    .filter_map(|s| match s {
        Stream::Next(item) => Some(item),
        _ => None,
    })
    .collect();
```

## Sync Backends Don't Need Valtron

The in-memory backend uses no Valtron at all — direct `Mutex` locks and synchronous HashMap operations. Valtron is only needed when wrapping genuinely async I/O.

```rust
// Memory backend: purely synchronous
fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>> {
    let data = self.data.lock().map_err(|e| StorageError::Backend(...))?;
    match data.get(key) {
        Some(bytes) => Ok(Some(serde_json::from_slice(bytes)?)),
        None => Ok(None),
    }
}
```

Even sync backends return `StorageItemStream` for multi-value methods (consistency with the trait), but create it directly from `Vec::into_iter().map(Stream::Next)`.

## Three-Level Error Handling

Every `exec_future` call handles errors at three levels:

1. **Valtron execution failure** — `execute()` itself fails (runtime/pool issue)
2. **Empty stream** — The future ran but produced no `Stream::Next` item
3. **Backend error** — The future's `Result` was `Err` (SQL error, connection failure)

All three are mapped to `StorageError` variants, preserving the original error message.

## Connection Sharing with `Arc`

Both Turso and libsql connections are wrapped in `Arc<Connection>` to enable cloning into async blocks without lifetime issues. The connection is created once in `new()` and shared across all operations.

## Encryption at Rest with ChaCha20-Poly1305

### Overview

The encryption integration adds transparent encryption/decryption for sensitive data stored in SQL backends. When an `EncryptionKey` is provided to `TursoStorage::with_encryption()`, all values are encrypted before being written to the database and decrypted when read.

### Implementation Pattern

```rust
pub struct TursoStorage {
    conn: Arc<turso::Connection>,
    encryption_key: Option<EncryptionKey>,
}

impl TursoStorage {
    pub fn with_encryption(url: &str, key: Option<EncryptionKey>) -> StorageResult<Self> {
        // ... connection setup ...
        Ok(Self { conn, encryption_key: key })
    }

    fn maybe_encrypt(&self, json_str: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                let encrypted = encrypt(key, json_str.as_bytes())?;
                Ok(STANDARD.encode(&encrypted))  // Base64 for TEXT column
            }
            None => Ok(json_str.to_string()),
        }
    }

    fn maybe_decrypt(&self, stored_value: &str) -> StorageResult<String> {
        match &self.encryption_key {
            Some(key) => {
                let encrypted = STANDARD.decode(stored_value)?;
                let decrypted = decrypt(key, &encrypted)?;
                String::from_utf8(decrypted)
                    .map_err(|e| StorageError::Encryption(...))
            }
            None => Ok(stored_value.to_string()),
        }
    }
}
```

### Key Design Decisions

1. **Optional encryption** - The same storage instance can work with or without encryption based on the `encryption_key` option
2. **Base64 encoding** - Encrypted binary data is base64-encoded for safe storage in TEXT columns
3. **Transparent to callers** - The `get` and `set` methods handle encryption/decryption internally; callers don't need to know
4. **ChaCha20-Poly1305** - AEAD cipher provides both confidentiality and integrity verification

### Testing

Key test scenarios:
- `test_turso_storage_encryption` - Verifies encryption on write, decryption on read, and that stored value differs from plaintext
- `test_turso_storage_encryption_wrong_key` - Verifies that decryption fails with a different key

### Security Considerations

- Keys are zeroized on drop via `EncryptionKey::Drop`
- Each encryption operation uses a random nonce (12 bytes)
- Authenticated encryption (Poly1305 tag) detects tampering
- Key management is the caller's responsibility (store securely, rotate as needed)

---

_Created: 2026-03-28_
_Source: backends/foundation_db/src/backends/ (async_utils.rs, turso_backend.rs, libsql_backend.rs, memory.rs), backends/foundation_db/src/crypto/ (encryption.rs, zeroize.rs)_
