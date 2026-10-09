---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/00e-state-store-query-filtering"
this_file: "specifications/07-foundation-ai/features/00e-state-store-query-filtering/feature.md"

feature: "State Store Query Filtering — SQL-Level Prefix and Column Queries"
description: "Add StateStore trait methods for SQL-level filtering (prefix, kind, status, provider), database indexes, and optimize NamespacedStore to push filtering down to the database instead of doing it in memory"
status: completed
priority: high
depends_on:
  - "00a-foundation-db"
  - "foundation_core"
estimated_effort: "medium"
created: 2026-04-21
author: "Main Agent"

tasks:
  completed: 10
  uncompleted: 0
  total: 10
  completion_percentage: 100%
---

# State Store Query Filtering — SQL-Level Prefix and Column Queries

## Overview

This feature adds targeted query filtering methods to the `StateStore` trait and overrides them in SQL backends (SQLite, LibSQL, Turso, D1) to push filtering down to the database layer. It also adds database indexes on commonly queried columns and rewrites `NamespacedStore` to use these efficient paths instead of fetching all rows and filtering in memory.

**The Problem:** `NamespacedStore` currently calls `inner.list()` / `inner.all()` which scans the entire table, then filters by prefix in Rust. For large state stores this is O(n) memory + CPU when the database can do O(log n) with a LIKE query on the PRIMARY KEY.

**The Solution:** Add trait methods with SQL-level filtering, create indexes, and have `NamespacedStore` use them.

## Motivation

The existing `StateStore` trait only offers `list()`, `count()`, `all()`, `get()`, `set()`, `delete()` — all operating on ALL rows or single exact-key lookups. There is no way to:

1. List/count/all resources by key prefix without fetching everything
2. Find resources by kind (e.g. "cloudflare::worker")
3. Find resources by status (e.g. all "failed" resources)
4. Find resources by provider (e.g. all "cloudflare" resources)
5. Delete an entire namespace efficiently

SQL backends can handle all of these with indexed queries. The trait needs methods to express them, and SQL implementations need overrides that use `WHERE` clauses instead of full-table scans.

## Goals

- Add `list_by_prefix`, `count_by_prefix`, `all_by_prefix` trait methods with default in-memory fallbacks
- Add `find_by_kind`, `find_by_status`, `find_by_provider` trait methods with default in-memory fallbacks
- Add `delete_by_prefix` trait method with default in-memory fallback
- Override all new methods in SQLite, LibSQL, Turso, and D1 backends with proper SQL queries
- Add indexes on `kind`, `provider`, and `status` columns during `init()`
- Rewrite `NamespacedStore` to use prefix-aware methods instead of fetch-all-then-filter
- Add `find_by_kind`, `find_by_status`, `find_by_provider`, `remove_all` convenience methods on `NamespacedStore`
- No breaking changes to existing trait methods or backends
- Zero warnings from cargo clippy

## Iron Laws

Same as parent spec (`requirements.md`):

### Iron Law 1: No tokio, No async-trait
All async operations use Valtron patterns only.

### Iron Law 4: Zero Warnings, Zero Suppression
All clippy and doc warnings must be fixed, never suppressed.

### Iron Law 5: Error Convention
`derive_more::From` + manual `Display`, no `thiserror`.

## Implementation Details

### 1. LIKE Pattern with Escaping

Prefix queries must escape `%` and `_` characters in the prefix to prevent accidental wildcard matching and SQL injection:

```rust
fn escaped_prefix(prefix: &str) -> String {
    prefix.replace('%', "\\%").replace('_', "\\_")
}
// Query: WHERE id LIKE ?||'%' ESCAPE '\\'
```

### 2. New Trait Methods with Default Implementations

Each new method on `StateStore` gets a default implementation using existing methods + in-memory filtering, so non-SQL backends (File, R2) work without changes:

```rust
fn list_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<String>, StorageError> {
    // Default: call self.list(), filter by starts_with(prefix)
}
fn count_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
    // Default: call self.count_by_prefix via list_by_prefix + count
}
fn all_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
    // Default: call self.all(), filter by starts_with(prefix)
}
fn find_by_kind(&self, kind: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
    // Default: call self.all(), filter by state.kind == kind
}
fn find_by_status(&self, status: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
    // Default: call self.all(), filter by state.status serialized == status
}
fn find_by_provider(&self, provider: &str) -> Result<StateStoreStream<ResourceState>, StorageError> {
    // Default: call self.all(), filter by state.provider == provider
}
fn delete_by_prefix(&self, prefix: &str) -> Result<StateStoreStream<usize>, StorageError> {
    // Default: call self.list(), filter by prefix, call self.delete() for each, return count
}
```

