---
feature: "Storage: DurableStream and the VFS"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core", "foundation_nativeapis", "foundation_db"]
---

# Feature 05: Storage: DurableStream and the VFS

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** deterministic simulation testing (DST); Lightning Memory-Mapped
Database (LMDB, a name); Durable Object (DO); Structured Query Language (SQL); Amazon Simple
Storage Service (S3); JavaScript Object Notation (JSON); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100. R2 and D1 are names of
Cloudflare products, not abbreviations.

## 1. Goal

Put one small storage trait under `DurableStream`: `DurableStreamVfs`. Make the VFS swappable.
Implement memory and file VFS first. Add DO SQLite and D1 later. Feature 09 adds the S3/R2 VFS.
The protocol core never changes when the VFS changes.

**User story.** As a platform engineer, I want to swap the stream storage, so that the same
protocol core runs on memory, a file, R2/S3, DO SQLite or D1.

**BDD scenarios.**

- Given a memory VFS, when the business API creates and appends, then a reader sees the bytes.
- Given a file VFS, when the business API restarts, then the data is still there.
- Given two VFS implementations, when the same scenario runs, then the results match.

## 2. Requirements

1. **VFS trait.** `DurableStreamVfs` is object-safe. Its methods return boxed futures. It owns
   bytes, metadata and the append notifier.
2. **`DurableStream`.** `DurableStream` is a struct. It wraps `Arc<dyn DurableStreamVfs>`. It
   implements `DurableStreamStore` by delegation.
3. **Memory VFS.** In `foundation_core`. It uses a `BTreeMap` and a notifier. It serves DST and
   tests.
4. **File VFS.** In `foundation_nativeapis`. It uses a log file and LMDB metadata. Blocking file
   I/O stays at `blocking_boundary!`.
5. **Protocol state.** `DurableStream` owns producer state, `Stream-Seq`, TTL access time, fork
   edges, reference counts and soft-delete. It stores that state through VFS metadata records.
6. **Later VFS.** DO SQLite and D1 implement the VFS. They may also implement the store directly
   if the VFS does not fit. The S3/R2 VFS is feature 09.
7. **Paged reads.** A read returns a bounded page. No VFS materialises a whole stream.
8. **Async and runtime.** The VFS methods are async. The VFS has no transport, HTTP, valtron or
   global runtime.
9. **Injection.** `Ctx` holds `Arc<DurableStream>`. The VFS is boxed inside the struct. Swapping
   storage changes one construction line.
10. **DST.** Every VFS passes the same seeded scenario.
11. **`DurableStream` driver.** `DurableStream` implements `DurableDriver` with `Store = Self`.
    The business API takes the stream directly.
12. **Home crate.** Each VFS lives in the crate that owns its environment. This lets it use the
    full native capability of that environment.
13. **Tests in the home crate.** Each VFS crate has conformance tests and coverage tests. The
    shared scenarios come from the `foundation_core` testkit. Coverage exercises every VFS
    method and native path.

## 3. Design

```rust
pub struct DurableStream {
    vfs: Arc<dyn DurableStreamVfs>,
}

impl DurableStream {
    pub fn new(vfs: Arc<dyn DurableStreamVfs>) -> Self {
        Self { vfs }
    }
}

impl DurableStreamStore for DurableStream { /* delegate to self.vfs */ }
impl DurableDriver for DurableStream { type Store = Self; /* ... */ }
```

The VFS contract is in `requirements.md` section 3.7.

| VFS | Crate |
|---|---|
| Memory | `foundation_core` |
| File | `foundation_nativeapis` |
| R2 / S3 | `foundation_nativeapis` (feature 09) |
| DO SQLite | spec 83 crate |
| D1 | spec 83 crate |

## 4. Tasks

- [ ] **T1 — VFS trait.** `DurableStreamVfs`, `VfsMeta`, `VfsOffset`, `VfsEntry`, `VfsError` and
      `VfsFuture`. Test: `vfs_is_object_safe` (compile).
- [ ] **T2 — `DurableStream` struct.** Delegates every store method to the VFS. Test:
      `durable_stream_delegates_to_vfs`.
- [ ] **T3 — memory VFS.** In `foundation_core`. `BTreeMap` and a notifier. Tests: shared
      conformance scenarios, `memory_vfs_notifies_on_append`, bounded read coverage.
- [ ] **T4 — file VFS.** In `foundation_nativeapis`. A log file and LMDB metadata. Tests: shared
      conformance scenarios, `file_vfs_persists_across_restart`, LMDB and group-commit coverage.
- [ ] **T5 — protocol state.** Producer, `Stream-Seq`, TTL, fork and soft-delete state over VFS
      metadata. Tests: `producer_state_survives_restart`, `stream_seq_state_survives_restart`.
- [ ] **T6 — DO SQLite and D1 VFS.** Atomic producer state. Tests:
      `do_sqlite_vfs_passes_dst_scenario`, `d1_vfs_passes_dst_scenario`.
- [ ] **T7 — oracles.** Crash and recovery, model, differential and resource oracles. Tests:
      `restart_preserves_stream`, `backends_match_model`, `read_is_bounded`.
- [ ] **T8 — no valtron or runtime.** The VFS and `DurableStream` build with valtron and sockets
      disabled.
- [ ] **T9 — VFS testkit.** Export the shared VFS conformance scenarios from `foundation_core`
      behind a test-only feature. Test: `every_vfs_runs_the_same_scenarios`.
- [ ] **T10 — coverage.** Add coverage tests for each VFS native path. Record the covered
      methods and the uncovered branches.

## 5. Success criteria

- [ ] `cargo test -p foundation_core`, `cargo test -p foundation_nativeapis` and
      `cargo test -p foundation_db` green.
- [ ] The same DST scenario passes on memory, file, DO SQLite and D1. The S3/R2 VFS passes in
      feature 09.
- [ ] No VFS materialises a whole stream; reads are paged.
- [ ] `DurableStream` wraps a boxed VFS; `Ctx` holds `Arc<DurableStream>`.
- [ ] Swapping the VFS changes one construction line.
- [ ] The VFS has no valtron, socket or global runtime dependency.
- [ ] Each VFS lives in the crate that owns its environment and uses its full native capability.
- [ ] Every VFS crate runs the shared conformance suite and its own coverage tests.
