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
Storage Service (S3); AeroSpace and Defence Industries Association of Europe (ASD), the owner of
the STE standard ASD-STE100. R2 and D1 are names of Cloudflare products, not abbreviations.

## 1. Goal

Five `DurableStreamStore` implementations share one reader and writer. Each uses an existing seam
and satisfies the same DST scenario.

## 2. Backends

| Backend | Seam | Offset scheme | Notes |
|---|---|---|---|
| In-memory | `BTreeMap<StreamName, Vec<Chunk>>` + a notifier | a monotonic per-stream integer | DST and tests; seeded and replayable |
| File + LMDB | a native log file + LMDB metadata | a monotonic per-stream integer in LMDB | the native `foundation_log` tail; group commit as config |
| R2 / S3 segments | `foundation_db::BlobStore` + the `foundation_log` key layout | a padded last-sequence, epoch-prefixed (see [tiered-store.md section 6.2](../../../research/edge-data/tiered-store.md#62-key-layout-lunoras-archive-keys)) | the sealed tier; immutable, `If-None-Match: *` fencing |
| DO SQLite | `DoSqlStore` (`QueryStore`/`KeyValueStore`) | a per-stream SQL sequence | spec 65 T0 |
| D1 | `D1WasmStorage` | a per-stream SQL sequence | spec 65 T1 |

## 3. Requirements

1. **One trait, five impls.** Each backend implements every method of `DurableStreamStore`. The
   offset scheme is private to the backend and never leaks into the reader.
2. **The store is injected, never global** (rule 11). A host's entry point builds the bundle with
   the real backend; tests swap the in-memory backend.
3. **Acknowledgment.** `Accepted` is durable at the tail; `Sealed` is durable in the sealed
   segment. The file and R2/S3 backends implement `Sealed` with group commit (seal on bytes,
   records or age, all config — rule 8).
4. **Atomic producer state.** DO SQLite and LMDB commit the producer state with the append in one
   transaction. The file backend documents the false-`Conflict` recovery instead (feature 03).
5. **Stream bodies, never buffer them** (rule 6). A read returns a bounded page; the R2/S3 backend
   streams a segment with range `GET`s, never a whole object into memory.
6. **The DST scenario.** Every backend passes the same seeded scenario: create, append N chunks
   with a producer, read them back byte-exactly, trim the first half, and prove the trimmed offset
   returns `Trimmed`.

## 4. Tasks

- [ ] **T1 — in-memory backend.** `foundation_core`. Passes the DST scenario and the feature 02
      and 03 tests.
- [ ] **T2 — file + LMDB backend.** Native only (`blocking_boundary!` for the blocking file I/O).
      Group commit and `Sealed` are config fields. Passes the DST scenario.
- [ ] **T3 — R2/S3 backend.** Uses `foundation_db::BlobStore` and the `foundation_log` key layout.
      Immutable segments, `If-None-Match: *` fencing, range `GET` reads. Passes the DST scenario.
- [ ] **T4 — DO SQLite and D1 backends.** Use `DoSqlStore` and `D1WasmStorage`. A doc comment maps
      each to the spec 65 T0/T1 tiers. Passes the DST scenario where the seam is available in CI.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` and `cargo test -p foundation_db` green.
- [ ] The same DST scenario passes on every backend that CI can run.
- [ ] No backend materialises a whole stream; reads are paged.
- [ ] `cargo check --workspace --all-targets --profile dev` green.
