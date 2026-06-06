---
feature_name: "SyncBridge Seekable Cleanup"
description: "Remove the dangerous Arc<Mutex<Option<A>>> pattern from SyncSeekableFile, eliminate the generic seekable bridge, and have each backend provide its own seekable wrapper. Duplication is acceptable; correctness is not negotiable."
status: "done"
priority: "critical"
phase: 3
created: 2026-06-06
updated: 2026-06-06
dependencies: ["20-async-first-migration"]
tasks:
  completed: 18
  uncompleted: 0
  total: 18
  completion_percentage: 100%
---

# Feature 21: SyncBridge Seekable Cleanup

## Problem

In Feature 20's `sync_bridge.rs`, `SyncSeekableFile<A: AsyncSeekableVfsFile>` uses a dangerous `Arc<Mutex<Option<A>>>` take/put-back pattern:

```rust
// BROKEN — concurrent callers panic
fn with_mut<T>(&self, f: impl FnOnce(A) -> Fut) -> VfsResult<T> {
    let taken = slot.lock().unwrap().take()  // Option becomes None
        .expect("inner value was already taken");  // concurrent thread → 💥
    let result = exec_async(async move {
        let (file, result) = f(taken).await;
        Ok((file, result))
    });
    // ... put back
}
```

**Race window**: Thread A takes the value and spawns an async future on valtron. Thread B calls any method and hits `None.unwrap()` → panic.

**Why it exists**: `AsyncSeekableVfsFile` methods take `&mut self`. The future needs to own `A` (valtron requires `Send + 'static`). We can't hold a `MutexGuard` across the `await`. Cloning won't work because each clone gets its own cursor position and mutations are lost.

## Decision: Remove the Generic Bridge

**`SyncSeekableFile<A>` is deleted.** It solves nothing that can't be done more safely with per-backend code or the existing `LocalSeekableFile`.

## Architecture After

### `&self` methods (all backends) — Generic bridge works perfectly

`SyncFile<A>` clones `Arc<A>`, runs via valtron, copies result back. No state mutation, no concurrency issues.

```
SyncFile<SqliteFile> → clone Arc<Connection> → exec_async → done
```

### `&mut self` methods (seekable) — Per-backend or `LocalSeekableFile`

Two paths:

#### Path A: Async-native backends with local cursor (libsql, turso)

The `SqliteFile` struct has `Arc<Connection>` (shared, immutable) + `ino` (immutable). No native cursor in the DB — cursor is a `u64`. The seekable wrapper uses `Arc<AtomicU64>` for the cursor, making it cheaply clonable with shared state:

```rust
pub struct SeekableSqliteFile {
    pub inner: SqliteFile,           // Arc<Connection> + ino
    pub cursor: Arc<AtomicU64>,      // shared cursor, atomic reads/writes
}
```

**Why `AtomicU64` here (sensible):** The cursor is a single `u64` that's cheaply atomic. Cloning is `cursor: Arc::clone(&self.cursor)`. All clones share the same cursor position. No Mutex, no lock contention, no panic windows. The seekable file is cheaply cloneable — `Clone` is just two `Arc::clone`s.

For the **sync** version: `read_at`/`write_at` go through valtron (`&self` bridge). Cursor updates are `cursor.store(new_val, Ordering::Relaxed)` — relaxed is fine because the valtron call already provides ordering.

For the **async** version: Same pattern — cursor is `load`/`store` atomic, no `&mut self` needed. `AsyncSeekableVfsFile` methods work with `&self`.

#### Path B: Any backend without native seekable

`LocalSeekableFile<SyncFile<A>>` already exists in `sync_bridge.rs`. Plain cursor + delegates to `SyncFile`'s bridged `read_at`/`write_at`. Zero concurrency issues.

### `SyncFs<A>::SeekableFile` type

`SyncFs<A>::SeekableFile` uses `LocalSeekableFile<SyncFile<A::File>>`. Every backend gets seekable support for free via `read_at`/`write_at` + local cursor.

### Backend override pattern (`SyncLibsqlDelta`)

Backends that want optimized seekable implement their own struct that wraps `SyncFs<A>` and delegates everything except `open_seekable`:

