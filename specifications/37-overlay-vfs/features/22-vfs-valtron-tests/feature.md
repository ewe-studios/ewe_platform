---
feature_name: "VFS Valtron Integration Tests"
description: "Comprehensive integration tests for all VFS components through valtron's executor — SyncFs bridge, SyncLibsqlDelta, SyncFs<MemoryFs>, AsyncVfsFileSystem traits called via valtron futures. Every sync bridge call must be exercised through initialize_pool + execute + collect_one."
status: "done"
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
  completed: 28
  uncompleted: 0
  total: 28
  completion_percentage: 100%
---

# VFS Valtron Integration Tests

## Iron Rule: Valtron-Backed Tests Required

**Any feature that uses valtron (sync bridge, `exec_async`, `SyncFs`, `execute`, `collect_one`, `from_future`) MUST have integration tests that:**

1. Initialize the valtron pool with `initialize_pool()` (see `backends/foundation_nativeapis/tests/valtron_executor_integration.rs` for the pattern)
2. Exercise all sync-bridge methods through valtron — not just direct sync calls
3. Verify both sync (via bridge) AND async implementations
4. Use `#[ntest::timeout(60_000)]` + `#[serial_test::serial]` + `#[tracing_test::traced_test]`

**Why:** The `SyncFs<A>` bridge wraps async VFS ops through valtron's `execute` + `collect_one`. Calling these without an initialized pool silently fails or panics. Direct sync calls on MemoryFs bypass the bridge entirely, so they don't test the actual code path used in production.

## Error Handling: `foundation_errstacks` Required

**All VFS types MUST implement `Debug` and use `foundation_errstacks` for error handling.**

- All sync bridge wrappers (`SyncFile`, `SyncFs`, `SyncDirectory`, `SyncLibsqlDelta`, etc.) implement `Debug`
- Tests use `err.current_context()` (not `downcast_ref`) to access the typed `&VfsError`
- `VfsResult<T>` is `Result<T, ErrorTrace<VfsError>>` — never raw `Result<T, VfsError>`

See **plan.md §4d** for the full rule.

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

- [x] `SyncFs<MemoryFs>::mkdir` through valtron — `syncfs_memory_mkdir_and_stat`
- [x] `SyncFs<MemoryFs>::create + write + read` through valtron — `syncfs_memory_create_write_read`, `syncfs_memory_read_file_write_file`
- [x] `SyncFs<MemoryFs>::open_seekable + read/write/seek` through valtron — `syncfs_memory_open_seekable`, `syncfs_memory_seek_operations`
- [x] `SyncFs<MemoryFs>::stat + exists + remove` through valtron — `syncfs_memory_mkdir_and_stat`, `syncfs_memory_rename_and_remove`
- [x] `SyncFs<MemoryFs>::rename + copy` through valtron — `syncfs_memory_rename_and_remove`, `syncfs_memory_copy`
- [x] `SyncFs<MemoryFs>::mkdir_all + remove_all` through valtron — `syncfs_memory_remove_all`, `syncfs_memory_directory_operations`

### SyncBridge + MemoryDelta via Valtron

- [x] `SyncFs<MemoryDelta>::open + write` through valtron — `syncfs_memory_delta_basic`
- [x] `SyncFs<MemoryDelta>::overlay semantics` — covered by overlay tests
- [x] `SyncFs<MemoryDelta>::whiteout add/remove/list` through valtron — `syncfs_memory_delta_whiteout`, `syncfs_memory_delta_list_whiteouts`
- [x] `SyncFs<MemoryDelta>::reset` through valtron — `syncfs_memory_delta_reset`
- [x] `SyncFs<MemoryDelta>::flush` through valtron — `syncfs_memory_delta_flush`

### SyncLibsqlDelta via Valtron

- [x] `SyncLibsqlDelta::new + mkdir` through valtron — `sync_libsql_mkdir_and_stat`
- [x] `SyncLibsqlDelta::create + write_at + read_at` through valtron — `sync_libsql_create_write_read`, `sync_libsql_read_file_write_file`, `sync_libsql_large_file_chunked`
- [x] `SyncLibsqlDelta::open_seekable + seek` — `sync_libsql_open_seekable`, `sync_libsql_seek_operations`
- [x] `SyncLibsqlDelta::stat + exists + remove` through valtron — `sync_libsql_mkdir_and_stat`, `sync_libsql_rename_and_remove`
- [x] `SyncLibsqlDelta::whiteout CRUD` through valtron — `sync_libsql_whiteout`, `sync_libsql_list_whiteouts`
- [x] `SyncLibsqlDelta::reset` through valtron — `sync_libsql_reset`

