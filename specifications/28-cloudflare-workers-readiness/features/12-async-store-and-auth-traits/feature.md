---
feature: "Async Store Traits and Auth Async Methods"
description: "Add async trait variants for KeyValueStore, BlobStore, RateLimiterStore, CredentialStore, SessionManager, and NativeOAuth, enabling native .await usage in async contexts. Uses #[async_trait(?Send)] for wasm compatibility, with the valtron stream-to-future bridge for native async implementations."
status: "proposed"
priority: "high"
depends_on:
  - "11-valtron-async-bridge"
estimated_effort: "large"
created: 2026-05-20
last_updated: 2026-05-20
author: "Main Agent"
---

# Feature: Async Store Traits and Auth Async Methods

## Overview

Add async trait variants for storage and authentication types, enabling native `.await` usage in async contexts (CF Workers, axum servers, etc.). This extends the pattern established by `AsyncQueryStore` to cover all storage operations and auth operations.

**Two categories of additions:**

1. **Async storage traits** — `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore` mirroring their sync counterparts, returning `Result<T, StorageError>` directly (no streams).
2. **Async auth methods** — `_async` variants on `SessionManager` and `NativeOAuth` for native async token exchange and session management.

## Problem Statement

Currently, all storage and auth operations are synchronous. In async contexts:

- **CF Workers**: Wasm backends like `D1WasmStorage` use `futures_lite::block_on` to resolve JS Promises inside sync trait methods. This works because CF Workers have no real thread pool, but it's semantically wrong — blocking inside an async handler.
- **Native async servers** (axum, actix): Calling sync storage blocks the async runtime thread, reducing throughput.
- **Composition**: Async code cannot compose storage operations with other futures using `.await`.

The valtron stream-to-future bridge (feature 11) provides the mechanism to convert sync `StreamIterator`-based operations into `Future`s. This feature defines the async traits that expose them.

## Design Principles

### 1. `#[async_trait(?Send)]` for wasm compatibility

All async traits use `#[async_trait::async_trait(?Send)]` — not `Send`. This is required because wasm32 futures are `!Send` (single-threaded). Native implementations can still implement these traits; `?Send` only relaxes the bound, it doesn't require `!Send`.

### 2. Return `Result<T>`, not streams

Async trait methods return `Result<T, StorageError>` directly — not `StorageItemStream`. The async bridge is for callers that want `.await`. Callers that want streams should use the sync traits.

### 3. Wasm implementations call JS async APIs directly

Wasm backends (like `D1WasmStorage`) already have `_async` private methods that resolve `JsFuture`. The async trait impl calls these directly — no valtron bridge needed.

### 4. Native implementations use valtron bridge or native async

For native backends, two strategies:

- **HTTP-based backends** (D1 REST API via SimpleHttpClient): Use `schedule_future` + `StreamReadyFuture` bridge to convert sync HTTP into async.
- **Native async HTTP** (reqwest, etc.): Implement truly async methods when available.

### 5. Sync traits remain the default

Sync traits (`KeyValueStore`, `BlobStore`, etc.) remain the primary interface. Async traits are opt-in additions for async contexts. No breaking changes to existing implementations.

### 6. Why `#[async_trait(?Send)]` over native async traits

Rust 1.75+ supports native async trait methods (RPITIT), but this codebase uses `#[async_trait::async_trait(?Send)]` for two reasons:

- **Consistency with `AsyncQueryStore`** — the existing async trait already uses this macro. Mixing native and macro-based async traits creates an inconsistent API surface.
- **`?Send` control** — the macro explicitly makes futures `!Send`, which is required for wasm32 where all code runs on a single thread. Native async traits infer `Send` from bounds, which can cause compile errors on wasm if a trait method happens to be used in a `Send`-requiring context.

Native async traits can be adopted once the entire async surface is migrated together — not piecemeal.

## Architecture

### Async Storage Traits

Mirroring the existing sync traits in `foundation_db/src/core/storage_provider.rs`:

