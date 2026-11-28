---
feature: "Storage: DurableStream and the VFS"
status: "done"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core", "foundation_nativeapis", "foundation_db"]
---

# Feature 05: Storage: DurableStream and the VFS

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** deterministic simulation testing (DST); Lightning Memory-Mapped
Durable Object (DO); Structured Query Language (SQL); Amazon Simple
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
4. **File VFS.** In `foundation_nativeapis`. It uses an append-only log file and one metadata file
   per stream, replaced by write-then-rename after the data log is flushed. The owner chose the atomic
   metadata file on 2026-11-27: the workspace has no LMDB dependency. Blocking file
   I/O stays at `blocking_boundary!`.
5. **Protocol state.** `DurableStream` owns producer state, `Stream-Seq`, TTL access time, fork
   edges, reference counts and soft-delete. It stores that state through VFS metadata records.
6. **DO SQLite, D1 and the R2 tier.** DO SQLite and D1 implement the VFS. They may also implement
   the store directly if the VFS does not fit. The S3/R2 VFS is feature 09.
   - The metadata record belongs to the VFS. The protocol rules stay in the core store.
   - A Durable Object keeps `stream_meta` and `pending_data` in SQLite. A batcher sends immutable
     blocks to R2 and marks the rows delivered. A vacuum removes them after the block is durable.
   - The acknowledge point is configuration: after the SQLite commit (default) or after the R2
     PUT. Block size, batch age, vacuum threshold and the retry bound are configuration.
   - See task T6 for the tables and the read, crash and recovery paths.
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

| VFS | Crate and path |
|---|---|
| Memory | `foundation_core/src/durable/shared/memory.rs` |
| File | `foundation_nativeapis/src/native/vfs/durable_file.rs` |
| R2 / S3 | `foundation_nativeapis/src/native/vfs/{r2,s3}.rs` (feature 09) |
| DO SQLite | spec 83 `cf/` module |
| D1 | `foundation_db/src/wasm/bindgen/cf/d1.rs` |

Shared VFS pieces live in `foundation_nativeapis/src/shared/vfs/durable/`. Platform code
follows the repository layout.

## 4. Tasks

- [x] **T1 — VFS trait.** `DurableStreamVfs`, `VfsMeta`, `VfsOffset`, `VfsEntry`, `VfsError` and
      `VfsFuture`. Test: `vfs_is_object_safe` (compile).
- [x] **T2 — `DurableStream` struct.** Delegates every store method to the VFS. Test:
      `durable_stream_delegates_to_vfs`.
- [x] **T3 — memory VFS.** In `foundation_core`. `BTreeMap` and a notifier. Tests: shared
      conformance scenarios, `memory_vfs_notifies_on_append`, bounded read coverage.
- [x] **T4 — file VFS.** In `foundation_nativeapis`. An append-only log file and an atomically
      replaced metadata file. Tests: shared conformance scenarios, `file_vfs_persists_across_restart`,
      metadata and group-commit coverage. The 8 tests in `tests/durable_file_vfs/` pass.
- [x] **T5 — protocol state.** Producer, `Stream-Seq`, TTL, fork and soft-delete state over VFS
      metadata. Tests: `producer_state_survives_restart`, `stream_seq_state_survives_restart`.