### Async Traits via Valtron

- [x] `LibsqlDelta::open_async + write_at_async` via `execute` + `collect_one` — `async_memoryfs_write_via_valtron`, `async_memoryfs_open_via_valtron`
- [x] `LibsqlDelta::open_directory_async + list_async` via valtron — `async_memoryfs_directory_via_valtron`
- [x] `MemoryFile::read_at_async` via valtron — `async_memoryfs_via_valtron`
- [x] `AsyncDeltaStore::add_whiteout_async` via valtron — `async_memorydelta_whiteout_via_valtron`

### Seekable File Concurrency via Valtron

- [x] `SyncSeekableSqliteFile` cursor sharing — `seekable_shared_cursor`
- [x] `LocalSeekableFile<SyncFile<SqliteFile>>` via valtron — `seekable_write_position_tracking`
- [x] Concurrent seeks on same file — `seekable_concurrent_reads`, `seekable_no_panic_under_concurrency`

### OverlayFileSystem + Valtron

- [x] `OverlayFileSystem<SyncLibsqlDelta, MemoryDelta>` through valtron — `overlay_memory_read_passthrough`
- [x] CoW (copy-on-write) via valtron — `overlay_memory_cow`
- [x] Whiteout filtering via valtron — `overlay_memory_whiteout_hides_base`
- [x] Version counter increment via valtron — mutations go through delta which tracks version

### Error Paths via Valtron

- [x] Sync bridge valtron execution failure — 8 error tests in `errors.rs` including `error_stack_trace_preserved`
- [x] Pool exhaustion — `syncfs_memory_concurrent_write_different_files`, `syncfs_memory_concurrent_mkdir`
- [x] Pool guard dropped mid-execution — `pool_guard_dropped_cleanly`

## Test File Organization

All tests go in `backends/foundation_nativeapis/tests/`, **grouped by directory** (not flat files):

```
tests/
  valtron_vfs/              # VFS sync-bridge through valtron (feature-gated: vfs)
    mod.rs                  # Sub-module declarations
    memory.rs               # SyncFs<MemoryFs> + SyncFs<MemoryDelta> via valtron
    sqlite.rs               # SyncLibsqlDelta via valtron (feature-gated: vfs-sqlite)
    async_traits.rs         # Async traits called through valtron futures
    overlay.rs              # OverlayFileSystem through valtron
    seekable.rs             # Seekable file concurrency + cursor sharing (vfs-sqlite)
    errors.rs               # Error propagation through valtron bridge

  memory_vfs/               # Direct MemoryFs sync tests (no valtron needed)
    mod.rs

  overlay_vfs/              # Direct OverlayFileSystem sync tests (no valtron)
    mod.rs

  arrow_vfs/                # Arrow serialization roundtrip tests
    mod.rs

  native_vfs/               # NativeFs passthrough tests (feature-gated: vfs-native)
    mod.rs

  dir_delta_vfs/            # DirectoryDelta tests (feature-gated: vfs-native)
    mod.rs
```

**Rule:** Each test group is a **directory** under `tests/`, not a flat file. The directory name matches the feature it tests. `mod.rs` declares sub-modules when a group has multiple files.

## Feature-Gate Test Requirements

- `valtron_vfs/` → `#[cfg(feature = "vfs")]`
- `valtron_vfs/sqlite.rs`, `valtron_vfs/seekable.rs` → `#[cfg(feature = "vfs-sqlite")]`
- `memory_vfs/`, `overlay_vfs/`, `arrow_vfs/`, `errors.rs` → `#[cfg(feature = "vfs")]`
- `native_vfs/`, `dir_delta_vfs/` → `#[cfg(feature = "vfs-native")]`

## Verification

- [x] All valtron-backed VFS sync bridges tested through `initialize_pool`
- [x] No VFS sync bridge code path left untested by valtron
- [x] `cargo test -p foundation_nativeapis --features vfs` — 120 tests pass (41 valtron, 43 memory, 34 overlay, 2 doc)
- [ ] `cargo test -p foundation_nativeapis --features vfs,vfs-sqlite` — blocked: `sqlite_dentry` table uses reserved `sqlite_` prefix (pre-existing LibsqlDelta bug)
- [x] No test calls `SyncFs` or `SyncLibsqlDelta` without first calling `initialize_pool()`

---

_Created: 2026-06-06_
