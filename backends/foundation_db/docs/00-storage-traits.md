# Fundamentals 00 — Storage traits and the DocumentStore hierarchy

Zero-to-expert on `foundation_db`'s storage abstraction layer. Every database
backend, KV store, and document store in the platform implements one of these
traits.

---

## 1. The trait hierarchy

```
BlobStore / AsyncBlobStore          ← raw bytes (R2, S3, filesystem)
KeyValueStore / AsyncKeyValueStore  ← typed key-value (KV, Redis)
QueryStore / AsyncQueryStore        ← SQL-like queries (SQLite, D1, Turso)
     ↓
DocumentStore / AsyncDocumentStore  ← structured documents with ordering
```

Each level builds on the one below. `AsyncDocumentStore` needs `AsyncQueryStore`
for SQL backends and `AsyncBlobStore` for large blobs.

## 2. BlobStore

The simplest trait — store and retrieve raw bytes:

```rust
trait BlobStore: Send + Sync {
    fn put_blob(&self, key: &str, data: &[u8]) -> StorageResult<()>;
    fn get_blob(&self, key: &str) -> StorageResult<Option<Vec<u8>>>;
    fn delete_blob(&self, key: &str) -> StorageResult<()>;
    fn blob_exists(&self, key: &str) -> StorageResult<bool>;
}
```

**Backends**:
- `MemoryStorage` — in-memory HashMap (tests, dev)
- `R2WasmStorage` — Cloudflare R2 via wasm-bindgen
- `JsonFileStorage` — JSON files on disk

## 3. KeyValueStore

Typed key-value with serialization:

```rust
trait KeyValueStore: Send + Sync {
    fn get<V: DeserializeOwned>(&self, key: &str) -> StorageResult<Option<V>>;
    fn set<V: Serialize>(&self, key: &str, value: V) -> StorageResult<()>;
    fn delete(&self, key: &str) -> StorageResult<()>;
    fn exists(&self, key: &str) -> StorageResult<bool>;
    fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>>;
}
```

**Backends**:
- `MemoryStorage` (also implements this)
- `KVWasmStorage` — Cloudflare KV via wasm-bindgen
- `D1WasmStorage` — D1 SQL used as KV table

## 4. QueryStore

SQL-like queries with typed parameters and result rows:

```rust
trait QueryStore: Send + Sync {
    fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<StorageItemStream<'_, SqlRow>>;
    fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64>;
    fn execute_batch(&self, sql: &str) -> StorageResult<()>;
}
```

**`DataValue`** — the parameter type, representing SQL values:
- `Text(String)` — VARCHAR/TEXT
- `Integer(i64)` — INTEGER
- `Blob(Vec<u8>)` — BLOB
- `Float(f64)` — REAL
- `Null` — NULL

**`SqlRow`** — a result row with positional access:
```rust
let name: String = row.get(0)?;
let count: i64 = row.get(1)?;
let data: Option<String> = row.get(2)?;
```

**Backends**:
- `TursoStorage` — Turso/libSQL embedded SQLite
- `LibsqlStore` — libSQL with remote sync
- `D1WasmStorage` — Cloudflare D1 via wasm-bindgen

## 5. AsyncQueryStore

The async version — the canonical form for wasm/CF Workers:

```rust
#[async_trait]
trait AsyncQueryStore: Send + Sync {
    async fn query_async(&self, sql: &str, params: &[DataValue])
        -> StorageResult<AsyncQueryStream>;
    async fn execute_async(&self, sql: &str, params: &[DataValue])
        -> StorageResult<u64>;
    async fn execute_batch_async(&self, sql: &str) -> StorageResult<()>;
}
```

**The `?Send` resolution (F00e)**: `AsyncQueryStore` is a single `Send` async
trait. On wasm, `!Send` futures (JS Promises) are wrapped in `SendWrapper` to
satisfy the `Send` bound safely on single-threaded targets.

## 6. DocumentStore / AsyncDocumentStore

The highest-level trait — structured documents with ordering, namespaces, and
promoted columns:

```rust
#[async_trait]
trait AsyncDocumentStore: Send + Sync {
    async fn append_async<V: Serialize + Send + 'static>(
        &self, key: &str, content: V) -> StorageResult<Document>;
    async fn scan_async<V: DeserializeOwned + Send + 'static>(
        &self, key: &str, limit: usize) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn scan_from_async<V: DeserializeOwned + Send + 'static>(
        &self, key: &str, from_id: &str, limit: usize) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn scan_all_async<V: DeserializeOwned + Send + 'static>(
        &self, key: &str) -> StorageResult<AsyncStorageItemStream<'_, V>>;
    async fn delete_async(&self, key: &str, doc_id: &str) -> StorageResult<()>;
    async fn delete_all_async(&self, key: &str) -> StorageResult<u64>;
    async fn count_async(&self, key: &str) -> StorageResult<u64>;
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
- `AsyncSqlDocumentStore<Q>` — generic over any `AsyncQueryStore` (F22b)
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

MigrationRunner::new(MIGRATIONS).run(&store)?;
```

Migrations are idempotent (tracked by `id` in a `_migrations` table). Each
backend runs its own migrations — `foundation_db` owns core migrations, each
feature crate owns its own.

## 8. The StorageProvider (unified backend)

`StorageProvider` implements all traits simultaneously, routing to the
appropriate backend:

```rust
let provider = StorageProvider::new(config)?;

// As KeyValueStore
provider.set("session:123", &session_data)?;

// As QueryStore
let rows = provider.query("SELECT * FROM users WHERE id = ?", &[DataValue::Text("123")])?;

// As DocumentStore (via AsyncDocumentStore + valtron bridge)
let doc = provider.append_async("messages", &message).await?;
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