- [x] **T6 — DO SQLite VFS and D1 VFS.** Atomic producer state. Tests:
      `do_sqlite_vfs_passes_dst_scenario`, `d1_vfs_passes_dst_scenario`.

      **Status (2026-11-27): done.** The generic engine
      (`foundation_core::durable::shared::sql`), the block format, the batcher,
      the vacuum, the D1 executor and the DO SQLite executor landed. The engine
      passes the shared VFS kit against a real SQLite engine (libsql) in
      `foundation_core/tests/durable/sql.rs`. Issue #1052 is closed.

      The DO executor is `foundation_db::DoSqlExecutor` in
      `src/wasm/wasm_storage/do_sql_executor.rs`. It runs `exec` with bound
      parameters for a statement and a query, and `transactionSync` for one
      atomic transaction. The Workers Rust library (`workers-rs`) version 0.8
      wraps `exec` but not `transactionSync`, so
      `src/wasm/bindgen/cf/do_sql.rs` binds the missing call. The executor takes
      the storage handle (`DoSqlExecutor::new`) or the Durable Object state that
      the runtime passes to the constructor (`DoSqlExecutor::from_state`).

      Changed-row counts: the runtime's `rowsWritten` counter includes index
      rows, so an insert into an indexed table reports two. The executor
      measures the rise of the SQLite `total_changes()` counter instead. That
      value matches the D1 `meta.changes` count, and a query leaves it
      unchanged.

      The real-service check is
      `backends/foundation_db/integration/do_sql/`. It is a standalone worker.
      `wrangler dev --local` runs it in workerd:

      ```
      cd backends/foundation_db/integration/do_sql
      worker-build --release
      wrangler dev --local --port 8791
      curl --fail http://127.0.0.1:8791/run
      curl --fail http://127.0.0.1:8791/run-d1
      curl --fail http://127.0.0.1:8791/rollback
      ```

      **Wasm check command.** A wasm check of this crate must disable the
      default features:

      ```
      cargo check -p foundation_db --target wasm32-unknown-unknown \
        --no-default-features --features do-sql
      ```

      The default set includes `turso`, which turns on `multi`. `multi` needs
      threads, and `foundation_db` keeps the state store future `!Send` on wasm
      by design. The same gap breaks a plain default-feature wasm check without
      `do-sql`. Issue #1056 records it. The CI `wasm-runs` list uses
      `--no-default-features` for the same reason and now includes `do-sql`.

      Measured 2026-11-27: `/run` and `/run-d1` return `{"ok":true,...}` with
      all nine scenarios: `create`, `append`, `read_byte_exact`,
      `bounded_pages`, `fork_prefix`, `fork_append`, `soft_delete_cascade`,
      `ttl_slide` and `version_conflict`. The DO route uses a SQLite-backed
      Durable Object (`new_sqlite_classes`); the D1 route uses the local D1
      binding. `/rollback` returns `{"failed":true,"ok":true,"remaining":0}`: a
      failing second statement rolls the first insert back. No route is a stub.

      **Metadata is the VFS.** `DurableStreamVfs` stores the metadata record: content type,
      length, closed, expiry, writer state (producers and `Stream-Seq`), fork, last access,
      trim-from and version. The core store owns the semantics: producer checks, JSON encoding,
      fork resolution and TTL decisions. A DO SQLite VFS therefore replaces the metadata bytes,
      not the rules.

      **DO design: SQLite hot path plus R2 cold tier.**

      | Table | Holds |
      |---|---|
      | `stream_meta` | one row per stream: the metadata record above, plus `delivered_seq` |
      | `pending_data` | appended chunks: stream, sequence, bytes, state (`pending` or `delivered`) |

      - An append writes the chunk row and the metadata row in one SQLite transaction. The
        server acknowledges after the commit, so the hot copy is durable.
      - A batcher groups pending chunks per stream into one block up to `max_block_bytes`
        (configuration), then PUTs the block to R2 under a deterministic key derived from the
        stream and the first and last sequence. The block is immutable.
      - After the PUT succeeds, the rows move to `delivered` and `stream_meta.delivered_seq`
        advances. A vacuum removes delivered rows once `delivered_seq` is durable in R2.
      - A read serves from SQLite while the rows are present. For a block already vacuumed, the
        read fetches the block from R2 by range and decodes the chunk.
      - A crash replays pending rows. The deterministic block key plus the starting sequence make
        the PUT idempotent.
      - `durability` is configuration: `hot` (ack after the SQLite commit) or `object` (ack after
        the R2 PUT). `hot` is the default for a Durable Object.
      - Block size, batch age, vacuum threshold and the retry bound are configuration, never
        constants.
      - This tiering builds on the R2 layout in feature 09. The SQLite VFS is the owner of both
        tables; feature 09 owns the object format.
