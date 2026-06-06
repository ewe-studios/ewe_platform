---
feature_name: "VFS Valtron Integration Tests"
description: "Comprehensive integration tests for all VFS components through valtron's executor — SyncFs bridge, SyncLibsqlDelta, SyncFs<MemoryFs>, AsyncVfsFileSystem traits called via valtron futures. Every sync bridge call must be exercised through initialize_pool + execute + collect_one."
status: "in-progress"
priority: "critical"
phase: 3
created: 2026-06-06
updated: 2026-06-06
dependencies:
  - "01-core-traits"
  - "02-memory-impls"
  - "03-foundation-fs"
  - "06-sqlite-delta"
  - "20-async-first-migration"
  - "21-seekable-sync-cleanup"
tasks:
  completed: 0
  uncompleted: 28
  total: 28
  completion_percentage: 0%
---

# VFS Valtron Integration Tests

## Iron Rule: Valtron-Backed Tests Required

**Any feature that uses valtron (sync bridge, `exec_async`, `SyncFs`, `execute`, `collect_one`, `from_future`) MUST have integration tests that:**

1. Initialize the valtron pool with `initialize_pool()` (see `backends/foundation_nativeapis/tests/valtron_executor_integration.rs` for the pattern)
2. Exercise all sync-bridge methods through valtron — not just direct sync calls
3. Verify both sync (via bridge) AND async implementations
4. Use `#[ntest::timeout(60_000)]` + `#[serial_test::serial]` + `#[tracing_test::traced_test]`

**Why:** The `SyncFs<A>` bridge wraps async VFS ops through valtron's `execute` + `collect_one`. Calling these without an initialized pool silently fails or panics. Direct sync calls on MemoryFs bypass the bridge entirely, so they don't test the actual code path used in production.

### Valtron Test Pattern (from `valtron_executor_integration.rs`)

```rust
use foundation_core::valtron::{collect_one, collect_result, execute, initialize_pool, PoolGuard, from_future};

/// Initialize the valtron thread pool.
fn init_pool() -> PoolGuard {
    initialize_pool(42, Some(3))
}

#[test]
#[ntest::timeout(60_000)]
#[serial_test::serial]
#[tracing_test::traced_test]
fn test_sync_bridge_through_valtron() {
    let _guard = init_pool();  // ← MUST be first thing in test

    let fs = LibsqlDelta::new("/tmp/test.db").unwrap();
    let sync = SyncLibsqlDelta::new(fs);

    // Exercise through valtron — the actual production code path
    sync.mkdir("/src").unwrap();  // goes through valtron's executor

    // Also test async directly for completeness
    // (but the critical path is the sync bridge → valtron)
}
```

**Reference implementations:**
- `backends/foundation_nativeapis/tests/valtron_executor_integration.rs` — full valtron lifecycle test
- `backends/foundation_nativeapis/tests/valtron_multi_executor.rs` — multi-threaded executor test
- `backends/foundation_nativeapis/tests/valtron_integration.rs` — FileWatcherTask + valtron

## Tasks

### SyncBridge + MemoryFs via Valtron

- [ ] `SyncFs<MemoryFs>::mkdir` through valtron — pool initialized, method called via `exec_async`
- [ ] `SyncFs<MemoryFs>::create + write + read` through valtron — full CRUD cycle via bridge
- [ ] `SyncFs<MemoryFs>::open_seekable + read/write/seek` through valtron — `LocalSeekableFile` cursor
- [ ] `SyncFs<MemoryFs>::stat + exists + remove` through valtron — metadata operations
- [ ] `SyncFs<MemoryFs>::rename + copy` through valtron — multi-step operations
- [ ] `SyncFs<MemoryFs>::mkdir_all + remove_all` through valtron — recursive operations

### SyncBridge + MemoryDelta via Valtron

- [ ] `SyncFs<MemoryDelta>::open + write` through valtron — write captured in delta
- [ ] `SyncFs<MemoryDelta>::overlay semantics` — verify delta hides base files via valtron
- [ ] `SyncFs<MemoryDelta>::whiteout add/remove/list` through valtron
- [ ] `SyncFs<MemoryDelta>::reset` through valtron — clears all delta state
- [ ] `SyncFs<MemoryDelta>::flush` through valtron — no-op but must not panic

