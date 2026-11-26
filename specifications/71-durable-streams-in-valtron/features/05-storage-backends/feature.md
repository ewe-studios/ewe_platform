---
feature: "Storage backends"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types"]
crates: ["foundation_core", "foundation_db"]
---

# Feature 05: Storage backends

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** deterministic simulation testing (DST); Lightning Memory-Mapped
Database (LMDB, a name); Durable Object (DO); Structured Query Language (SQL); Amazon Simple
Storage Service (S3); JavaScript Object Notation (JSON); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100. R2 and D1 are names of
Cloudflare products, not abbreviations.

## 1. Goal

Five `DurableStreamStore` implementations share one core. Each uses an existing seam. Each
stores the full protocol state. The core and the backends have no valtron dependency.

## 2. Backends

| Backend | Seam | Offset scheme | Notes |
|---|---|---|---|
| In-memory | `BTreeMap` + a notifier | a monotonic per-stream integer | DST and tests |
| File + LMDB | a native log file + LMDB metadata | a monotonic per-stream integer in LMDB | the native `foundation_log` tail |
| R2 / S3 segments | `foundation_db::BlobStore` + the `foundation_log` key layout | a padded last-sequence, epoch-prefixed | the sealed tier |
| DO SQLite | `DoSqlStore` (`QueryStore`/`KeyValueStore`) | a per-stream SQL sequence | spec 83 T0 |
| D1 | `D1WasmStorage` | a per-stream SQL sequence | spec 83 T1 |

## 3. Requirements

1. **One trait, five impls.** Each backend implements every store method.
2. **Full state.** Each backend stores producer state, `Stream-Seq`, last-access time for TTL,
   fork edges, reference counts and soft-delete state.
3. **Injected.** The host builds `Arc<DurableStream>`; tests use the in-memory backend or a mock.
4. **Async.** The store methods are native `async fn`. A blocking file backend uses
   `blocking_boundary!`.
5. **Atomicity.** DO SQLite and LMDB commit producer state with the append in one transaction.
   The file backend documents the false-`SequenceGap` recovery.
6. **Paged reads.** A read returns a bounded page. No backend materialises a whole stream.
7. **Offsets.** The offset scheme is private. The store builds every offset. It is strictly
   increasing and unique within a stream. It never contains `,`, `&`, `=`, `?` or `/`.
8. **Compatibility.** The standalone port is `4437`. The store does not depend on a transport.
9. **DST.** Every backend passes the same seeded scenario: create, append N chunks with a
   producer, read back byte-exactly, trim the first half, and prove the trimmed offset returns
   `Trimmed`.
10. **No valtron.** The backends compile and test with valtron disabled.

## 4. Tasks

- [ ] **T1 — in-memory backend.** `foundation_core`. Passes the DST scenario and the feature 02
      and 03 tests. Test: `in_memory_passes_dst_scenario`.
- [ ] **T2 — file + LMDB backend.** Native only. Group commit and `Sealed` are config fields.
      Test: `file_lmdb_passes_dst_scenario`.
- [ ] **T3 — R2/S3 backend.** Uses `foundation_db::BlobStore` and the `foundation_log` key
      layout. Immutable segments, `If-None-Match: *` fencing, range `GET` reads. Test:
      `r2_s3_passes_dst_scenario`.
- [ ] **T4 — DO SQLite and D1 backends.** Use `DoSqlStore` and `D1WasmStorage`. Atomic producer
      state. Test: `do_sqlite_passes_dst_scenario`, `d1_passes_dst_scenario`.
- [ ] **T5 — fork and TTL state.** Every backend stores fork edges, refcounts, soft-delete and
      last-access time. Test: `backends_store_fork_and_ttl_state`.
- [ ] **T6 — no valtron.** A test crate builds the backends with valtron disabled.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` and `cargo test -p foundation_db` green.
- [ ] The same DST scenario passes on every backend CI can run.
- [ ] No backend materialises a whole stream; reads are paged.
- [ ] The backends have no valtron dependency.
- [ ] `cargo check --workspace --all-targets --profile dev` green.