```rust
// storage_provider.rs — new traits

/// Async key-value store operations.
#[async_trait::async_trait(?Send)]
pub trait AsyncKeyValueStore {
    async fn get_async<V: DeserializeOwned + Send + 'static>(
        &self,
        key: &str,
    ) -> StorageResult<Option<V>>;

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> StorageResult<()>;

    async fn delete_async(&self, key: &str) -> StorageResult<()>;

    async fn exists_async(&self, key: &str) -> StorageResult<bool>;

    async fn list_keys_async(&self, prefix: Option<&str>) -> StorageResult<Vec<String>>;
}

/// Async blob storage operations.
#[async_trait::async_trait(?Send)]
pub trait AsyncBlobStore {
    async fn put_blob_async(&self, key: &str, data: &[u8]) -> StorageResult<()>;
    async fn get_blob_async(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;
    async fn delete_blob_async(&self, key: &str) -> StorageResult<()>;
    async fn blob_exists_async(&self, key: &str) -> StorageResult<bool>;
}

/// Async rate limiting operations.
#[async_trait::async_trait(?Send)]
pub trait AsyncRateLimiterStore {
    async fn check_rate_limit_async(
        &self,
        key: &str,
        max_count: u32,
        window_seconds: u64,
    ) -> StorageResult<bool>;

    async fn record_rate_limit_async(&self, key: &str) -> StorageResult<u32>;
    async fn reset_rate_limit_async(&self, key: &str) -> StorageResult<()>;
}
```

### Async Auth Traits

In `foundation_auth/src/shared/credential_store.rs`:

```rust
/// Async credential storage API.
#[async_trait::async_trait(?Send)]
pub trait AsyncCredentialStore {
    async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
        &self,
        key: &str,
    ) -> Result<Option<V>, CredentialStoreError>;

    async fn set_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> Result<(), CredentialStoreError>;

    async fn delete_async(&self, key: &str) -> Result<(), CredentialStoreError>;
    async fn exists_async(&self, key: &str) -> Result<bool, CredentialStoreError>;
    async fn list_keys_async(&self, prefix: Option<&str>) -> Result<Vec<String>, CredentialStoreError>;
}
```

### SessionManager Async Methods

In `foundation_auth/src/shared/session.rs`, add `_async` methods to the existing `impl<S: CredentialStore>` block. These methods mirror the sync versions but use `AsyncCredentialStore` when available:

```rust
impl<S: CredentialStore + AsyncCredentialStore + Send + Sync> SessionManager<S> {
    pub async fn create_session_async(
        &self,
        user_id: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(Session, Vec<Cookie>), SessionError> {
        // Same logic as create_session, but calls store.set_async() etc.
    }

    pub async fn get_session_async(
        &self,
        token: &str,
    ) -> Result<Option<Session>, SessionError> {
        // Same logic as get_session, but calls store.get_async(), list_keys_async()
    }

    pub async fn revoke_session_async(&self, session_id: &str) -> Result<(), SessionError>;
    pub async fn revoke_all_sessions_async(&self, user_id: &str) -> Result<usize, SessionError>;
}
```

**Design decision**: Rather than duplicating all session logic, the sync methods can internally delegate to async methods via `futures_lite::block_on` on native, or vice versa. The cleanest approach:

- **Sync methods** call sync `CredentialStore` (current behavior, unchanged).
- **Async methods** call `AsyncCredentialStore` directly.

This avoids any `block_on` in async contexts and keeps both paths clean.

### NativeOAuth Async Methods

In `foundation_auth/src/native/oauth.rs`, add async variants that use the valtron stream-to-future bridge:

