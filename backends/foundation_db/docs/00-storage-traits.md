# Fundamentals 00 — Storage traits and the DocumentStore hierarchy

Zero-to-expert on `foundation_db`'s storage abstraction layer. Every database
backend, KV store, and document store in the platform implements one of these
traits.

---

## 1. The trait hierarchy

```
BlobStore          ← raw bytes (R2, S3, filesystem)
KeyValueStore      ← typed key-value (KV, Redis)
QueryStore         ← SQL-like queries (SQLite, D1, Turso)
RateLimiterStore   ← fixed-window rate limits
     ↓
DocumentStore      ← structured documents with ordering
```

Each level builds on the one below. `DocumentStore` needs `QueryStore`
for SQL backends and `BlobStore` for large blobs.

### Async only

Every store implements **only** the `Async*` traits: Turso, libsql, D1, R2,
`JsonFileStorage`, `StorageProvider`, the Cloudflare D1/KV/R2 Worker stores,
`SqlDocumentStore`, `D1R2DocumentStore`, and the in-memory stores
(`MemoryStorage`, `MemoryJsonStore`, `MemoryDocumentStore`, whose futures are
ready at once). There are no sync storage traits and no sync wrapper (the old
`KeyValueStore`/`QueryStore`/… traits and `Blocking<S>` were removed in spec 50
F04).

```rust
use foundation_db::{KeyValueStore, TursoStorage};

let store = TursoStorage::in_memory().await?;   // or TursoStorage::new("app.db").await? + init_schema()
store.set("k", "v").await?;
```

- Single values are awaited directly; multi-row results are async streams
  (`AsyncStorageItemStream`, `AsyncQueryStream`, `AsyncListStream`) pulled one
  item at a time with `.next().await`.
- Code that must be sync (a `main`, an OS thread) is the only place that waits:
  `block_on_future(fut)` / `fut.block()`, imported behind
  `foundation_core::blocking_boundary!`. That boundary doesn't exist on
  JS-hosted wasm (Cloudflare Workers, browsers), where blocking would freeze the
  isolate; there everything `.await`s.
- Inside a valtron `TaskIterator`, hold the store future as a `FutureTask` and
  poll it from `next_status` (see `StoreStateTask`).

## 2. BlobStore

The simplest trait — store and retrieve raw bytes:

```rust
#[async_trait]
trait BlobStore: Send + Sync {
    async fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;
    async fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;
    async fn delete_blob(&self, key: &str) -> StorageResult<()>;
    async fn blob_exists(&self, key: &str) -> StorageResult<bool>;
}
```

**Backends**: `MemoryStorage`, `JsonFileStorage`, `R2Store` (REST),
`R2WasmStorage` (Workers binding), `D1WasmStorage`, `KVWasmStorage`.

## 3. KeyValueStore

Typed key-value with serialization:

```rust
#[async_trait]
trait KeyValueStore: Send + Sync {
    async fn get<V: DeserializeOwned + Send + 'static>(&self, key: &str) -> StorageResult<Option<V>>;
    async fn set<V: Serialize + Send + 'static>(&self, key: &str, value: V) -> StorageResult<()>;
    async fn delete(&self, key: &str) -> StorageResult<()>;
    async fn exists(&self, key: &str) -> StorageResult<bool>;
    async fn list_keys(&self, prefix: Option<&str>) -> StorageResult<AsyncListStream>;
}
```

**Backends**: `MemoryStorage`, `MemoryJsonStore`, `JsonFileStorage`,
`TursoStorage`, `LibsqlStore`, `D1Store`, `KVWasmStorage`, `D1WasmStorage`.

## 4. QueryStore

SQL-like queries with typed parameters and streamed result rows:

```rust
#[async_trait]
trait QueryStore: Send + Sync {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream>;
    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;
    async fn execute_batch(&self, sql: &str) -> StorageResult<()>;
}
```

**`DataValue`** — the parameter type, representing SQL values:
- `Text(String)` — VARCHAR/TEXT
- `Integer(i64)` — INTEGER
- `Blob(Vec<u8>)` — BLOB
- `Real(f64)` — REAL
- `Null` — NULL

**`SqlRow`** — a result row with positional or named access:
```rust
let name: String = row.get(0)?;
let count: i64 = row.get_by_name("count")?;
```

