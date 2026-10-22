---
feature: "Remaining Sync Twins"
description: "Finish the async-first work: the vector store and the virtual file system still have sync traits with input/output-backed implementations; the storage trait methods keep their _async suffix; record the sync boundaries that stay."
status: "in-progress"
priority: "high"
depends_on: ["04-async-only-storage"]
builds_on:
  - "PR #133 (sync storage traits deleted)"
  - "PR #134 (Async* storage traits renamed to plain names)"
estimated_effort: "large"
created: 2026-10-22
last_updated: 2026-10-22
author: "Main Agent"
tasks:
  completed: 2
  uncompleted: 2
  total: 4
  completion_percentage: 50
---

# Remaining Sync Twins

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** input/output (I/O); virtual file system (VFS); inter-process
communication (IPC); Hypertext Transfer Protocol version 1.1 (HTTP/1.1); operating system (OS);
pull request (PR); application programming interface (API); Structured Query Language (SQL);
Network File System (NFS); Filesystem in Userspace (FUSE); Network File System version 3 (NFSv3);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

## 1. Why this feature exists

Features 01 to 04 and PRs #130 to #134 made the storage layer async-only. A survey of `master` on
2026-10-22 found more code with the same problem: a **sync twin**. A sync twin is a sync trait
next to an async trait for the same thing, where some implementations of the sync trait do I/O.
Sync I/O blocks a valtron worker, and it cannot run on Cloudflare Workers at all.

The rule (`CLAUDE.md` rule 6): async all the way; sync only at real boundaries.

## 2. What the survey found

| Area | Sync trait | Async trait | Sync implementations that do I/O | Callers of the sync trait |
|---|---|---|---|---|
| `foundation_vectors` | `VectorStore` | `AsyncVectorStore` | `TurboPufferVectorStore` (network), `SqliteVecVectorStore` (SQLite), `FjallVectorStore` (disk) | `foundation_ai` `shed` tool (`ToolDiscovery`) |
| `foundation_nativeapis` VFS | `VfsFile`, `SeekableVfsFile`, `VfsDirectory`, `VfsFileSystem`, `DeltaStore` | `AsyncVfsFile`, `AsyncSeekableVfsFile`, `AsyncVfsDirectory`, `AsyncVfsFileSystem`, `AsyncDeltaStore` | `TursoDelta`, `SyncLibsqlDelta` (database), `VfsClient` (IPC), `NativeFs`, `DirectoryDelta`, `InodeFs` / `InodeDelta` (Fjall, disk) | `OverlayFileSystem`, `ObservableFs`, `search`, `dynfs`; `SyncFs<A>` runs an async VFS behind the sync traits |

Memory-only implementations (`InMemoryVectorStore`, `MemoryFs`, `MemoryDelta`) do no I/O. They
are not a problem by themselves.

### 2.1 Sync code that stays (real boundaries)

These sync waits are correct. Each one runs on its own OS thread, not on a valtron worker:

| Place | Why it is a boundary |
|---|---|
| `foundation_proxy/src/control.rs` | Each control-socket connection runs on its own OS thread. |
| `foundation_proxy/src/health.rs` | A probe runs on its own thread with a deadline, so a hung probe cannot stall shutdown. |
| `foundation_proxy/src/rpc_relay.rs` | The HTTP/1.1 front end of `foundation_http` is a sync server today. The relay waits per request and per chunk on that server's thread. Making the HTTP/1.1 server async is backlog §3 (`foundation_http`), not this feature. |
| `foundation_keychain/src/bin/keychain.rs` | Process entry point (`main`). |
| `foundation_macros` (`valtron_entry`, `docker_container`) | Generated test and entry wrappers: the boundary by design. |

## 3. Tasks

| Task | What | Done when | State |
|---|---|---|---|
| T1 | **Vectors:** make every vector store implement `AsyncVectorStore` only (`TurboPufferVectorStore`, `SqliteVecVectorStore`, `FjallVectorStore`, `InMemoryVectorStore`); move the `shed` tool to the async trait; delete the sync `VectorStore`; rename `AsyncVectorStore` to `VectorStore` (and its methods, if task T3 decides so). | No sync vector trait remains; `ci.py run foundation_vectors` and `foundation_ai` pass. | Done (branch `db/async-vectors`); see section 5 |
| T2 | **VFS:** async-first core (decision 1). Move `TursoDelta`, `SyncLibsqlDelta`, `VfsClient`, `NativeFs`, `DirectoryDelta` and the Fjall stores to the async traits only; keep `SyncFs<A>` as the only sync face for I/O stores; move `OverlayFileSystem`, `ObservableFs`, `search` and `dynfs` to the async traits, with sync wrappers only where a sync consumer needs them. | No I/O-backed type implements a sync VFS trait directly; `ci.py run foundation_nativeapis` and its feature runs pass. | Done (branch `nativeapis/async-vfs`), except `TursoDelta` (open decision); see section 6 |
| T3 | **Method suffixes:** drop `_async` from the storage trait methods (decision 2). Check each type for inherent methods with the plain names first. | No storage trait method ends in `_async`; workspace, wasm and example builds pass. | Open |
| T4 | **Records:** `progress.md`, `learnings.md`, the backlog. | Updated in the same PRs. | Updated for T1 and T2 |

