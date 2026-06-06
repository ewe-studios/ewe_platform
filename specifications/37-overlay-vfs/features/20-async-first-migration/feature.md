---
feature_name: "Async-First VFS Migration"
description: "Define async trait counterparts for all VFS traits, provide a generic sync bridge wrapper (SyncFs<A>), centralize exec_async, and migrate LibsqlDelta to implement async traits only. Sync-native backends also get async trait impls for uniform async context usage."
status: "in-progress"
priority: "critical"
phase: 3
created: 2026-06-06
updated: 2026-06-06
dependencies: ["01-core-traits"]
tasks:
  completed: 14
  uncompleted: 6
  total: 20
  completion_percentage: 70%
---

# Feature 20: Async-First VFS Migration

## Overview

The spec (plan.md, design decision #4) says "async-first, sync wraps via valtron" but this was never done. LibsqlDelta inlines ~30 async closures inside sync trait methods — each one clones Arcs, `.to_string()`s parameters, and wraps everything in `exec_future(async move { ... })`. The same exec_future helper is duplicated 3 times across files. Each sync method that should be 5 lines of async logic balloons to 15-25 lines of boilerplate.

This feature defines async trait counterparts for all VFS traits, provides a generic sync bridge wrapper (`SyncFs<A>`), and migrates LibsqlDelta to implement async traits exclusively. Sync-native backends (MemoryFs, NativeFs) also implement the async traits so they can be used uniformly in async contexts.

## Iron Law: Async-First Architecture

**The async traits own all I/O logic. Period.**

Every VFS backend falls into one of two categories:

### Category 1: Async-native backends (LibsqlDelta, TursoDelta, future S3Delta, etc.)

These backends have async-only underlying libraries (libsql, turso, aws-sdk). They:
1. Implement `AsyncVfsFileSystem` (and `AsyncDeltaStore` if applicable)
2. **Never** implement the sync traits directly
3. Get sync API for free via `SyncFs<LibsqlDelta>` — zero boilerplate per backend
4. Expose a type alias: `pub type SyncLibsqlDelta = SyncFs<LibsqlDelta>;`

### Category 2: Sync-native backends (MemoryFs, NativeFs, DirectoryDelta)

These backends use synchronous I/O (in-memory data, `std::fs`). They:
1. Implement the sync traits directly (as they do today)
2. **Also** implement the async traits — methods just return the result (no actual awaiting)
3. This lets them be used in async contexts (async tests, async composition, async pipelines)

### The Pattern

```
                    ┌──────────────────┐
                    │  Async Traits    │  ← ALL I/O logic lives here
                    │  (async_traits)  │
                    └────────┬─────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
    ┌─────────▼────┐  ┌─────▼──────┐  ┌───▼────────────┐
    │ LibsqlDelta  │  │ TursoDelta │  │ MemoryFs       │
    │ (async only) │  │ (async)    │  │ (sync + async) │
    └─────────┬────┘  └─────┬──────┘  └───┬────────────┘
              │             │             │
    ┌─────────▼────┐  ┌─────▼──────┐      │ (sync traits
    │ SyncFs<L>    │  │ SyncFs<T>  │      │  implemented
    │ (auto sync)  │  │ (auto sync)│      │  directly)
    └──────────────┘  └────────────┘      │
              │             │             │
              └──────────────┼─────────────┘
                             │
                    ┌────────▼─────────┐
                    │  Sync Traits     │  ← Public API consumed by
                    │  (traits.rs)     │     OverlayFileSystem, etc.
                    └──────────────────┘
```

## Design Decisions

### 1. `async-trait` crate, not RPITIT

`VfsDirectory` returns `Box<dyn VfsDirectory<...>>`, requiring object safety. RPITIT traits aren't object-safe because each method's return type is an opaque, distinct type per implementor. `async-trait` boxes the future, making the trait dyn-compatible.

Already used across foundation_db, foundation_auth, and 4+ other crates in this workspace.

### 2. Five async traits mirror five sync traits

Every sync trait gets an async counterpart. No exceptions.

| Sync Trait | Async Trait | Notes |
|---|---|---|
| `VfsFile` | `AsyncVfsFile` | `read_at` returns `Vec<u8>` (owned), `write_at` takes `Vec<u8>` (owned) |
| `SeekableVfsFile` | `AsyncSeekableVfsFile` | Cursor state managed by the backend, not the bridge |
| `VfsDirectory` | `AsyncVfsDirectory` | String params are `String` not `&str` |
| `VfsFileSystem` | `AsyncVfsFileSystem` | `capabilities()` stays sync (no I/O) |
| `DeltaStore` | `AsyncDeltaStore` | Extends `AsyncVfsFileSystem` |

### 3. AsyncSeekableVfsFile exists — backends own cursor state

Some backends have cursors we don't control:
- A WASI file descriptor has a kernel-level cursor
- A database result set may have server-side cursor state
- Network-backed files may have server-side seek positions
- OS file handles (`std::fs::File`) have kernel-managed seek positions

The async seekable trait lets backends manage cursor state asynchronously through their native mechanism. The sync bridge wraps `AsyncSeekableVfsFile` into `SeekableVfsFile` directly.

For backends that **don't** have native cursor support, the sync bridge also provides `SyncSeekableFile<A>` that adds local cursor tracking on top of `AsyncVfsFile::read_at`/`write_at`.

### 4. Owned parameters for Send + 'static compatibility

Async traits use `String` instead of `&str` and `Vec<u8>` instead of `&[u8]` for parameters. This is required because:
- `async-trait` requires `Send + 'static` for the future
- Borrowed references from `&self` or method parameters can't be moved into `async move {}` blocks
- The sync bridge must clone params anyway to cross the async boundary

The sync bridge methods accept `&str`/`&[u8]` (matching the sync trait signatures) and `.to_string()`/`.to_vec()` them before calling the async version.

### 5. Centralized `exec_async` replaces 3 duplicated helpers

The current codebase has 3 identical copies of the valtron bridging helper (`exec_future` in mod.rs, `exec_async` in file_handle.rs, `exec_future` in file_handle.rs). Centralize into one `exec_async` function in `exec_async.rs`.

## Async Trait Definitions

### AsyncVfsFile

```rust
#[async_trait]
pub trait AsyncVfsFile: Send + Sync {
    async fn read_at_async(&self, len: usize, offset: u64) -> VfsResult<Vec<u8>>;
    async fn write_at_async(&self, data: Vec<u8>, offset: u64) -> VfsResult<usize>;
    async fn sync_data_async(&self) -> VfsResult<()>;
    async fn size_async(&self) -> VfsResult<u64>;
    async fn truncate_async(&self, size: u64) -> VfsResult<()>;
    async fn metadata_async(&self) -> VfsResult<VfsMetadata>;
}
```

**Naming:** All methods use `_async` suffix to avoid name collisions with sync `VfsFile` methods. Types implementing both traits use UFCS-free calls: `self.read_at()` is sync, `self.read_at_async()` is async.

### AsyncSeekableVfsFile

```rust
#[async_trait]
pub trait AsyncSeekableVfsFile: AsyncVfsFile {
    async fn read_async(&mut self, len: usize) -> VfsResult<Vec<u8>>;
    async fn write_async(&mut self, data: Vec<u8>) -> VfsResult<usize>;
    async fn seek_async(&mut self, pos: SeekFrom) -> VfsResult<u64>;
    fn position_async(&self) -> u64;
}
```

**Naming convention:** All async trait methods use `_async` suffix (e.g. `read_at_async`, `open_async`, `stat_async`). This eliminates UFCS disambiguation noise when a type implements both sync and async traits. The sync methods use the base name (`read_at`, `open`, `stat`); the async ones use `_async`.

**`position_async`**: Still `&self` — reads cached local state (atomic cursor), no I/O needed.

### AsyncVfsDirectory

```rust
#[async_trait]
pub trait AsyncVfsDirectory: Send + Sync {
    type File: AsyncVfsFile + 'static;
    type SeekableFile: AsyncSeekableVfsFile + 'static;

    fn path(&self) -> String;  // owned, not &str
    async fn metadata(&self) -> VfsResult<VfsMetadata>;

    async fn list(&self) -> VfsResult<Vec<VfsDirEntry>>;
    async fn get_entry(&self, name: String) -> VfsResult<Option<VfsDirEntry>>;

    async fn create_file(&self, name: String, mode: u32) -> VfsResult<Self::File>;
    async fn create_dir(&self, name: String)
        -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    async fn remove_entry(&self, name: String) -> VfsResult<()>;
    async fn rename_entry(&self, old_name: String, new_name: String) -> VfsResult<()>;

    async fn open(&self, path: String, mode: OpenMode) -> VfsResult<Self::File>;
    async fn open_seekable(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    async fn open_directory(&self, path: String)
        -> VfsResult<Box<dyn AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile>>>;
    async fn stat(&self, path: String) -> VfsResult<VfsMetadata>;
    async fn exists(&self, path: String) -> VfsResult<bool>;

    async fn remove_all(&self, name: String) -> VfsResult<()> { /* default impl */ }
    async fn mkdir_all(&self, path: String) -> VfsResult<()> { /* default impl */ }
    async fn copy(&self, from: String, to: String) -> VfsResult<()> { /* default impl */ }
}
```

### AsyncVfsFileSystem

```rust
#[async_trait]
pub trait AsyncVfsFileSystem: Send + Sync {
    type File: AsyncVfsFile + 'static;
    type SeekableFile: AsyncSeekableVfsFile + 'static;
    type Directory: AsyncVfsDirectory<File = Self::File, SeekableFile = Self::SeekableFile> + 'static;

    fn capabilities(&self) -> VfsCapabilities;  // sync — no I/O

    async fn stat(&self, path: String) -> VfsResult<VfsMetadata>;
    async fn exists(&self, path: String) -> VfsResult<bool>;
    async fn chmod(&self, path: String, mode: u32) -> VfsResult<()>;
    async fn symlink(&self, target: String, link: String) -> VfsResult<()>;
    async fn readlink(&self, path: String) -> VfsResult<String>;
    async fn rename(&self, from: String, to: String) -> VfsResult<()>;
    async fn remove(&self, path: String) -> VfsResult<()>;

    async fn open(&self, path: String, mode: OpenMode) -> VfsResult<Self::File>;
    async fn open_seekable(&self, path: String, mode: OpenMode) -> VfsResult<Self::SeekableFile>;
    async fn open_directory(&self, path: String) -> VfsResult<Self::Directory>;
    async fn create(&self, path: String, mode: u32) -> VfsResult<Self::File>;
    async fn mkdir(&self, path: String) -> VfsResult<()>;

    async fn read_file(&self, path: String) -> VfsResult<Vec<u8>> { /* default impl */ }
    async fn write_file(&self, path: String, data: Vec<u8>) -> VfsResult<()> { /* default impl */ }
    async fn copy(&self, from: String, to: String) -> VfsResult<()> { /* default impl */ }
    async fn remove_all(&self, path: String) -> VfsResult<()> { /* default impl */ }
    async fn mkdir_all(&self, path: String) -> VfsResult<()> { /* default impl */ }
}
```

### AsyncDeltaStore

```rust
#[async_trait]
pub trait AsyncDeltaStore: AsyncVfsFileSystem {
    async fn add_whiteout(&self, path: String, version: u64) -> VfsResult<()>;
    async fn is_whiteout(&self, path: String) -> VfsResult<Option<u64>>;
    async fn remove_whiteout(&self, path: String) -> VfsResult<()>;
    async fn list_whiteouts(&self, dir: String) -> VfsResult<Vec<(String, u64)>>;
    async fn flush(&self) -> VfsResult<()>;
    async fn reset(&self) -> VfsResult<()>;
}
```

## Sync Bridge Structs (`sync_bridge.rs`)

### SyncFile<A: AsyncVfsFile>

Wraps `Arc<A>`, implements `VfsFile`. Each method clones the Arc, converts params to owned, calls `exec_async(async move { inner.method().await })`, copies result into caller's buffer.

```rust
pub struct SyncFile<A: AsyncVfsFile + 'static> {
    inner: Arc<A>,
}

impl<A: AsyncVfsFile + 'static> VfsFile for SyncFile<A> {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> {
        let inner = self.inner.clone();
        let len = buf.len();
        let data = exec_async(async move { inner.read_at(len, offset).await })?;
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }
    // ... other methods follow same pattern
}
```

### SyncSeekableFile<A: AsyncSeekableVfsFile>

Wraps the async seekable file directly, delegates to its cursor management.

```rust
pub struct SyncSeekableFile<A: AsyncSeekableVfsFile + 'static> {
    inner: Arc<tokio::sync::Mutex<A>>,  // needs &mut self for seek/read/write
}
```

Wait — `async-trait` methods that take `&mut self` mean the future borrows `self` mutably. For the sync bridge, we need interior mutability. Since the bridge is sync and single-threaded per call, a `std::sync::Mutex` suffices.

Actually, the simpler approach: the bridge holds the `A` behind a `Mutex`, locks it for each call, and runs the async method. No need for async Mutex since we're in sync context.

```rust
pub struct SyncSeekableFile<A: AsyncSeekableVfsFile + 'static> {
    inner: Arc<std::sync::Mutex<A>>,
}

impl<A: AsyncSeekableVfsFile + 'static> SeekableVfsFile for SyncSeekableFile<A> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let inner = self.inner.clone();
        let len = buf.len();
        let data = exec_async(async move {
            let mut guard = inner.lock().unwrap();
            guard.read(len).await
        })?;
        let n = data.len().min(buf.len());
        buf[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }
    // ...
}
```

Note: `Mutex::lock()` inside an async block is safe here because the future is executed via valtron (not inside a tokio runtime), so there's no risk of holding a sync lock across an await point in a cooperative scheduler.

### LocalSeekableFile<A: AsyncVfsFile>

For backends that **don't** implement `AsyncSeekableVfsFile` but do implement `AsyncVfsFile`, the bridge provides local cursor tracking:

```rust
pub struct LocalSeekableFile<A: AsyncVfsFile + 'static> {
    inner: SyncFile<A>,
    cursor: u64,
}

impl<A: AsyncVfsFile + 'static> SeekableVfsFile for LocalSeekableFile<A> {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inner.read_at(buf, self.cursor)?;
        self.cursor += n as u64;
        Ok(n)
    }
    // seek, write, position — pure cursor math + delegation
}
```

This replaces the current `SeekableSqliteFile` pattern. Any `AsyncVfsFile` backend gets seekable support for free.

### SyncDirectory<A: AsyncVfsDirectory>

Wraps `Arc<A>`, implements `VfsDirectory`. Returns `SyncFile<A::File>` for file operations.

```rust
pub struct SyncDirectory<A: AsyncVfsDirectory + 'static> {
    inner: Arc<A>,
}
```

### SyncFs<A: AsyncVfsFileSystem>

The main entry point. Wraps `Arc<A>`, implements `VfsFileSystem`.

```rust
pub struct SyncFs<A: AsyncVfsFileSystem + 'static> {
    inner: Arc<A>,
}

impl<A: AsyncVfsFileSystem + 'static> VfsFileSystem for SyncFs<A> {
    type File = SyncFile<A::File>;
    type SeekableFile = SyncSeekableFile<A::SeekableFile>;
    type Directory = SyncDirectory<A::Directory>;
    // ...
}

impl<A: AsyncDeltaStore + 'static> DeltaStore for SyncFs<A> {
    // auto-bridges all DeltaStore methods
}
```

## exec_async.rs — Centralized Bridge

```rust
use foundation_core::valtron::{collect_one, execute, from_future, Stream};
use foundation_errstacks::ErrorTrace;
use crate::shared::vfs::error::{VfsError, VfsResult};

pub fn exec_async<T: Send + 'static, F>(future: F) -> VfsResult<T>
where
    F: std::future::Future<Output = VfsResult<T>> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None)
        .map_err(|e| ErrorTrace::new(VfsError::Backend {
            message: format!("valtron execution failed: {e}"),
        }))?;
    let result: Option<Result<T, ErrorTrace<VfsError>>> = collect_one(stream);
    match result {
        Some(Ok(v)) => Ok(v),
        Some(Err(e)) => Err(e),
        None => Err(ErrorTrace::new(VfsError::Backend {
            message: "no result from async execution".into(),
        })),
    }
}
```

Replaces 3 identical copies of this function across the codebase.

## Before vs After

### Before — `stat` in LibsqlDelta (10 lines, inline async)

```rust
fn stat(&self, path: &str) -> VfsResult<VfsMetadata> {
    let ino = exec_future(resolve_path_async(Arc::clone(&self.conn), path.to_string()))?;
    let conn = Arc::clone(&self.conn);
    let path_str = path.to_string();
    exec_future(async move {
        let mut stmt = conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?")
            .await.map_err(err)?;
        let row = stmt.query([ino]).await.map_err(err)?
            .next().await.map_err(err)?
            .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path: path_str }))?;
        Ok(SqliteDentry::from_row(&row)?.to_metadata())
    })
}
```

### After — `stat` in LibsqlDelta (6 lines, pure async)

```rust
async fn stat(&self, path: String) -> VfsResult<VfsMetadata> {
    let ino = resolve_path_async(self.conn.clone(), path.clone()).await?;
    let mut stmt = self.conn.prepare("SELECT * FROM sqlite_dentry WHERE ino = ?")
        .await.map_err(err)?;
    let row = stmt.query([ino]).await.map_err(err)?
        .next().await.map_err(err)?
        .ok_or_else(|| ErrorTrace::new(VfsError::NotFound { path }))?;
    Ok(SqliteDentry::from_row(&row)?.to_metadata())
}
```

The sync version is handled by `SyncFs<LibsqlDelta>` automatically — zero per-method code.

### Before — `SeekableSqliteFile` (60 lines, per-backend boilerplate)

```rust
pub struct SeekableSqliteFile {
    pub inner: SqliteFile,
    pub cursor: u64,
}

impl VfsFile for SeekableSqliteFile {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> VfsResult<usize> { self.inner.read_at(buf, offset) }
    fn write_at(&self, buf: &[u8], offset: u64) -> VfsResult<usize> { self.inner.write_at(buf, offset) }
    // ... 4 more delegated methods
}

impl SeekableVfsFile for SeekableSqliteFile {
    fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> { ... }
    fn write(&mut self, buf: &[u8]) -> VfsResult<usize> { ... }
    fn seek(&mut self, pos: SeekFrom) -> VfsResult<u64> { ... }
    fn position(&self) -> u64 { self.cursor }
}
```

### After — deleted, replaced by `LocalSeekableFile<SqliteFile>` (0 per-backend lines)

The generic `LocalSeekableFile<A>` in sync_bridge.rs handles this for ALL backends that don't have native cursor support. `SeekableSqliteFile` is deleted entirely.

## What changes

| File | Change |
|---|---|
| `Cargo.toml` | Add `async-trait` optional dep behind `vfs` feature |
| `shared/vfs/async_traits.rs` | **NEW** — async trait definitions |
| `shared/vfs/exec_async.rs` | **NEW** — centralized valtron bridge |
| `shared/vfs/sync_bridge.rs` | **NEW** — SyncFile, LocalSeekableFile, SyncDirectory, SyncFs. ~~SyncSeekableFile~~ removed in Feature 21. |
| `shared/vfs/mod.rs` | Add 3 new module declarations + re-exports |
| `shared/vfs/libsql_delta/mod.rs` | Implement AsyncVfsFileSystem + AsyncDeltaStore instead of sync traits. Delete exec_future, schedule_future, wrap_async, to_threaded_iter. Add `pub type SyncLibsqlDelta = SyncFs<LibsqlDelta>;` |
| `shared/vfs/libsql_delta/file_handle.rs` | Implement AsyncVfsFile + AsyncVfsDirectory instead of sync traits. Delete SeekableSqliteFile. Delete local exec_async/exec_future. Delete unsafe Send/Sync impls. |

## What stays unchanged

- `traits.rs` — sync traits are the public API, untouched
- `memory_fs.rs`, `memory_delta.rs` — sync-native, keep sync trait impls (async impls added later as separate task)
- `native_fs.rs`, `dir_delta.rs` — sync-native, keep sync trait impls
- `overlay_fs.rs` — composition logic, uses sync traits
- `libsql_delta/chunking.rs` — already standalone async fns
- `libsql_delta/path_resolve.rs` — already standalone async fns
- `libsql_delta/types.rs`, `schema.rs` — data types
- `turso_delta/` — separately broken (75 errors), separate fix later

## Tasks

### Dependencies

- [x] Feature 01 (core traits) must be complete — it is (22/22 tasks done)

### Cargo.toml

- [x] Add `async-trait = "0.1"` as regular dependency (not feature-gated — lightweight proc macro)

### New modules — Trait definitions (`src/shared/vfs/async_traits.rs`)

- [x] Define `AsyncVfsFile` trait with `read_at(len, offset) -> Vec<u8>`, `write_at(data: Vec<u8>, offset)`, `sync_data`, `size`, `truncate`, `metadata`
- [x] Define `AsyncSeekableVfsFile: AsyncVfsFile` trait with `read(len) -> Vec<u8>`, `write(data: Vec<u8>)`, `seek(pos)`, `position()`
- [x] Define `AsyncVfsDirectory` trait with all directory methods (String params), associated types `File: AsyncVfsFile`, `SeekableFile: AsyncSeekableVfsFile`, default impls for `remove_all`, `mkdir_all`, `copy`
- [x] Define `AsyncVfsFileSystem` trait with all filesystem methods (String params), associated types, `capabilities()` as sync, default impls for `read_file`, `write_file`, `copy`, `remove_all`, `mkdir_all`
- [x] Define `AsyncDeltaStore: AsyncVfsFileSystem` trait with whiteout + lifecycle methods

### New modules — Centralized bridge (`src/shared/vfs/exec_async.rs`)

- [x] Implement `exec_async<T, F>(future) -> VfsResult<T>` using valtron's `from_future` + `execute` + `collect_one`

### New modules — Sync bridge (`src/shared/vfs/sync_bridge.rs`)

- [x] Implement `SyncFile<A: AsyncVfsFile>` — stores `Arc<A>`, implements `VfsFile`
- [x] Implement `LocalSeekableFile<A: AsyncVfsFile>` — stores `SyncFile<A>` + cursor, implements `VfsFile` + `SeekableVfsFile` (for backends without native cursor)
- [x] Implement `SyncDirectory<A: AsyncVfsDirectory>` + `SyncDynDirectory` — stores `Arc<A>`, implements `VfsDirectory`
- [x] Implement `SyncFs<A: AsyncVfsFileSystem>` — stores `Arc<A>`, implements `VfsFileSystem`
- [x] ~~`SyncSeekableFile<A>`~~ — removed in Feature 21 (dangerous `Arc<Mutex<Option<A>>>` pattern)
- [x] Implement `DeltaStore for SyncFs<A>` where `A: AsyncDeltaStore`

### Module registration (`src/shared/vfs/mod.rs`)

- [x] Add `pub mod async_traits;`, `pub mod exec_async;`, `pub mod sync_bridge;`
- [x] Add re-exports for async traits and bridge types

### Migrate LibsqlDelta (`src/shared/vfs/libsql_delta/`)

- [x] `file_handle.rs`: SqliteFile implements `AsyncVfsFile`, SqliteDirectory implements `AsyncVfsDirectory`, SeekableSqliteFile implements `AsyncSeekableVfsFile`. All exec_async wrappers removed. All `unsafe impl Send/Sync` removed. Methods are pure async.
- [x] `mod.rs`: LibsqlDelta implements `AsyncVfsFileSystem + AsyncDeltaStore`. All methods pure async. `exec_future`, `schedule_future`, `wrap_async`, `to_threaded_iter` deleted. `SyncLibsqlDelta` is a real struct wrapping `SyncFs<LibsqlDelta>` with custom seekable (see Feature 21). `into_sync()` returns `SyncLibsqlDelta`.

### Remaining (Future Work)

- [x] Add `AsyncVfsFileSystem` impl to MemoryFs (done — sync methods wrapped in async, `AsyncMemoryDirectory` wrapper for dyn dirs)
- [x] Add `AsyncVfsFileSystem` impl to MemoryFs types (`MemoryFile`, `SeekableMemoryFile`, `MemoryDirectory`)
- [x] Add `AsyncDeltaStore` impl to MemoryDelta (sync-native, delegates to sync methods)
- [ ] Add `AsyncVfsFileSystem` impl to NativeFs (not yet implemented)
- [ ] Add `AsyncVfsDirectory` impl to DirectoryDelta (not yet implemented)
- [ ] Migrate TursoDelta to async traits (separate — has 75 compilation errors)
- [ ] Consider async OverlayFileSystem composition

## Verification

1. `cargo check -p foundation_nativeapis --features vfs` — async traits + sync bridge + exec_async compile
2. `cargo check -p foundation_nativeapis --features vfs-sqlite` — LibsqlDelta compiles with async traits, `SyncLibsqlDelta` satisfies `VfsFileSystem + DeltaStore`
3. `cargo check -p foundation_nativeapis --features vfs-native` — NativeFs/DirectoryDelta unaffected
4. `cargo check -p foundation_nativeapis` — default features still work
5. Existing overlay/memory tests pass unchanged
6. No duplicate `exec_future`/`exec_async` definitions remain in libsql_delta

## Future Work (Not This Feature)

- Add `AsyncVfsFileSystem` impls to MemoryFs, MemoryDelta (trivial — sync methods wrapped in async)
- Add `AsyncVfsFileSystem` impls to NativeFs, DirectoryDelta (use `tokio::fs` or `spawn_blocking`)
- Migrate TursoDelta to async traits (separate, has 75 compilation errors)
- Consider async OverlayFileSystem composition

---

_Created: 2026-06-06 | Updated: 2026-06-06_