Turso and libsql stream rows as they are fetched; D1's REST API returns the
whole result set, which the stream then yields one row at a time.

**Backends**: `TursoStorage`, `LibsqlStore`, `D1Store`, `D1WasmStorage`.

## 5. RateLimiterStore

Fixed-window counters (`check_rate_limit`, `record_rate_limit`,
`reset_rate_limit`). The window length comes from the caller's
configuration.

## 6. DocumentStore

The highest-level trait — structured documents with ordering, namespaces, and
promoted columns:

```rust
#[async_trait]
trait DocumentStore: Send + Sync {
    async fn append<V: Serialize + Send + 'static>(
        &self, key: &str, content: V) -> StorageResult<Document>;
    async fn scan<V: DeserializeOwned + Send + 'static>(
        &self, key: &str, limit: usize) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn scan_from<V: DeserializeOwned + Send + 'static>(
        &self, key: &str, from_id: &str, limit: usize) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn scan_all<V: DeserializeOwned + Send + 'static>(
        &self, key: &str) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn delete(&self, key: &str, doc_id: &str) -> StorageResult<()>;
    async fn delete_all(&self, key: &str) -> StorageResult<u64>;
    async fn count(&self, key: &str) -> StorageResult<u64>;
    fn config(&self) -> &VectorStoreConfig; // (shared config pattern)
}
```

**Key concepts**:
- **Namespace (`key`)** — documents are grouped into named collections
- **`doc_id`** — scru128 IDs (time-sortable, globally unique)
- **`scan_from`** — ordered range scan from a specific doc_id (inclusive)
- **Promoted columns** — `title`, `summary`, `record_type` stored as real SQL
  columns for filtering without parsing the JSON blob
- **R2 offload** — documents > 4KB stored in R2 with D1 keeping the `r2_key`

**Backends**:
- `MemoryDocumentStore` — in-memory (tests)
- `SqlDocumentStore<Q>` — generic over any `QueryStore` (F22b)
- `D1R2DocumentStore<Q, B>` — D1 + R2 with offload (F23)
- `FjallDocumentStore` — fjall key-value with NDJSON (F22)
- `VectorStore` implementations use DocumentStore patterns for vector metadata

## 7. Migration system

Database schema changes are managed via `MigrationRunner`:

```rust
pub static MIGRATIONS: &[Migration] = &[
    Migration { id: "001_create_users", name: "...", sql: include_str!("001.sql") },
    Migration { id: "002_add_email", name: "...", sql: include_str!("002.sql") },
];

MigrationRunner::new(MIGRATIONS).run_async(&store).await?;
```

There is no sync runner: a sync entry point awaits `run_async` through
`block_on_future` / `.block()` behind `blocking_boundary!`.

Expired auth rows are removed with `StorageCleanup` (implemented for
every `QueryStore`), with retention periods from `CleanupConfig`:

```rust
let stats = store.run_full_cleanup(&CleanupConfig::default()).await?;
```

Migrations are idempotent (tracked by `id` in a `_migrations` table). Each
backend runs its own migrations — `foundation_db` owns core migrations, each
feature crate owns its own.

## 8. The StorageProvider (unified backend)

`StorageProvider` implements the async KV, query, rate-limit and blob traits,
routing to the selected backend:

```rust
let provider = StorageProvider::new(config).await?;

// As KeyValueStore
provider.set("session:123", session_data).await?;

// As QueryStore
let rows = provider
    .query("SELECT * FROM users WHERE id = ?", &[DataValue::Text("123".into())])
    .await?
    .collect_all()
    .await?;

```

## 9. Target gating

| Backend | Native | Wasm | Feature |
|---|---|---|---|
| MemoryStorage | ✓ | ✓ | default |
| TursoStorage | ✓ | ✗ | `turso` |
| LibsqlStore | ✓ | ✗ | `libsql` |
| D1WasmStorage | ✗ | ✓ | `wasm-bindgen-storage` |
| KVWasmStorage | ✗ | ✓ | `wasm-bindgen-storage` |
| R2WasmStorage | ✗ | ✓ | `wasm-bindgen-storage` |
| FjallDocumentStore | ✓ | ✗ | `fjall` |
