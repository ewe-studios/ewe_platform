---
feature: "Remaining Sync Twins"
description: "Finish the async-first work: the vector store and the virtual file system still have sync traits with input/output-backed implementations; the storage trait methods keep their _async suffix; record the sync boundaries that stay."
status: "pending"
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
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
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

| Task | What | Done when |
|---|---|---|
| T1 | **Vectors:** make every vector store implement `AsyncVectorStore` only (`TurboPufferVectorStore`, `SqliteVecVectorStore`, `FjallVectorStore`, `InMemoryVectorStore`); move the `shed` tool to the async trait; delete the sync `VectorStore`; rename `AsyncVectorStore` to `VectorStore` (and its methods, if task T3 decides so). | No sync vector trait remains; `ci.py run foundation_vectors` and `foundation_ai` pass. |
| T2 | **VFS:** owner decision needed (section 4). | As decided. |
| T3 | **Method suffixes:** owner decision needed: drop `_async` from the storage trait methods (`get_async` → `get`, …) or keep them. A rename needs a clash check per type, because some stores have inherent methods with the plain names. | As decided; if "drop", a mechanical PR like #134. |
| T4 | **Records:** `progress.md`, `learnings.md`, the backlog. | Updated in the same PRs. |

## 4. Open questions for the owner

1. **VFS scope.** The VFS has a full sync API and a full async API. Options:
   - (a) **Async-first core, sync only as a bridge.** Every I/O implementation (`TursoDelta`,
     `SyncLibsqlDelta`, `VfsClient`, `NativeFs`, `DirectoryDelta`, Fjall) implements the async
     traits only. The sync traits stay only for consumers that must be sync (for example a SQLite
     VFS, which SQLite calls synchronously), through `SyncFs<A>` at that boundary. Large change.
   - (b) **Delete the sync VFS traits.** Every consumer moves to async. Largest change; removes
     the dual API.
   - (c) **Leave the VFS as it is** and record it as a known sync area.
   
   **Recommendation: (a).** It follows the rule (sync only at boundaries) and keeps a sync face
   where a sync consumer needs one.
2. **Method suffixes (task T3).** **Recommendation: drop them**, in one mechanical PR after task
   T1, with a clash check per type.