## 4. Owner decisions (2026-10-22)

1. **VFS scope: async-first core, sync only as a bridge (option a).** Every I/O implementation
   (`TursoDelta`, `SyncLibsqlDelta`, `VfsClient`, `NativeFs`, `DirectoryDelta`, the Fjall
   stores) implements the async traits only. The sync traits stay only for consumers that must be
   sync (for example a SQLite VFS, which SQLite calls synchronously). Those consumers wrap an
   async VFS in `SyncFs<A>` at that boundary. Memory-only stores may keep both faces.
2. **Method suffixes: drop them (task T3).** One mechanical PR after task T1: `get_async` → `get`,
   `query_async` → `query`, and so on, with a clash check per type first.

Order: T1 (vectors), then T2 (VFS), then T3 (method suffixes), then spec 65 feature 07.

## 5. Task T1 result (vector stores)

**Owner decisions during T1 (2026-10-22):**

1. The `shed` tool keeps `register` / `deregister` sync. They queue the index change, and the
   async search applies the queue first. A failed change is reported to the caller, not only
   logged.
2. `len_async` returns `Result<usize, VectorStoreError>`.
3. The `TurboPuffer` `get` and `len` bugs are fixed in T1.

**Breaking application programming interface (API) changes:**

| Crate | Change |
|---|---|
| `foundation_vectors` | The sync `VectorStore` trait is gone. `AsyncVectorStore` is renamed to `VectorStore`. Methods keep `_async` (task T3). |
| `foundation_vectors` | `len_async` returns `Result<usize, VectorStoreError>`. New default method `is_empty_async`. |
| `foundation_vectors` | `VectorStoreError` has a new variant `Unsupported(String)`, and implements `std::error::Error`. |
| `foundation_vectors` | `TurboPufferVectorStore::new` returns `Result` (it refuses `DistanceMetric::Dot`). New `with_limits` and `TurboPufferLimits`. The store uses the v2 API (`/v2/namespaces/...`). |
| `foundation_vectors` | `SqliteVecVectorStore::open` is `async` and refuses `DistanceMetric::Dot`. The `vec0` table schema changed (see the backlog). |
| `foundation_vectors` | `hybrid_search` is `async` and returns `Result`. |
| `foundation_ai` | `ToolDiscovery::{index, index_tool, remove, search}` are `async`. New `queue_index`, `queue_remove`, `apply_pending`. |
| `foundation_ai` | `ToolCallManager::search_tools` is `async` and returns `ShedResult` (new field `index_failures`). `enable_discovery` and `ToolShed::build` are `async`. New type `IndexFailure`. |

**Sync boundaries that stay:** fjall has only a sync API, so `FjallVectorStore` calls it inline
(the same choice as `FjallDocumentStore` in feature 04). `SqliteVecVectorStore::open` writes and
loads the extension with sync calls, once. Both are in the backlog.

**Checks (2026-10-22):**

| Check | Result |
|---|---|
| `ci.py run foundation_vectors --step build --step clippy --step test --step wasm` (runs `""` and `sql-store`) | pass |
| `cargo test -p foundation_vectors --features sql-store,turbopuffer,fjall-store,sqlite-vec` | pass |
| `ci.py run foundation_ai`, steps one at a time (build, clippy, test, wasm) | pass; test 2745 pass, 0 fail |
| `cargo check --workspace --tests` | pass |
| `apps/`, `examples/` | no use of the changed APIs |

## 6. Task T2 result (virtual file system)

**What changed, per type:**

