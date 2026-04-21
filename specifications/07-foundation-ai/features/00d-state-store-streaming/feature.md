---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/00d-state-store-streaming"
this_file: "specifications/07-foundation-ai/features/00d-state-store-streaming/feature.md"

feature: "State Store Streaming - Proper Valtron Iterator Patterns"
description: "Fix all state stores AND storage backends to use run_future_iter for streaming multi-row results instead of collecting to Vec"
status: complete
priority: high
depends_on:
  - "00a-foundation-db"
  - "foundation_core"
estimated_effort: "large"
created: 2026-04-06
author: "Main Agent"

tasks:
  completed: 12
  uncompleted: 0
  total: 12
  completion_percentage: 100%
---

# State Store Streaming - Proper Valtron Iterator Patterns

## Overview

This feature fixes all state store implementations AND storage backends in `foundation_db` to properly use Valtron's `run_future_iter` pattern for streaming multi-row database results.

**The Problem:** Current implementations collect all rows into a `Vec` before returning, which:
1. Defeats the purpose of streaming - all data loaded into memory at once
2. Breaks composability - can't use Valtron combinators on the stream
3. Wastes memory - large result sets unnecessarily allocated
4. Inconsistent with `KeyValueStore::query()` pattern - which properly uses `run_future_iter`

**The Solution:** Use `run_future_iter` to wrap async row iterators, allowing proper streaming of results through Valtron's `ThreadedValue` -> `Stream` pipeline.

## Motivation

The `KeyValueStore::query()` method (Turso/libsql backends) correctly uses `run_future_iter`:

```rust
fn query(
    &self,
    sql: &str,
    params: &[DataValue],
) -> StorageResult<StorageItemStream<'_, SqlRow>> {
    use crate::rows_stream::RowsIterator;

    let iter = run_future_iter(
        move || async move {
            let mut stmt = conn.prepare(&sql).await?;
            let rows = stmt.query(params).await?;
            Ok::<_, StorageError>(RowsIterator::new(rows))
        },
        None, None,
    )?;

    let stream = iter.map(|threaded_value| match threaded_value {
        ThreadedValue::Value(result) => Stream::Next(result),
    });

    Ok(Box::new(stream))
}
```

But many other methods incorrectly collect to `Vec`:

```rust
// WRONG: libsql_backend.rs list_keys() - lines 305-342
let stream = schedule_future(async move {
    let mut keys = Vec::new();  // ❌ Collects all to Vec!
    while let Some(row) = rows.next().await? {
        keys.push(row.get::<String>(0)?);  // ❌ Pushes to Vec!
    }
    Ok::<_, libsql::Error>(keys)
})?;
```

## Requirements

### Storage Backends (src/backends/)

#### libsql_backend.rs

1. **Fix `list_keys()`** - Currently collects to `Vec`, should use `run_future_iter` with `LibsqlRowsIterator`

Current code (lines 294-342):
```rust
fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
    let stream = schedule_future(async move {
        let mut keys = Vec::new();
        while let Some(row) = rows.next().await? {
            keys.push(key);
        }
        Ok::<_, libsql::Error>(keys)
    })?;
    // Then flat_map_next to expand Vec - pointless!
}
```

Fixed code:
```rust
fn list_keys(&self, prefix: Option<&str>) -> StorageResult<StorageItemStream<'_, String>> {
    use crate::rows_stream::LibsqlRowsIterator;
    
    let iter = run_future_iter(
        move || async move {
            let mut stmt = conn.prepare(sql).await?;
            let rows = stmt.query(params).await?;
            Ok::<_, StorageError>(LibsqlRowsIterator::new(rows))
        },
        None, None,
    )?;
    
    let stream = iter.map(|tv| match tv {
        ThreadedValue::Value(Ok(row)) => {
            match row.get::<String>(0) {
                Ok(key) => Stream::Next(Ok(key)),
                Err(e) => Stream::Next(Err(e)),
            }
        }
        ThreadedValue::Value(Err(e)) => Stream::Next(Err(e)),
    });
    
    Ok(Box::new(stream))
}
```