```rust
impl NativeOAuth {
    /// Async version of exchange_code.
    pub async fn exchange_code_async(
        &self,
        code: &str,
        code_verifier: Option<&str>,
    ) -> Result<OAuthToken, OAuthError> {
        // Use schedule_future + StreamReadyFuture bridge
        let config = self.inner.config.clone();
        let code = code.to_string();
        let code_verifier = code_verifier.map(String::from);

        let future = async move {
            // ... same logic as exchange_code, but async ...
            // Uses foundation_core async HTTP client or reqwest
            Ok::<_, OAuthError>(token)
        };

        // Bridge via valtron stream-to-future
        let stream = schedule_future(future)
            .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?;

        stream
            .into_ready_future()
            .await
            .ok_or(OAuthError::TokenRequestFailed("no result".into()))?
    }

    pub async fn client_credentials_async(...) -> Result<OAuthToken, OAuthError>;
    pub async fn refresh_token_async(...) -> Result<OAuthToken, OAuthError>;
}
```

**Alternative**: Since `foundation_core::wire::simple_http::client::SimpleHttpClient` is sync, we need an async HTTP client for native. Options:

1. **Add `reqwest` as optional dependency** for native async OAuth — cleanest, truly async.
2. **Use `schedule_future`** to wrap the sync HTTP call into a valtron Future — this runs on the valtron thread pool, providing concurrency without true async I/O.

Option 2 is preferred for consistency — it uses the existing valtron infrastructure and avoids adding new dependencies. The valtron thread pool provides the concurrency benefit even though the HTTP call itself is blocking.

## Implementation Strategy

### Phase 1: Async Storage Traits

**Files to modify:**

1. `backends/foundation_db/src/core/storage_provider.rs` — add `AsyncKeyValueStore`, `AsyncBlobStore`, `AsyncRateLimiterStore` traits.

2. `backends/foundation_db/src/core/backends/d1_kvstore.rs` — implement async traits for `D1KeyValueStore`:
   ```rust
   #[async_trait::async_trait(?Send)]
   impl AsyncKeyValueStore for D1KeyValueStore {
       async fn get_async<V: DeserializeOwned + Send + 'static>(
           &self,
           key: &str,
       ) -> StorageResult<Option<V>> {
           // Use schedule_future to wrap the existing sync get() call
           let key = key.to_string();
           let store = &self; // clone Arc or borrow
           let future = async move {
               // This runs on valtron thread pool
               let stream = store.get::<V>(&key)?;
               stream.flat_map(|s| match s {
                   Stream::Next(r) => vec![r],
                   _ => vec![],
               }).next().ok_or_else(|| StorageError::NotFound(key.clone()))
           };
           schedule_future(future)?
               .into_ready_future()
               .await
               .ok_or_else(|| StorageError::NotFound(key))?
       }
       // ... other methods
   }
   ```

   **Simpler approach**: Since `D1KeyValueStore` already has working sync methods, the async impl can just delegate:
   ```rust
   #[async_trait::async_trait(?Send)]
   impl AsyncKeyValueStore for D1KeyValueStore {
       async fn get_async<V: DeserializeOwned + Send + 'static>(
           &self,
           key: &str,
       ) -> StorageResult<Option<V>> {
           // D1 REST API is HTTP-based; no native async available.
           // Use valtron thread pool for concurrency.
           let key = key.to_string();
           let this = Arc::clone(&self.shared);
           let future = async move {
               Self::get_impl(&this, &key)
           };
           // ... bridge
       }
   }
   ```

   Actually, the simplest correct approach: factor out the core logic from sync methods into shared private methods, then both sync and async trait impls call them.

3. `backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs` — implement async traits for `D1WasmStorage`:
   ```rust
   #[async_trait::async_trait(?Send)]
   impl AsyncKeyValueStore for D1WasmStorage {
       async fn get_async<V: DeserializeOwned + Send + 'static>(
           &self,
           key: &str,
       ) -> StorageResult<Option<V>> {
           // Call existing private async method directly — no bridge needed
           self.get_async(key).await
       }
       // ... etc. (maps to existing get_async, set_async, etc.)
   }
   ```

   `D1WasmStorage` already has `get_async`, `set_async`, etc. as private methods. The trait impl simply makes them public through the trait.

### Phase 2: Async Auth Traits