| Type | Before | After |
|---|---|---|
| `NativeFs` (and its file, seekable file, directory) | sync traits | async traits only; `std::fs` calls inline (see the backlog) |
| `DirectoryDelta`, `FilteredDirectory` | sync traits (`scaffold_impl`) | async traits only, written out |
| `VfsClient`, `RemoteFile`, `RemoteSeekableFile`, `RemoteDirectory` | sync traits | async traits only |
| `VfsDaemon` | served a sync VFS | serves an `AsyncVfsFileSystem`; `dispatch` is `async`; new sync `release` for close requests |
| `VfsTransport` | sync `request` | `async fn request` and a sync `release` (for `Drop`) |
| `LibsqlDelta` | async traits; constructors blocked through `exec_async` | constructors and `sync_remote` are `async` |
| `SyncLibsqlDelta`, `SyncSqliteDirectory`, `SyncSeekableSqliteFile`, `SyncLibsqlDeltaGeneric`, `LibsqlDelta::into_sync` | sync wrappers | deleted; use `SyncFs<LibsqlDelta>` |
| `OverlayFileSystem`, `OverlayDirectory`, `OverlayFile`, `OverlaySeekableFile` | sync traits over sync layers | async traits over async layers; new `from_arcs` |
| `ObservableFs`, `ObservableFile`, `ObservableSeekableFile` | sync traits | async traits |
| `VfsSearcher`, `InCodeVfsSearcher` and the factories | sync `search` over `VfsFileSystem` | `async fn search` over `AsyncVfsFileSystem` |
| `VfsNfs` | `VfsFileSystem` | `AsyncVfsFileSystem` (the NFS server is async) |
| `VfsTask`, `VfsTaskBuilder` | `ObservableFs<F: VfsFileSystem>` | `F: AsyncVfsFileSystem` |
| `AsyncVfsFileSystem` | no inode methods | `inode_async`, `path_by_inode_async`, `stat_by_inode_async` (defaults) |
| `SyncFs` | inode methods approximated or unsupported; whole-file helpers through the defaults | delegates every method, including the inode and whole-file methods, to the async store |
| `exec_async` | always went to a valtron pool | polls once on the calling thread first; new `poll_once` |
| `DynFs` | sync | stays sync (its only users, the shim and ptrace, are sync boundaries); new `DynFs::from_async` |
| `foundation_ai` `FileSearch` | sync `search` | `async fn search`; `SearchFileTool::from_vfs` / `native` take an `AsyncVfsFileSystem` |

**Sync boundaries that stay (they wrap an async VFS in `SyncFs`):** the
`LD_PRELOAD` shim (libc calls), the ptrace tracer (syscall stops), the FUSE
adapter (`fuser` calls back synchronously), `run_daemon_loop` (a thread on
the sync IPC bus) and example `main` functions.

**Stores that keep the sync face:** `MemoryFs` and `MemoryDelta` (both
faces, decision 1), and `InodeFs` / `InodeDelta`. The survey called these
"Fjall stores", but they keep everything in memory and do no I/O; they keep
only the sync face (see the backlog).

**Not done:** `TursoDelta` still has its sync trait impls. It has never
compiled (83 errors when its module is built). The owner must decide: delete
it (`LibsqlDelta` with `TursoRemoteConfig` already covers a Turso cloud
database), or rebuild it on `libsql` behind the async traits. The
recommendation is to delete it. `FjallDocumentStore` still uses the sync VFS
traits (it is a consumer, not a store); see the backlog.

**Breaking application programming interface (API) changes:**

| Crate | Change |
|---|---|
| `foundation_nativeapis` | `NativeFs`, `DirectoryDelta`, `VfsClient`, `OverlayFileSystem`, `ObservableFs` no longer implement the sync VFS traits. Use the async traits, or `SyncFs::new(..)` at a sync boundary. |
| `foundation_nativeapis` | `SyncLibsqlDelta` and its types are deleted. `LibsqlDelta::{new, with_config, in_memory, with_turso_replica, sync_remote}` are `async`. |
| `foundation_nativeapis` | `VfsTransport::request` is `async`; new required method `release`. `VfsClient::new` is `async`. `VfsDaemon::dispatch` is `async`. `run_daemon_loop` takes `&Arc<VfsDaemon<F>>`. |
| `foundation_nativeapis` | `VfsSearcher::search` is `async`; `InCodeVfsSearcher`, `vfs_searcher` and `native_vfs_searcher` take an `AsyncVfsFileSystem`. |
| `foundation_nativeapis` | `VfsNfs<F>`, `OverlayFileSystem<B, D>`, `ObservableFs<F>`, `VfsTask::from_observable` need the async traits. `OverlayFileSystem::new` keeps its signature. |
| `foundation_ai` | `FileSearch::search` is `async`. |

**Bugs fixed in T2 (each has a test):**

| Bug | Test |
|---|---|
| `NativeFs` mapped a path with `..` to the root, so `remove("/a/..")` aimed at the root. | `native_vfs::dot_dot_paths_are_refused_not_mapped_to_root` |
| A `NativeFs` directory handle joined names without the root checks (`open_directory("..")` left the root). | `native_vfs::directory_handles_cannot_leave_the_root` |
| `DirectoryDelta` hid `.wh.*` sentinels only in the top directory. | `dir_delta_vfs::subdirectories_hide_whiteout_sentinels` |
| `OverlayDirectory` held raw pointers to the layers (a dangling pointer after the overlay was dropped). | `vfs_async::overlay::directory_handle_outlives_the_overlay` |
| `create_file` on an overlay directory handle of a base-only directory failed (no delta parents). | `vfs_async::overlay::create_file_in_a_base_only_directory` |
| NFS `remove` and `rename` ignored the directory id. | `nfs_vfs::test_nfs_remove_and_rename_in_subdirectory` |
| A remote directory handle joined an absolute path to its own path. | `vfs_async::ipc::directory_handles_accept_absolute_paths` |
| `ClientTransport` sent under one lock and received under another, so two concurrent requests could swap responses. | none (needs the IPC bus); one lock now covers both |
| `vfs-sqlite` did not compile (`scru128` was not a dependency). | the `valtron_vfs` SQLite tests build and run again |