- [x] **T7 — oracles.** Crash and recovery, model, differential and resource oracles. Tests:
      `restart_preserves_stream`, `backends_match_model`, `read_is_bounded`. The differential
      oracle runs the shared kit on the memory and file VFS and compares the observations. The
      restart oracle writes, drops, reopens the same root and reads. The resource oracle reads
      with the configured page limits and proves each page stays bounded.
- [x] **T8 — no valtron or runtime.** The VFS and `DurableStream` build with valtron and sockets
      disabled.
- [x] **T9 — VFS testkit.** Export the shared VFS conformance scenarios from `foundation_core`
      behind the `vfs-testkit` feature. Test: `every_vfs_runs_the_same_scenarios`.
- [x] **T10 — coverage.** Add coverage tests for each VFS native path. Record the covered
      methods and the uncovered branches. Both VFS crates call every `DurableStreamVfs` method.

## 5. Success criteria

- [x] `cargo test -p foundation_core`, `cargo test -p foundation_nativeapis` and
      `cargo test -p foundation_db` green.
      Recorded 2026-11-28: `cargo test --profile uat -p foundation_core` passed 1,046 tests,
      with 0 failed and 0 ignored. With `--features test-clock,multi` it passed 1,134 tests,
      with 0 failed and 0 ignored. The full `foundation_nativeapis` run
      (with `vfs-object-http`) and `cargo test -p foundation_db` were recorded as green
      earlier.
- [x] The same DST scenario passes on memory, file, DO SQLite and D1. The S3/R2 VFS passes in
      feature 09. Evidence: `vfs_testkit::run_all` runs the nine deterministic scenarios on
      memory (`tests/durable/vfs_conformance.rs`), file (`tests/durable_file_vfs/oracle.rs`),
      libsql (`sql_vfs_passes_conformance`) and DO SQLite and D1 (`integration/do_sql` routes
      `/run` and `/run-d1`).
- [x] No VFS materialises a whole stream; reads are paged. Tests:
      `paged_reads_are_bounded_and_reassemble` (`tests/durable/property.rs`),
      `sql_vfs_range_read_is_bounded` (`tests/durable/sql.rs`) and `read_is_bounded`
      (`tests/durable_file_vfs/oracle.rs`).
- [x] `DurableStream` wraps a boxed VFS; `Ctx` holds `Arc<DurableStream>`. Evidence:
      `DurableStream { vfs: Arc<dyn DurableStreamVfs>, .. }`
      (`foundation_core/src/durable/shared/mod.rs`); hosts store it with
      `ctx.get_or_insert(DurableStream::...)` and read it with `ctx.get::<DurableStream>()`
      (`foundation_http/src/native/durable.rs`).
- [x] Swapping the VFS changes one construction line. Evidence:
      `DurableStream::new(Arc::new(MemoryVfs::new()), &ctx)` and
      `DurableStream::new(Arc::new(FileVfs::new(root)?), &ctx)` share the same
      `DurableStreams<DurableStream>` type; only the VFS argument changes.
- [x] The VFS has no valtron, socket or global runtime dependency. Evidence:
      `given_the_durable_core_when_read_then_it_names_no_valtron_or_http_type`
      (`tests/durable/architecture.rs`), and a `grep` of the SQL, file and shared VFS sources
      finds no `valtron`, `tokio` or socket use.
- [x] Each VFS lives in the crate that owns its environment and uses its full native capability.
      Evidence: memory in `foundation_core/src/durable/shared/memory.rs`; file in
      `foundation_nativeapis/src/native/vfs/durable_file.rs`; DO and D1 executors in
      `foundation_db/src/wasm/wasm_storage/{do_sql_executor,d1_executor}.rs`.
- [x] Every VFS crate runs the shared conformance suite and its own coverage tests. Evidence:
      `every_vfs_runs_the_same_scenarios` (`tests/durable/vfs_conformance.rs`) plus the same
      kit in the file, S3, R2, SQL and DO/D1 tests. Coverage tests:
      `memory_vfs_covers_every_method`, `sql_vfs_covers_every_method`,
      `file_vfs_covers_every_method`, `s3_vfs_coverage` and `r2_vfs_coverage`. The DO and D1
      executor paths run through `integration/do_sql`.

## 6. Open items

None.
