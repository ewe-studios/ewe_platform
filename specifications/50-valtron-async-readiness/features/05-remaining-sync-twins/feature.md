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
  completed: 1
  uncompleted: 3
  total: 4
  completion_percentage: 25
---

# Remaining Sync Twins

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** input/output (I/O); virtual file system (VFS); inter-process
communication (IPC); Hypertext Transfer Protocol version 1.1 (HTTP/1.1); operating system (OS);
pull request (PR); application programming interface (API); Structured Query Language (SQL);
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
| T2 | **VFS:** async-first core (decision 1). Move `TursoDelta`, `SyncLibsqlDelta`, `VfsClient`, `NativeFs`, `DirectoryDelta` and the Fjall stores to the async traits only; keep `SyncFs<A>` as the only sync face for I/O stores; move `OverlayFileSystem`, `ObservableFs`, `search` and `dynfs` to the async traits, with sync wrappers only where a sync consumer needs them. | No I/O-backed type implements a sync VFS trait directly; `ci.py run foundation_nativeapis` and its feature runs pass. | Open |
| T3 | **Method suffixes:** drop `_async` from the storage trait methods (decision 2). Check each type for inherent methods with the plain names first. | No storage trait method ends in `_async`; workspace, wasm and example builds pass. | Open |
| T4 | **Records:** `progress.md`, `learnings.md`, the backlog. | Updated in the same PRs. | Updated for T1 |

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