```rust
pub struct SyncLibsqlDelta {
    inner: SyncFs<LibsqlDelta>,
}

impl VfsFileSystem for SyncLibsqlDelta {
    type File = <SyncFs<LibsqlDelta> as VfsFileSystem>::File;
    type SeekableFile = SyncSeekableSqliteFile;  // custom!
    type Directory = SyncSqliteDirectory;        // custom (matches SeekableFile)
    // delegate all 12 methods to self.inner...
    fn open_seekable(&self, path: &str, mode: OpenMode) -> VfsResult<Self::SeekableFile> {
        let file = self.inner.open(path, mode)?;  // returns SyncFile<SqliteFile>
        Ok(SyncSeekableSqliteFile::new(file))
    }
}
```

`SyncFs::inner()` accessor is provided for backends that need to delegate to it.

### `Arc<AtomicU64>` for seekable cursors

**Rule:** Only for seekable file cursors — a single `u64` where sharing via `Arc` makes sense and atomic ops are cheaper than a lock.

**Where applied:**
- `SeekableSqliteFile::cursor: Arc<AtomicU64>` — enables `Clone` without losing state, no `&mut self` needed for async trait
- `SeekableMemoryFile::position` — same reasoning for sync-native

**Where NOT applied:**
- `MemoryFsInner::version` — already behind `Arc<RwLock<MemoryFsInner>>` protecting the `HashMap`, no benefit to atomic
- Generic counters or IDs — use `AtomicU64` only for cheap, shared, single-field state like cursors. Not as a blanket replacement.

## What Changes

| File | Change |
|------|--------|
| `sync_bridge.rs` | **Remove** `SyncSeekableFile` struct + impls. Change `SyncFs::SeekableFile` to `LocalSeekableFile<SyncFile<A::File>>`. Change `SyncDirectory::SeekableFile` similarly. Use `AsyncVfsFile::` UFCS disambiguation to avoid trait name collisions when `A` also implements `VfsFile`. |
| `libsql_delta/file_handle.rs` | `SeekableSqliteFile::cursor` changes from `u64` to `Arc<AtomicU64>`. Async trait impl uses `load`/`store` on the atomic (no `&mut self`). Sync `SeekableVfsFile` impl uses same atomic — valtron for I/O, atomic for cursor. |
| `libsql_delta/mod.rs` | Update `VfsFileSystem`/`AsyncVfsFileSystem` `SeekableFile` associated types to reference the updated `SeekableSqliteFile`. |
| `memory_fs.rs` | `SeekableMemoryFile::position` changes from `u64` to `Arc<AtomicU64>`. Disambiguate `read_at`/`write_at` calls via `VfsFile::` UFCS (type implements both `VfsFile` and `AsyncVfsFile`). |
| `async_traits.rs` | **Keep** `AsyncSeekableVfsFile` trait — cursor becomes `fn position(&self) -> u64` (already `&self`). Seekable methods work with `&self` when cursor is atomic. |

## Why Not Other Approaches

| Approach | Rejected Because |
|----------|-----------------|
| `Arc<Mutex<Option<A>>>` take/put-back | Concurrent access → panic. Unacceptable. |
| `parking_lot::Mutex` / spin-wait | Adds dep or busy-loop; still fundamentally wrong to hold a lock across an await boundary in valtron |
| `Clone` on seekable with `u64` cursor | Cloning gives a new cursor — mutations on the clone are lost when dropped |
| `Clone` on seekable with `Arc<AtomicU64>` cursor | **This is what we're doing** — cheap, correct, shared state |

## `Arc<AtomicU64>` Decision

**Sensible here because:**
- The cursor is a single `u64` — fits in one atomic operation on every architecture
- Cloning is two `Arc::clone`s — no deep copy, no state divergence
- Read/write/seek all work with `&self` — no `&mut self` needed for `AsyncSeekableVfsFile`
- `Ordering::Relaxed` is sufficient — the valtron call already provides ordering guarantees for I/O

**Not a generic rule:**
- Don't replace `Mutex<u64>` with `AtomicU64` just because you can
- `MemoryFsInner::version` stays behind `RwLock` — it's mutated alongside `HashMap` operations, atomic doesn't help
- Only use `AtomicU64` for cheap, shared, single-field state (cursors, counters) that benefits from atomic ops and sharing

## Iron Rule: Valtron-Backed Tests Required

**This feature uses valtron (`SyncSeekableFile` removed, `SyncLibsqlDelta` delegates to `SyncFs`, `SyncFs::inner()` exposed). All sync-bridge seekable paths MUST be tested through valtron.**

