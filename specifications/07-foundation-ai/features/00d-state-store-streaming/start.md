---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
this_file: "specifications/07-foundation-ai/features/00d-state-store-streaming/start.md"
created: 2026-04-06
---

# Start: State Store Streaming - Proper Valtron Iterator Patterns

## Agent Workflow

1. **Read `feature.md`** - Full requirements and implementation details
2. **Read existing correct pattern** - `src/backends/turso_backend.rs::list_keys()` (lines 348-383) shows correct `run_future_iter` usage
3. **Read `src/rows_stream.rs`** - Understand `RowsIterator` and `LibsqlRowsIterator`
4. **Audit all files** - Identify all methods that collect to Vec incorrectly (see Files to Modify below)
5. **Implement fixes** - One file at a time, starting with `libsql_backend.rs::list_keys()`
6. **Test each change** - Verify compilation, clippy, and tests after each file
7. **Update LEARNINGS.md** - Document patterns and discoveries

## Prerequisites

Before starting, ensure you understand:
- `foundation_core::valtron::run_future_iter` - How it wraps async futures into streams
- `ThreadedValue<T>` - The type that crosses thread boundaries
- `Stream<T, P>` - The Valtron stream type with progress
- `!Send` iterators - Why `turso::Rows` and `libsql::Rows` can't cross async boundaries

## Files to Modify

### High Priority - Storage Backends

| File | Method | Problem | Fix |
|------|--------|---------|-----|
| `src/backends/libsql_backend.rs` | `list_keys()` (lines 294-342) | Collects keys to `Vec`, then `flat_map_next` to expand | Use `run_future_iter` with `LibsqlRowsIterator`, extract keys in `.map()` |

### High Priority - State Stores

| File | Methods | Problem | Fix |
|------|---------|---------|-------|
| `src/state/d1.rs` | `all()`, `list()`, `get_batch()` | Collects to Vec, wraps as stream | Use `run_future_iter` or stream parsing |
| `src/state/sqlite.rs` | `all()`, `list()`, `query()` | Collects to Vec | Use `run_future_iter` with `RowsIterator` |
| `src/state/libsql_state.rs` | `all()`, `list()`, `query()` | Collects to Vec | Use `run_future_iter` with `LibsqlRowsIterator` |
| `src/state/turso.rs` | `all()`, `list()`, `query()` | Collects to Vec | Use `run_future_iter` with `RowsIterator` |

### Medium Priority - State Stores

| File | Methods | Notes |
|------|---------|-------|
| `src/state/r2.rs` | `all()`, `list()` | R2 supports paginated listing - can do true streaming |

## Files Already Correct (NO CHANGE)

| File | Method | Notes |
|------|--------|-------|
| `src/backends/turso_backend.rs` | `list_keys()`, `query()` | Uses `run_future_iter` correctly |
| `src/backends/libsql_backend.rs` | `query()` | Uses `run_future_iter` correctly |
| `src/backends/memory.rs` | All | Sync backend, no async wrapping needed |
| `src/backends/json_file.rs` | All | Sync backend, no async wrapping needed |

## Implementation Order

1. **`libsql_backend.rs::list_keys()`** - Single method, good warm-up
2. **`src/state/sqlite.rs`** - Simplest state store, rusqlite-based
3. **`src/state/libsql_state.rs`** - Similar to sqlite, uses libsql
4. **`src/state/turso.rs`** - Uses turso crate, same pattern
5. **`src/state/d1.rs`** - HTTP API, different pattern (stream parsing not fetching)
6. **`src/state/r2.rs`** - S3-style API, can do true paginated streaming

## Pattern to Follow

### For SQL Backends (libsql, turso, sqlite)

```rust
use foundation_core::valtron::{run_future_iter, ThreadedValue, Stream};
use crate::rows_stream::LibsqlRowsIterator;  // or RowsIterator for turso

fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
    let sql = "SELECT key FROM kv_store WHERE key LIKE ? ORDER BY key".to_string();
    let params = /* ... */;
    let conn = Arc::clone(&self.conn);

    let iter = run_future_iter(
        move || async move {
            let mut stmt = conn.prepare(&sql).await?;
            let rows = stmt.query(params).await?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows))
        },
        None,  // default queue size
        None,  // default backpressure
    )?;

    // Extract key from each row in the stream
    let stream = iter.map(|threaded_value| match threaded_value {
        ThreadedValue::Value(Ok(row)) => {
            match row.get::<String>(0) {
                Ok(key) => Stream::Next(Ok(key)),
                Err(e) => Stream::Next(Err(StorageError::SqlConversion(e.to_string()))),
            }
        }
        ThreadedValue::Value(Err(e)) => Stream::Next(Err(e)),
    });

    Ok(Box::new(stream))
}
```

### For HTTP Backends (D1)

D1 returns all rows in one HTTP response, so streaming happens at parsing level:

```rust
fn all(&self) -> Result<StateStoreStream<ResourceState>, StorageError> {
    let sql = "SELECT ...".to_string();
    let store = self.clone();

    let iter = run_future_iter(
        move || async move {
            let response = store.execute_sql(&sql, &[])?;
            let rows = D1StateStore::extract_rows(&response);
            Ok::<_, StorageError>(rows.into_iter())  // Stream parsing
        },
        None, None,
    )?;

    let stream = iter.map(move |tv| match tv {
        ThreadedValue::Value(Ok(row)) => match D1StateStore::parse_row(&row) {
            Ok(state) => Stream::Next(Ok(state)),
            Err(e) => Stream::Next(Err(e)),
        },
        ThreadedValue::Value(Err(e)) => Stream::Next(Err(e)),
    });

    Ok(Box::new(stream))
}
```

## Verification After Each File

```bash
# Check compilation
cargo check --package foundation_db

# Check clippy
cargo clippy --package foundation_db -- -D warnings

# Run tests
cargo test --package foundation_db
```

## Common Mistakes to Avoid

1. **Collecting to Vec before returning** - Defeats the purpose of streaming
2. **Not cloning Arc/self before async block** - Lifetime issues
3. **Wrong error type in async block** - Use `Ok::<_, ErrorType>(...)` turbofish
4. **Forgetting to convert ThreadedValue to Stream** - The `.map()` is required
5. **Blocking inside async block** - All I/O must be `.await`
6. **Using `schedule_future` for multi-row results** - Use `run_future_iter` instead

## Success Criteria

After implementation:
- [ ] `libsql_backend::list_keys()` uses `run_future_iter`
- [ ] All state store `all()` methods use `run_future_iter`
- [ ] All state store `list()` methods use `run_future_iter`
- [ ] No `Vec::new()` followed by `for row in rows { push }` patterns in async code
- [ ] `cargo clippy -- -D warnings` passes
- [ ] All tests pass
- [ ] Stream combinators (`.take()`, `.filter()`) work on results

---

_Created: 2026-04-06_