### 3. SQL Backend Overrides

SQL backends override with direct SQL:

- `list_by_prefix`: `SELECT id FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id`
- `count_by_prefix`: `SELECT COUNT(*) FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\'`
- `all_by_prefix`: `SELECT ... FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\' ORDER BY id`
- `find_by_kind`: `SELECT ... FROM {table} WHERE kind = ? ORDER BY id`
- `find_by_status`: `SELECT ... FROM {table} WHERE status = ? ORDER BY id`
- `find_by_provider`: `SELECT ... FROM {table} WHERE provider = ? ORDER BY id`
- `delete_by_prefix`: `DELETE FROM {table} WHERE id LIKE ?||'%' ESCAPE '\\'`

For `delete_by_prefix`, the return value is the number of deleted rows. libsql's `execute()` returns `rows_affected`. D1 extracts it from the response.

### 4. Indexes

During `init()`, after `CREATE TABLE`:

```sql
CREATE INDEX IF NOT EXISTS idx_{table}_kind ON {table}(kind);
CREATE INDEX IF NOT EXISTS idx_{table}_provider ON {table}(provider);
CREATE INDEX IF NOT EXISTS idx_{table}_status ON {table}(status);
```

The `id` column already has a PRIMARY KEY index.

### 5. NamespacedStore Rewrite

`NamespacedStore` currently:
- `list()`: fetches ALL, filters in Rust → use `inner.list_by_prefix(&self.prefix)`
- `count()`: fetches ALL, counts in Rust → use `inner.count_by_prefix(&self.prefix)`
- `all()`: fetches ALL, filters in Rust → use `inner.all_by_prefix(&self.prefix)`

New convenience methods on `NamespacedStore`:
- `find_by_kind(kind)` → delegate to inner with prefix + kind combination
- `find_by_status(status)` → delegate to inner with prefix + status combination
- `find_by_provider(provider)` → delegate to inner with prefix + provider combination
- `remove_all()` → delete all keys in this namespace via `delete_by_prefix`

### 6. Non-SQL Backends

FileStateStore and R2StateStore use default trait implementations. No changes needed.

## Dependencies

- `foundation_db` state store module (existing)
- No new external dependencies

## Tasks

### Task 1: Add new trait methods to StateStore
- [x] Add `list_by_prefix`, `count_by_prefix`, `all_by_prefix` with default impls
- [x] Add `find_by_kind`, `find_by_status`, `find_by_provider` with default impls
- [x] Add `delete_by_prefix` with default impl

### Task 2: Add indexes to SQL backends
- [x] Add index creation to `SqliteStateStore::init()`
- [x] Add index creation to `LibSQLStateStore::init()`
- [x] Add index creation to `TursoStateStore::init()`
- [x] Add index creation to `D1StateStore::init()`

### Task 3: Implement SQL backend overrides
- [x] Override all new methods in `SqliteStateStore`
- [x] Override all new methods in `LibSQLStateStore`
- [x] Override all new methods in `TursoStateStore`
- [x] Override all new methods in `D1StateStore`

### Task 4: Rewrite NamespacedStore
- [x] Rewrite `list()`, `count()`, `all()` to use prefix-aware inner methods
- [x] Add `find_by_kind`, `find_by_status`, `find_by_provider` convenience methods
- [x] Add `remove_all()` convenience method
- [x] Update trait impl to use new methods

### Task 5: Tests
- [x] Test SQL-level filtering produces same results as default in-memory filtering
- [x] Test prefix escaping (prefixes containing `%` or `_`)
- [x] Test NamespacedStore delegation
- [x] Test empty results for all new methods
- [x] Test `remove_all()` deletes only namespaced keys

### Task 6: Verify
- [x] `cargo check --package foundation_db`
- [x] `cargo clippy --package foundation_db -- -D warnings`
- [x] `cargo test --package foundation_db`
- [x] `cargo fmt --package foundation_db -- --check`

## Verification Commands

```bash
cargo check --package foundation_db
cargo clippy --package foundation_db -- -D warnings
cargo test --package foundation_db
cargo fmt --package foundation_db -- --check
```

---

_Created: 2026-04-21_
