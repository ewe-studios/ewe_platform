# Decision 02: Storage — foundation_db Traits Only

**Status:** Resolved (2026-07-18)

## Problem

The keychain needs database, KV, and blob storage. Options:

1. **Use foundation_db's existing traits** — `QueryStore`, `KeyValueStore`, `BlobStore`, `RateLimiterStore`, `StorageProvider`
2. **Define custom traits** — `KeychainStore`, `BlobStore`, etc.

## Analysis

`foundation_db` already provides everything:
- `QueryStore` / `AsyncQueryStore` — `query()`, `execute()`, `execute_batch()` with `DataValue` params, `SqlRow` results
- `KeyValueStore` / `AsyncKeyValueStore` — `get`, `set`, `delete`, `exists`, `list_keys`
- `BlobStore` / `AsyncBlobStore` — `put_blob`, `get_blob`, `delete_blob`, `blob_exists`
- `RateLimiterStore` / `AsyncRateLimiterStore` — `check_rate_limit`, `record_rate_limit`, `reset_rate_limit`
- `StorageProvider` — unified enum over all backends (Turso, Libsql, D1Wasm, R2Wasm, KVWasm, JsonFile, Memory)
- Backends: `D1Wasm` (Cloudflare), `R2Wasm` (Cloudflare), `KVWasm` (Cloudflare), `Turso` (native), `Libsql` (native), `JsonFile` (native)

Defining custom traits would duplicate ~500 lines of trait definitions and require mapping between the two.

## Decision: Use foundation_db traits directly

Zero custom storage traits. Keychain query functions use `&dyn QueryStore`. KV operations use `&dyn KeyValueStore`. Blob operations use `&dyn BlobStore`. Rate limiting uses `&dyn RateLimiterStore`. Backend selection uses `StorageProvider::new(StorageBackend::...)`.

Row parsing follows the pattern in `foundation_auth::server::storage.rs`: iterate `StorageItemStream<SqlRow>`, extract columns with `row.get_by_name::<T>()`, map into domain structs.

## Consequences

- No trait duplication
- All backends work out of the box (D1Wasm, R2Wasm, KVWasm for Cloudflare; Turso/Libsql for native)
- Row parsing is more explicit than OrangeVault's `serde::Deserialize` approach — but matches `foundation_auth`'s existing pattern
