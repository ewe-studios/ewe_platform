//! Integration tests: VFS through valtron executor.
//!
//! All sync-bridge code paths are exercised through a valtron-initialized pool.
//! Direct sync calls bypass the bridge and do NOT test the production code path.
//!
//! Sub-modules:
//! - `memory` — SyncFs<MemoryFs> and SyncFs<MemoryDelta> through valtron
//! - `sqlite` — SyncLibsqlDelta through valtron
//! - `async` — AsyncVfsFileSystem traits called through valtron futures
//! - `overlay` — OverlayFileSystem through valtron
//! - `seekable` — Seekable file concurrency + cursor sharing via valtron
//! - `errors` — Error propagation through valtron bridge

#![cfg(feature = "vfs")]

mod memory;
mod async_traits;
mod overlay;
mod errors;
mod vfs_task;

#[cfg(feature = "vfs-sqlite")]
mod sqlite;
#[cfg(feature = "vfs-sqlite")]
mod seekable;
