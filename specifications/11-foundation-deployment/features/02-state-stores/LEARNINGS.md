# Learnings: State Stores (Feature 02)

## Implementation Location

The spec says implementation lives in `foundation_db` crate (`backends/foundation_db/`), not in `foundation_deployment`. The deployment crate imports and uses these stores via dependency.

## Valtron Async Patterns Used

### Pattern Selection

| Pattern | Used For | Example |
|---------|----------|---------|
| `schedule_future` | Single-value SQL ops (get, set, delete, count) | `schedule_future(async { conn.execute(...).await })` |
| `run_future_iter` | Multi-value SQL ops with `!Send` row iterators | `run_future_iter(\|\| async { Ok(RowsIterator::new(rows)) })` |
| `exec_future` | One-shot bootstrap only (init, connection setup) | `exec_future(async { Builder::new_local(&path).build().await })` |
| No Valtron | Sync backends (FileStateStore) | Direct `std::fs` ops |

### Stream Type Mapping

The `StateStore` trait uses `ThreadedValue<T, StorageError>` (from `run_future_iter`), not `Stream<T, P>` (from `schedule_future`/`execute`). This requires a conversion layer:

```rust
fn to_state_stream<T>(stream: impl StreamIterator<D = Result<T, StorageError>, P = ()>) -> StateStoreStream<T> {
    Box::new(stream.filter_map(|item| match item {
        Stream::Next(result) => Some(ThreadedValue::Value(result)),
        _ => None,
    }))
}
```

For multi-value ops (list, all, get_batch), we use `map_circuit` + `flat_map_next` to expand Vec results into individual stream items, then convert to `ThreadedValue`.

### `!Send` Row Iterators

`libsql::Rows` and `turso::Rows` are `!Send`. For single-value ops we collect inside the async block and return the result. For streaming large result sets, `run_future_iter` spawns a worker thread that owns the `!Send` iterator.

## Shared SQL Schema

All SQL backends (SQLite, LibSQL, Turso, D1) share the same schema:

```sql
CREATE TABLE IF NOT EXISTS deployment_resources (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    provider TEXT NOT NULL,
    status TEXT NOT NULL,        -- JSON-serialized StateStatus
    environment TEXT,
    config_hash TEXT NOT NULL,
    output TEXT NOT NULL,         -- JSON
    config_snapshot TEXT NOT NULL, -- JSON
    created_at TEXT NOT NULL,     -- RFC 3339
    updated_at TEXT NOT NULL      -- RFC 3339
);
```

Shared helpers (`parse_resource_row`, `state_to_params`, `CREATE_TABLE_SQL`, `UPSERT_SQL`) live in `sqlite.rs` and are reused by `libsql_state.rs`. The Turso backend has its own copies because `turso::Row` and `libsql::Row` are different types.

## SimpleHttpClient API (for R2 and D1)

`SimpleHttpClient` from `foundation_core` is the **only** HTTP client used in this project. No `curl`, no `reqwest`, no other HTTP libraries.

Key API:

```rust
let client = SimpleHttpClient::from_system();
let response = client.get(url)?
    .header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"))
    .send()?;

let status: Status = response.get_status();       // Status enum (OK, NotFound, etc.)
let body: &SendSafeBody = response.get_body_ref(); // SendSafeBody::Text(String) or ::Bytes(Vec<u8>)
```

For task-based (non-blocking) usage with Valtron:

```rust
let request = client.get(url)?.build()?;
let stream = SendRequestTask::new(request, retries, pool, config);
// Use with execute(), combinators, etc.
```

R2 and D1 stores use the simple `.send()` API since responses are small JSON. No Valtron needed — they wrap results in `ThreadedValue` via `Vec::into_iter().map(...)` like `FileStateStore`.

## Error Conventions

- `derive_more::From` + manual `Display` (no `thiserror`)
- All errors map to `StorageError` variants
- Three-level errors for Valtron ops: scheduling failure, empty stream, backend error
- `#[from(ignore)]` on String-wrapping variants

## Visibility

All public items use `pub`, never `pub(super)` or `pub(crate)`. This is a project convention.

---

_Created: 2026-04-05_
_Source: Implementation of state/ module in backends/foundation_db/_