**Files to modify:**

1. `backends/foundation_auth/src/shared/credential_store.rs` — add `AsyncCredentialStore` trait.

2. `backends/foundation_auth/src/shared/credential_store.rs` — implement for `CredentialStorage`:
   ```rust
   #[async_trait::async_trait(?Send)]
   impl AsyncCredentialStore for CredentialStorage {
       async fn get_async<V: for<'de> Deserialize<'de> + Send + 'static>(
           &self,
           key: &str,
       ) -> Result<Option<V>, CredentialStoreError> {
           // If underlying provider supports async, use it.
           // Otherwise, bridge via schedule_future + StreamReadyFuture.
           let key = key.to_string();
           let provider = self.provider().clone();
           let future = async move {
               let stream = provider.get(&key).map_err(|e| match e {
                   StorageError::NotFound(_) => CredentialStoreError::NotFound(key.clone()),
                   other => CredentialStoreError::Storage(other),
               })?;
               stream.flat_map(|s| match s {
                   Stream::Next(result) => vec![result],
                   _ => vec![],
               }).next()
               .ok_or_else(|| CredentialStoreError::NotFound(key.clone()))?
               .map_err(CredentialStoreError::Storage)
           };
           schedule_future(future)
               .map_err(CredentialStoreError::Generic)?
               .into_ready_future()
               .await
               .ok_or(CredentialStoreError::NotFound(key))?
       }
       // ...
   }
   ```

   **For wasm**: When `D1WasmStorage` is the underlying storage, the `CredentialStorage` wrapper should have a separate wasm-specific impl that calls `D1WasmStorage`'s async methods directly, bypassing the valtron bridge. This requires a `CredentialStorage` variant that holds `Arc<D1WasmStorage>`.

   **Alternative**: Create `AsyncCredentialStorage` as a separate type that wraps `StorageProvider` or `D1WasmStorage` and implements `AsyncCredentialStore`:
   ```rust
   pub struct AsyncCredentialStorage {
       inner: CredentialStorageInner,
   }

   enum CredentialStorageInner {
       Provider(StorageProvider),      // native — uses valtron bridge
       D1Wasm(Arc<D1WasmStorage>),     // wasm — calls JS async directly
   }
   ```

   This is over-engineered. Simpler: just implement `AsyncCredentialStore` for `CredentialStorage` using the valtron bridge for native, and implement it for `D1WasmStorage` directly for wasm. The caller picks the right type.

### Phase 3: SessionManager Async Methods

**Files to modify:**

1. `backends/foundation_auth/src/shared/session.rs` — add async methods gated on `S: AsyncCredentialStore`:
   ```rust
   impl<S> SessionManager<S>
   where
       S: CredentialStore + AsyncCredentialStore + Send + Sync,
   {
       pub async fn create_session_async(...) -> Result<(Session, Vec<Cookie>), SessionError> {
           // Mirrors create_session but calls self.store.set_async()
       }
       pub async fn get_session_async(&self, token: &str) -> Result<Option<Session>, SessionError> {
           // Mirrors get_session but calls self.store.get_async(), list_keys_async()
       }
       pub async fn revoke_session_async(&self, session_id: &str) -> Result<(), SessionError>;
       pub async fn revoke_all_sessions_async(&self, user_id: &str) -> Result<usize, SessionError>;
   }
   ```

   **Critical**: `get_session` scans all sessions via `list_keys` + individual `get` calls. The async version should parallelize these using `futures_lite::future::join_all` or `futures::future::join_all`:
   ```rust
   pub async fn get_session_async(&self, token: &str) -> Result<Option<Session>, SessionError> {
       // Check cache first (same as sync)
       let cache = Self::get_cached_session(token);
       // ...

       // Parallel scan using async trait
       let keys = self.store.list_keys_async(Some("session:")).await
           .map_err(SessionError::Storage)?;

       // Check all sessions in parallel
       let futures: Vec<_> = keys.iter().map(|key| {
           let store = &self.store;
           let signer = &self.signer;
           let token = token.to_string();
           async move {
               store.get_async::<Session>(key).await
                   .map_err(SessionError::Storage)
                   .map(|opt| opt.filter(|s| {
                       !s.revoked && signer.lock().unwrap().verify(&s.token.get(), &token).unwrap_or(false)
                   }))
           }
       }).collect();

       for result in futures_lite::future::join_all(futures).await {
           if let Ok(Some(session)) = result {
               if session.is_valid() {
                   if self.config.sliding_expiration {
                       self.extend_session_async(&session).await?;
                   }
                   return Ok(Some(session));
               }
               return Ok(None);
           }
       }
       Ok(None)
   }
   ```

