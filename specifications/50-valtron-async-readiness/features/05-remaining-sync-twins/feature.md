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
last_updated: 2026-10-27
author: "Main Agent"
tasks:
  completed: 3
  uncompleted: 1
  total: 4
  completion_percentage: 75
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
| T2 | **VFS:** async-first core (decision 1). Move `TursoDelta`, `SyncLibsqlDelta`, `VfsClient`, `NativeFs`, `DirectoryDelta` and the Fjall stores to the async traits only; keep `SyncFs<A>` as the only sync face for I/O stores; move `OverlayFileSystem`, `ObservableFs`, `search` and `dynfs` to the async traits, with sync wrappers only where a sync consumer needs them. | No I/O-backed type implements a sync VFS trait directly; `ci.py run foundation_nativeapis` and its feature runs pass. | Done (branch `nativeapis/async-vfs`); `TursoDelta` deleted (rebuild: ewe-studios/ewe_platform#142); see section 6 |
| T3 | **Method suffixes:** drop `_async` from the storage trait methods (decision 2). Check each type for inherent methods with the plain names first. | No storage trait method ends in `_async`; workspace, wasm and example builds pass. | Done (branch `db/drop-async-suffixes`); see section 7 |
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

**Deleted:** `TursoDelta` never compiled (83 errors when its module was
built). By owner decision it is deleted, with its example, the `vfs-turso`
feature, the `turso` dependency and the shim's `turso` backend. `LibsqlDelta`
with `TursoRemoteConfig` covers a Turso cloud database. The rebuild on the
`turso` crate (async-only, local engine) is ewe-studios/ewe_platform#142.

**Not done:** `FjallDocumentStore` still uses the sync VFS traits (it is a
consumer, not a store); see the backlog.

**Breaking application programming interface (API) changes:**

| Crate | Change |
|---|---|
| `foundation_nativeapis` | `NativeFs`, `DirectoryDelta`, `VfsClient`, `OverlayFileSystem`, `ObservableFs` no longer implement the sync VFS traits. Use the async traits, or `SyncFs::new(..)` at a sync boundary. |
| `foundation_nativeapis` | `TursoDelta`, the `vfs-turso` feature and the shim's `turso` backend (`FOUNDATION_VFS_DELTA=turso`) are deleted (rebuild: ewe-studios/ewe_platform#142). |
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
| `VfsNfs` gave the first new entry id 1, the root's id, so the root mapped to that entry. | `nfs_vfs::test_nfs_remove_and_rename_in_subdirectory` (and the old `test_nfs_remove_file`) |
| `NativeFs::create` returned a `ReadWrite` handle over a write-only host file. | `native_vfs::created_files_can_be_read_back` |
| `LibsqlDelta::write_file_async` failed for a new file, kept the old chunks on overwrite (UNIQUE constraint), and sent invalid SQL for empty data. | `valtron_vfs::sqlite::sync_libsql_read_file_write_file` |

**Checks (2026-10-22):**

| Check | Result |
|---|---|
| `ci.py run foundation_nativeapis --step fmt --step build --step clippy --step test` (runs `""` and `vfs-native,vfs-d1,vfs-r2`; fmt is report-only for this crate; no wasm step) | pass |
| `cargo check -p foundation_nativeapis --tests --examples --features vfs-native,vfs-search-fff,vfs-sqlite,vfs-ipc,vfs-fjall,vfs-nfs,vfs-ptrace,vfs-d1,vfs-r2,vfs-arrow` | pass |
| `cargo test -p foundation_nativeapis` with those features (less `vfs-ptrace` and `vfs-arrow`): `native_vfs`, `dir_delta_vfs`, `overlay_vfs`, `observable_vfs`, `vfs_search`, `ipc_vfs`, `valtron_vfs`, `memory_vfs`, `nfs_vfs`, `vfs_async`, `blob_delta_vfs`, `d1_delta_vfs`, `r2_delta_vfs`, `fjall_vfs`, `fjall_document_store` | pass |
| `cargo check -p foundation_ai --no-default-features --features testing --tests` | pass |
| `ci.py run foundation_ai`, `cargo test` of `foundation_ai`, `cargo check --workspace --tests` | not run: the disk was full (other sessions built in the shared target at the same time). A grep found no other user of the changed APIs in `apps/`, `examples/`, `bin/`, `backends/` or `infrastructure/`. |
| `vfs-preload`, `vfs-fuse` | not built: they did not build before this change (see the backlog) |

## 7. Task T3 result (method suffixes)

**Scope.** Task T3 removes the `_async` suffix from the methods of the
storage traits. These traits are async only, so the suffix tells the reader
nothing:

| Crate | Trait | Methods (old name → new name) |
|---|---|---|
| `foundation_db` | `QueryStore` | `query`, `execute`, `execute_batch` |
| `foundation_db` | `KeyValueStore` | `get`, `set`, `delete`, `exists`, `list_keys` |
| `foundation_db` | `BlobStore` | `put_blob`, `get_blob`, `delete_blob`, `blob_exists` |
| `foundation_db` | `RateLimiterStore` | `check_rate_limit`, `record_rate_limit`, `reset_rate_limit` |
| `foundation_db` | `DocumentStore` | `append`, `append_with_id`, `append_promotable`, `append_promotable_with_id`, `scan_documents`, `scan_documents_from`, `scan`, `scan_all`, `scan_from`, `delete`, `delete_all`, `count` |
| `foundation_db` | `PasskeyStore`, `TosStore`, `AuthStore` | `store_passkey`, `find_passkeys_by_user`, `find_passkey_by_id`, `find_passkey_by_credential_id`, `update_passkey_counter`, `update_passkey_name`, `delete_passkey`, `store_tos_acceptance`, `find_tos_acceptance`, `find_latest_tos_acceptance`, `find_user_by_email` |
| `foundation_vectors` | `VectorStore` | `insert`, `delete`, `search`, `get`, `len`, `is_empty` |
| `foundation_auth` | `CredentialStore` | `get`, `set`, `delete`, `exists`, `list_keys` |
| `foundation_ai` | `MemoryStore` | `get`, `set`, `hydrate`, `clear` |

Each new name is the old name less `_async` (for example `get_async` →
`get`). `StateStore` and `StorageCleanup` had plain names before.
`CredentialStore` and `MemoryStore` are not in `foundation_db`, but they are
async-only storage traits over the `foundation_db` stores, so they are in
scope.

**Not in scope: the VFS traits.** The async VFS traits (`AsyncVfsFile`,
`AsyncSeekableVfsFile`, `AsyncVfsDirectory`, `AsyncVfsFileSystem`,
`AsyncDeltaStore`) keep `_async` (for example `inode_async`,
`read_file_async`). Decision 1 keeps a sync twin for each of them
(`VfsFile`, `VfsFileSystem`, ...), with the same method names less the
suffix. `MemoryFs` and `MemoryDelta` implement both faces, and `SyncFs`
gives the sync face over an async store. If the suffix goes, a call such as
`fs.read_file(..)` with both traits in scope is ambiguous, and the method
name no longer shows which face a caller uses. The trait names keep their
`Async` prefix for the same reason. Section 3 and decision 2 name the
storage traits only. If the owner wants the VFS methods renamed too, that is
a separate task, after the sync faces are reduced.

**Clash check (per type, before the rename).**

| Type | Clash | Resolution |
|---|---|---|
| `D1Store`, `LibsqlStore` | They implement `KeyValueStore` and `StateStore`. Both traits now have `get`, `set` and `delete`. | Names kept. A call with both traits in scope is a compile error (E0034), so the compiler cannot pick the wrong one. No caller has both in scope today. Use `KeyValueStore::get(&store, ..)` or `StateStore::get(&store, ..)` where both are needed. |
| `D1WasmStorage`, `KVWasmStorage`, `R2WasmStorage` | Public inherent methods with the old trait names (`get_async`, `query_async`, `put_blob_async`, ...). Before T3, an inherent method shadowed the trait method of the same name. | Inherent methods kept. After T3 the names are different, so no call changed its target: every call that went to an inherent method still does (`StorageProvider`, `WasmCredentialStore`, `examples/cf-login-app`). Fold them into the traits: backlog section 11. |
| `D1WasmStorage` | It implements `KeyValueStore` and `foundation_auth`'s `CredentialStore`, which both have `get`, `set`, `delete`, `exists`, `list_keys`. | No change: the names were the same before (both `*_async`). The `CredentialStore` impls call the inherent methods or use the qualified path `CredentialStore::get(..)`. |
| `InMemoryVectorStore` and the other vector stores | `VectorIndex` (`FlatIndex`, `Hnsw`, `Ivf`) and `Bm25Index` have sync `insert`, `search`, `len`, `is_empty`. | No clash: no type implements both `VectorStore` and `VectorIndex`, and no vector store has an inherent method with these names. |
| `KvMemoryStore` | `SessionMemory` has inherent `get` / `set`; `MemoryAccess::hydrate` and `MemoryCoordinator::hydrate_async` exist. | No clash: those are other types. `MemoryCoordinator` keeps `record_async` / `hydrate_async` (inherent, not a storage trait; backlog section 10). |
| Every other implementing type | None: no inherent method with a new name (checked with `rg` over each type's `impl` blocks). | — |

The only change in method resolution is the intended one: a call to a trait
method under its new name. A sync inherent method with the same name cannot
take the call either, because every call site ends in `.await`.

**How.** A script renamed the method definitions inside each trait and
`impl Trait for` block. The compiler then reported each old call site
(`no method named get_async`), and a second script changed exactly that
line and column. Code that a normal build does not compile was checked
with its own build: wasm32 (`wasm-bindgen-storage`, `wasm-bindgen-session`),
`libsql`, the vector store features, `foundation_auth` `server-test` and
`server-native`, `foundation_cedar` `db`, `foundation_nativeapis`
`vfs-fjall`, and `examples/cf-login-app`. Doc comments and the crate docs
(`backends/*/docs`) use the new names.

**Checks (2026-10-27):**

| Check | Result |
|---|---|
| `cargo check --workspace --all-targets --profile dev` | pass, except `foundation_ai`, `answerme-agent` and `bin/platform`: `infrastructure_llama_bindings` needs the `tools/llama.cpp` submodule, which this checkout does not have |
| `cargo check -p foundation_ai --no-default-features --features testing --all-targets` | pass (with the `infrastructure_llama_cpp` dev-dependency taken out locally for the check only; backlog section 10) |
| `cargo check -p foundation_db --all-targets` (workspace features include `libsql`) | pass |
| `cargo check -p foundation_vectors --all-targets --features sql-store,turbopuffer,fjall-store,sqlite-vec` | pass |
| `cargo check -p foundation_auth --all-targets --features server-test,server-native` | pass |
| `cargo check -p foundation_cedar --all-targets --features db` | pass |
| `cargo check -p foundation_nativeapis --all-targets --features vfs-fjall` | pass |
| `ci.py run <crate> --step wasm` for `foundation_db`, `foundation_vectors`, `foundation_ai`, `foundation_auth` | pass |
| `cargo check --target wasm32-unknown-unknown -p foundation_auth --no-default-features --features wasm,wasm-bindgen-session` | pass (same warnings as `master`) |
| `cargo check --target wasm32-unknown-unknown` of `examples/cf-login-app` | pass |
| `ci.py run <crate> --step fmt` for every changed crate | pass |
| `ci.py run` build, clippy and test steps | see the PR: CI runs them; this checkout had less than 1 GB of free disk |