#### turso_backend.rs

2. **Review `list_keys()`** - Already uses `run_future_iter` correctly (line 348-383) - NO CHANGE NEEDED

#### memory.rs

3. **Review `list_keys()`** - Memory backend is synchronous, current pattern acceptable (no async to wrap)

#### json_file.rs

4. **Review `list_keys()`** - JSON file backend is synchronous, current pattern acceptable

### State Stores (src/state/)

5. **D1StateStore** - Fix `all()`, `list()`, `get_batch()`
6. **R2StateStore** - Fix `all()`, `list()`
7. **SqliteStateStore** - Fix `all()`, `list()`, `query()`
8. **LibsqlStateStore** - Fix `all()`, `list()`, `query()`
9. **TursoStateStore** - Fix `all()`, `list()`, `query()`

## Files to Modify

| File | Methods to Fix | Priority |
|------|---------------|----------|
| `src/backends/libsql_backend.rs` | `list_keys()` | High |
| `src/state/d1.rs` | `all()`, `list()`, `get_batch()` | High |
| `src/state/sqlite.rs` | `all()`, `list()`, `query()` | High |
| `src/state/libsql_state.rs` | `all()`, `list()`, `query()` | High |
| `src/state/turso.rs` | `all()`, `list()`, `query()` | High |
| `src/state/r2.rs` | `all()`, `list()` | Medium |

## Files Already Correct

| File | Method | Notes |
|------|--------|-------|
| `src/backends/turso_backend.rs` | `list_keys()` | Uses `run_future_iter` ✓ |
| `src/backends/turso_backend.rs` | `query()` | Uses `run_future_iter` ✓ |
| `src/backends/libsql_backend.rs` | `query()` | Uses `run_future_iter` ✓ |
| `src/backends/memory.rs` | All | Sync, no async wrapping needed ✓ |
| `src/backends/json_file.rs` | All | Sync, no async wrapping needed ✓ |

## Testing Requirements

### Unit Tests for Each Fixed Method

Every method that is fixed MUST have tests that validate:

1. **Single-row retrieval** - Method correctly returns exactly one row
2. **Multi-row retrieval** - Method correctly returns multiple rows (all of them)
3. **Empty result** - Method correctly handles zero rows
4. **Stream consumption** - Rows can be consumed one at a time via iterator

### Test Patterns

#### Pattern 1: Single Row Test

```rust
#[test]
fn test_list_keys_single_row() {
    init_valtron();
    let storage = setup_storage();
    
    // Insert single key
    storage.set("only_key", "value").unwrap();
    
    // Stream should yield exactly one key
    let stream = storage.list_keys(None).unwrap();
    let keys: Vec<String> = stream
        .filter_map(|tv| match tv {
            ThreadedValue::Value(Ok(k)) => Some(k),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e}"),
            _ => None,
        })
        .collect();
    
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0], "only_key");
}
```

#### Pattern 2: Multi-Row Test

```rust
#[test]
fn test_list_keys_multiple_rows() {
    init_valtron();
    let storage = setup_storage();
    
    // Insert multiple keys
    let expected_keys = vec!["key1", "key2", "key3", "key4", "key5"];
    for key in &expected_keys {
        storage.set(key, "value").unwrap();
    }
    
    // Stream should yield all keys
    let stream = storage.list_keys(None).unwrap();
    let keys: Vec<String> = stream
        .filter_map(|tv| match tv {
            ThreadedValue::Value(Ok(k)) => Some(k),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e}"),
            _ => None,
        })
        .collect();
    
    assert_eq!(keys.len(), 5);
    for key in &expected_keys {
        assert!(keys.contains(&key.to_string()));
    }
}
```

#### Pattern 3: Empty Result Test