### Phase 4: NativeOAuth Async Methods

**Files to modify:**

1. `backends/foundation_auth/src/native/oauth.rs` — add `exchange_code_async`, `client_credentials_async`, `refresh_token_async`.

   **Implementation approach**: Factor out the request-building and response-parsing logic into shared private functions, then both sync and async methods call them. The async version uses `schedule_future` + valtron bridge:

   ```rust
   impl NativeOAuth {
       pub async fn exchange_code_async(
           &self,
           code: &str,
           code_verifier: Option<&str>,
       ) -> Result<OAuthToken, OAuthError> {
           self.inner.config.validate()?;

           let body = build_exchange_code_body(
               &self.inner.config, code, code_verifier
           );
           let token_url = self.inner.config.token_url.clone();

           let future = async move {
               let client = SimpleHttpClient::from_system();
               let response = client.post(&token_url)?
                   .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                   .body_text(body)
                   .build_client()?
                   .send()?;
               parse_oauth_response(response)
           };

           schedule_future(future)
               .map_err(|e| OAuthError::TokenRequestFailed(e.to_string()))?
               .into_ready_future()
               .await
               .ok_or(OAuthError::TokenRequestFailed("no result".into()))?
       }
   }
   ```

   **Refactoring**: Extract shared logic:
   - `build_exchange_code_body(config, code, code_verifier) -> String`
   - `build_client_credentials_body(config, scopes) -> String`
   - `build_refresh_token_body(config, refresh_token) -> String`
   - `parse_oauth_response(response: Response) -> Result<OAuthToken, OAuthError>`
   - `parse_refresh_response(response: Response, old_refresh: &str) -> Result<OAuthToken, OAuthError>`

   Both sync and async methods call these shared helpers.

### Phase 5: WasmOAuth Alignment

`WasmOAuth` already has `_async` methods. Ensure the sync methods continue to use `futures_lite::block_on` (current behavior). No changes needed, but verify consistency.

## Module Layout

```
backends/foundation_db/src/core/storage_provider.rs
  ├── KeyValueStore (existing, unchanged)
  ├── BlobStore (existing, unchanged)
  ├── QueryStore (existing, unchanged)
  ├── AsyncQueryStore (existing, unchanged)
  ├── RateLimiterStore (existing, unchanged)
  ├── AsyncKeyValueStore (NEW)
  ├── AsyncBlobStore (NEW)
  └── AsyncRateLimiterStore (NEW)

backends/foundation_db/src/core/backends/d1_kvstore.rs
  ├── D1KeyValueStore implements sync traits (unchanged)
  ├── impl AsyncKeyValueStore for D1KeyValueStore (NEW)
  ├── impl AsyncBlobStore for D1KeyValueStore (NEW)
  └── impl AsyncRateLimiterStore for D1KeyValueStore (NEW)

backends/foundation_db/src/wasm/wasm_storage/d1_wasm.rs
  ├── D1WasmStorage implements sync traits (unchanged)
  ├── impl AsyncQueryStore (existing, unchanged)
  ├── impl AsyncKeyValueStore for D1WasmStorage (NEW)
  ├── impl AsyncBlobStore for D1WasmStorage (NEW)
  └── impl AsyncRateLimiterStore for D1WasmStorage (NEW)

backends/foundation_auth/src/shared/credential_store.rs
  ├── CredentialStore (existing, unchanged)
  ├── CredentialStorage (existing, unchanged)
  ├── AsyncCredentialStore (NEW trait)
  └── impl AsyncCredentialStore for CredentialStorage (NEW)

backends/foundation_auth/src/shared/session.rs
  ├── SessionManager sync methods (unchanged)
  └── SessionManager async methods (NEW, gated on S: AsyncCredentialStore)

backends/foundation_auth/src/native/oauth.rs
  ├── NativeOAuth sync methods (unchanged)
  └── NativeOAuth async methods (NEW)
```