`SyncLibsqlDelta` wraps `SyncFs<LibsqlDelta>` which bridges async ops through valtron. Tests must initialize the pool — see `backends/foundation_nativeapis/tests/valtron_executor_integration.rs. Tests go in `tests/valtron_vfs/` directory.. Seekable file concurrency tests (cloned handles sharing `Arc<AtomicU64>` cursor) must run through valtron threads to verify no panic.

**Blocked by Feature 22:** No further seekable cleanup work until Feature 22 (VFS Valtron Tests) tests the sync bridge + seekable paths through valtron.

## Principle

> Just because we *can* write a generic bridge doesn't mean we *should*.  
> Duplication in seekable is fine; a panic window in production is not.  
> `&self` methods → generic bridge (`SyncFile`).  
> `&mut self` methods → per-backend or `LocalSeekableFile` (atomic cursor + delegated I/O).

## Tasks

### Remove broken bridge (`sync_bridge.rs`)

- [x] Delete `SyncSeekableFile<A: AsyncSeekableVfsFile>` struct and all impls (`VfsFile`, `SeekableVfsFile`)
- [x] Change `SyncFs<A>::SeekableFile` from `SyncSeekableFile<A::SeekableFile>` to `LocalSeekableFile<SyncFile<A::File>>`
- [x] Change `SyncDirectory<A>::SeekableFile` similarly
- [x] Remove `AsyncSeekableVfsFile` import if no longer needed in sync_bridge
- [x] Disambiguate `AsyncVfsFile::` method calls in `SyncFile` to avoid name collisions with `VfsFile`

### Seekable cursors: `AtomicU64` (`libsql_delta/file_handle.rs`)

- [x] Change `SeekableSqliteFile::cursor` from `u64` to `Arc<AtomicU64>`
- [x] Update `AsyncSeekableVfsFile` impl — use `load`/`store` on atomic, no `&mut self` needed
- [-] Implement `Clone` for `SeekableSqliteFile` — **not needed**: both fields (`SqliteFile` and `Arc<AtomicU64>`) are already `Clone`-capable; derived `Clone` is trivial when needed but no code path requires it
- [x] Implement `SeekableVfsFile` (sync) — same atomic cursor, I/O via valtron bridge

### MemoryFs (`memory_fs.rs`)

- [x] Change `SeekableMemoryFile::position` from `u64` to `Arc<AtomicU64>`
- [x] Update sync/async seekable impls to use `load`/`store` on atomic
- [x] Disambiguate `VfsFile::` / `AsyncVfsFile::` method calls via UFCS

### SyncLibsqlDelta override (`libsql_delta/mod.rs`)

- [x] Replace `SyncLibsqlDelta` type alias with real struct wrapping `SyncFs<LibsqlDelta>`
- [x] Add `SyncSeekableSqliteFile` — `SyncFile<SqliteFile>` + `Arc<AtomicU64>` cursor
- [x] Add `SyncSqliteDirectory` — bridges async directory, returns custom seekable type
- [x] Implement `VfsFileSystem` for `SyncLibsqlDelta` — delegates all methods except `open_seekable`
- [x] Implement `DeltaStore` for `SyncLibsqlDelta` — delegates all 6 methods
- [x] Add `SyncFs::inner()` accessor for delegation
- [x] Update `LibsqlDelta::into_sync()` to return `SyncLibsqlDelta`

### Verification

- [x] `cargo check -p foundation_nativeapis` — compiles cleanly
- [x] `cargo check -p foundation_nativeapis --features vfs` — compiles cleanly
- [x] `cargo check -p foundation_nativeapis --features vfs-sqlite` — compiles cleanly
- [x] No `Arc<Mutex<Option<A>>>` patterns remain in the codebase
- [x] All async trait methods use `_async` suffix (e.g. `read_at_async`, `open_async`, `stat_async`) — zero UFCS collisions
- [x] `SyncLibsqlDelta` overrides `open_seekable` with native `SyncSeekableSqliteFile`
- [x] `SyncFs::inner()` accessor added for delegation
- [ ] Existing tests pass


## Global Rule: `foundation_errstacks` Error Handling

All VFS types MUST implement `Debug` and use `VfsResult<T>` (`Result<T, ErrorTrace<VfsError>>`) for errors. Tests use `err.current_context()` for typed error matching. See **plan.md §4d** for the full rule.

---

_Created: 2026-06-06_