### SyncLibsqlDelta via Valtron

- [ ] `SyncLibsqlDelta::new + mkdir` through valtron — pool init, libsql-backed
- [ ] `SyncLibsqlDelta::create + write_at + read_at` through valtron — chunked storage via bridge
- [ ] `SyncLibsqlDelta::open_seekable + seek` — `SyncSeekableSqliteFile` with `Arc<AtomicU64>` cursor
- [ ] `SyncLibsqlDelta::stat + exists + remove` through valtron — SQL-backed lookups
- [ ] `SyncLibsqlDelta::whiteout CRUD` through valtron — prefix index tested
- [ ] `SyncLibsqlDelta::reset` through valtron — clears SQLite tables

### Async Traits via Valtron

- [ ] `LibsqlDelta::open_async + write_at_async` via `execute` + `collect_one` — future owns the async call
- [ ] `LibsqlDelta::open_directory_async + list_async` via valtron — directory listing
- [ ] `MemoryFile::read_at_async` via valtron — async through bridge, verify result
- [ ] `AsyncDeltaStore::add_whiteout_async` via valtron — async whiteout through executor

### Seekable File Concurrency via Valtron

- [ ] `SyncSeekableSqliteFile` cursor sharing — clone Arc, seek on both, verify shared state
- [ ] `LocalSeekableFile<SyncFile<SqliteFile>>` via valtron — generic seekable bridge
- [ ] Concurrent seeks on same file — spawn two tasks, verify no panic, correct positions

### OverlayFileSystem + Valtron

- [ ] `OverlayFileSystem<SyncLibsqlDelta, MemoryDelta>` through valtron — stacked overlay
- [ ] CoW (copy-on-write) via valtron — write to overlaid base file, verify base unchanged
- [ ] Whiteout filtering via valtron — delta whiteout hides base, overlay respects it
- [ ] Version counter increment via valtron — every mutation bumps version

### Error Paths via Valtron

- [ ] Sync bridge valtron execution failure — verify `VfsError::Backend` propagated correctly
- [ ] Pool exhaustion — spawn more concurrent ops than threads, verify no deadlock
- [ ] Pool guard dropped mid-execution — verify clean shutdown, no panic

## Test File Organization

All tests go in `backends/foundation_nativeapis/tests/`:

```
tests/
  valtron_vfs_memory.rs      # SyncFs<MemoryFs> + SyncFs<MemoryDelta> via valtron
  valtron_vfs_sqlite.rs      # SyncLibsqlDelta via valtron
  valtron_vfs_async.rs       # Async traits called via valtron futures
  valtron_vfs_overlay.rs     # OverlayFileSystem through valtron
  valtron_vfs_seekable.rs    # Seekable file concurrency + cursor sharing
  valtron_vfs_errors.rs      # Error propagation through valtron bridge
```

## Feature-Gate Test Requirements

Tests are gated on the same features as the code they test:
- `valtron_vfs_memory.rs` → `#[cfg(feature = "vfs")]`
- `valtron_vfs_sqlite.rs` → `#[cfg(feature = "vfs-sqlite")]`
- `valtron_vfs_async.rs` → `#[cfg(feature = "vfs")]`
- `valtron_vfs_overlay.rs` → `#[cfg(feature = "vfs")]`
- `valtron_vfs_seekable.rs` → `#[cfg(feature = "vfs")]`
- `valtron_vfs_errors.rs` → `#[cfg(feature = "vfs")]`

## Verification

- [ ] All valtron-backed VFS sync bridges tested through `initialize_pool`
- [ ] No VFS sync bridge code path left untested by valtron
- [ ] `cargo test -p foundation_nativeapis --features vfs,vfs-sqlite` — all valtron VFS tests pass
- [ ] `cargo test -p foundation_nativeapis --features vfs -- --ignored` — skipped tests verified
- [ ] No test calls `SyncFs` or `SyncLibsqlDelta` without first calling `initialize_pool()`

---

_Created: 2026-06-06_