## Testing

### Sync tests (unchanged)

All existing sync tests continue to pass. No breaking changes to sync traits.

### Async tests

New tests for each async trait:

```rust
// foundation_db tests
#[tokio::test]
async fn test_async_kv_store_roundtrip() {
    let store = D1WasmStorage::new(db, "test");
    store.set_async("key", "value").await.unwrap();
    let got: Option<String> = store.get_async("key").await.unwrap();
    assert_eq!(got, Some("value".to_string()));
}

#[tokio::test]
async fn test_async_blob_store_roundtrip() {
    let store = D1WasmStorage::new(db, "test");
    let data = b"hello binary world";
    store.put_blob_async("blob1", data).await.unwrap();
    let got = store.get_blob_async("blob1").await.unwrap();
    assert_eq!(got, Some(data.to_vec()));
}

// foundation_auth tests
#[tokio::test]
async fn test_async_credential_store() {
    let store = CredentialStorage::memory();
    store.set_async::<String>("k", "v").await.unwrap();
    let got: Option<String> = store.get_async("k").await.unwrap();
    assert_eq!(got, Some("v".to_string()));
}

#[tokio::test]
async fn test_session_manager_async() {
    let mgr = make_manager_async().await;
    let (session, cookies) = mgr.create_session_async("user_1", None, None).await.unwrap();
    assert!(!session.token.get().is_empty());
    assert!(!cookies.is_empty());
}
```

### Integration tests

- `test_async_then_sync_consistency` — write via async, read via sync, verify same value.
- `test_sync_then_async_consistency` — write via sync, read via async, verify same value.
- `test_parallel_async_operations` — fire multiple async ops in parallel, verify all complete.

## Dependency Impact

- `async-trait` already a dependency (used by `AsyncQueryStore`).
- `tokio` and `smol` already dev-dependencies (from feature 11).
- No new runtime dependencies.

## Migration Path for Existing Code

Existing code using sync traits is unaffected. Migration to async is opt-in:

```rust
// Before (sync)
let user = store.get::<User>("user:1")?;

// After (async, opt-in)
let user = store.get_async::<User>("user:1").await?;
```

For wasm backends, prefer async in async contexts (CF Workers handlers). For native backends, async provides concurrency via valtron thread pool but the underlying I/O may still be blocking.

## Key Learnings

### ?Send vs Send in async_trait

`#[async_trait(?Send)]` allows the future to be `!Send`. This is required for wasm32 where all code runs on a single thread. Native implementations can still implement `?Send` traits — the bound is relaxed, not restricted.

### Valtron bridge for "fake async"

When a backend has no native async capability (e.g., `SimpleHttpClient`), the valtron stream-to-future bridge (`schedule_future` + `StreamReadyFuture`) provides a way to expose async methods. The work runs on the valtron thread pool — concurrent with other tasks, but not truly async I/O. This is still valuable for throughput in async servers.

### Refactor before duplicating

Adding `_async` methods that duplicate sync logic leads to drift. Factor out shared helpers (request building, response parsing, business logic) first, then both sync and async methods call them with different execution wrappers.

### Parallel scan in get_session_async

`get_session` does a linear scan of all session keys. The async version should parallelize `get_async` calls using `join_all` or similar. This is a real performance improvement over the sync version.

---

_Created: 2026-05-20_