```rust
#[test]
fn test_list_keys_empty() {
    init_valtron();
    let storage = setup_storage();
    
    // No keys inserted
    let stream = storage.list_keys(None).unwrap();
    let keys: Vec<String> = stream
        .filter_map(|tv| match tv {
            ThreadedValue::Value(Ok(k)) => Some(k),
            ThreadedValue::Value(Err(e)) => panic!("Unexpected error: {e}"),
            _ => None,
        })
        .collect();
    
    assert_eq!(keys.len(), 0);
}
```

#### Pattern 4: Stream Combinator Test

```rust
#[test]
fn test_list_keys_take_combinator() {
    init_valtron();
    let storage = setup_storage();
    
    // Insert 10 keys
    for i in 0..10 {
        storage.set(&format!("key{}", i), "value").unwrap();
    }
    
    // Use .take() combinator to get only first 3
    let stream = storage.list_keys(None).unwrap();
    let keys: Vec<String> = stream
        .take(3)
        .filter_map(|tv| match tv {
            ThreadedValue::Value(Ok(k)) => Some(k),
            _ => None,
        })
        .collect();
    
    assert_eq!(keys.len(), 3);
}
```

### Test Matrix

| File | Single Row | Multi-Row | Empty | Combinator |
|------|-----------|-----------|-------|------------|
| `libsql_backend.rs::list_keys()` | ✓ | ✓ | ✓ | ✓ |
| `state/d1.rs::all()` | ✓ | ✓ | ✓ | ✓ |
| `state/d1.rs::list()` | ✓ | ✓ | ✓ | ✓ |
| `state/d1.rs::get_batch()` | ✓ | ✓ | ✓ | ✓ |
| `state/sqlite.rs::all()` | ✓ | ✓ | ✓ | ✓ |
| `state/sqlite.rs::list()` | ✓ | ✓ | ✓ | ✓ |
| `state/libsql_state.rs::all()` | ✓ | ✓ | ✓ | ✓ |
| `state/turso.rs::all()` | ✓ | ✓ | ✓ | ✓ |
| `state/r2.rs::all()` | ✓ | ✓ | ✓ | ✓ |

### Test File Locations

New test files to create:

| File | Test Module |
|------|-------------|
| `tests/backends/libsql_streaming_tests.rs` | libsql `list_keys()` streaming |
| `tests/state/d1_streaming_tests.rs` | D1 state store streaming |
| `tests/state/sqlite_streaming_tests.rs` | SQLite state store streaming |
| `tests/state/libsql_state_streaming_tests.rs` | libsql state store streaming |
| `tests/state/turso_state_streaming_tests.rs` | Turso state store streaming |
| `tests/state/r2_streaming_tests.rs` | R2 state store streaming |

### Verification Commands

```bash
# Run all streaming tests
cargo test --package foundation_db -- streaming

# Run with output to verify streaming order
cargo test --package foundation_db -- streaming --nocapture

# Run single test
cargo test --package foundation_db -- test_list_keys_multiple_rows
```

## Success Criteria

### Code Changes

- [ ] `libsql_backend::list_keys()` uses `run_future_iter`
- [ ] All state store `all()` methods use `run_future_iter`
- [ ] All state store `list()` methods use `run_future_iter`
- [ ] No `Vec::new()` followed by `for row in rows { push }` patterns in async code

### Test Coverage

- [ ] Single-row tests pass for all fixed methods
- [ ] Multi-row tests pass for all fixed methods  
- [ ] Empty result tests pass for all fixed methods
- [ ] Combinator tests pass (`.take()`, `.filter()` work)

### Quality Checks

- [ ] `cargo check --package foundation_db` passes
- [ ] `cargo clippy --package foundation_db -- -D warnings` passes
- [ ] `cargo test --package foundation_db` passes (all tests including new streaming tests)
- [ ] `cargo fmt --package foundation_db -- --check` passes

### Documentation

- [ ] LEARNINGS.md updated with streaming patterns and test patterns
- [ ] Module doc comments explain streaming design
