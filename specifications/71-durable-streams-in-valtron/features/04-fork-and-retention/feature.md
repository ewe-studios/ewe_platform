---
feature: "Fork and retention"
status: "planned"
priority: "medium"
depends_on: ["02-resumable-reader", "03-idempotent-producer"]
crates: ["foundation_core"]
---

# Feature 04: Fork and retention

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

## 1. Goal

A fork branches a stream without copying its prefix, and retention trims history by refusing a
dropped offset instead of silently skipping. Both reuse the reader and writer from features 02
and 03.

## 2. Requirements

1. **`fork_at(offset)`** builds a reader that chains the source's prefix with the fork's own tail.
   Offsets at or before the fork resolve against the source without copying; offsets after resolve
   against the fork. The reader sees one stream; the client never knows it is a fork.
2. **Independence.** A fork has its own name, TTL, closure and deletion. It can outlive its source.
3. **Soft-delete.** Deleting a source with live forks removes it from normal use (reads and
   appends on the source name get `NotFound`) and blocks re-creation (`Conflict`), but fork reads
   keep working. When the last fork is deleted, the source is garbage-collected, cascading up the
   chain. Cleanup may be asynchronous.
4. **Reference count.** The store keeps a fork reference count and the cascade. This is store
   bookkeeping, not reader state.
5. **Retention.** The store may drop data older than a retention window. A read at a dropped
   offset returns `StoreError::Trimmed`. The retention window is configuration, never a constant
   (rule 8).
6. **No data movement on fork.** Forking never copies prefix bytes; it adds a reference.

## 3. Design

`fork_at` is a combinator, not a new storage layout. It composes two readers:

```rust
pub struct ForkedReader<Store> {
    prefix: DurableStreamReader<Store>,  // source, up to the fork offset
    tail:   DurableStreamReader<Store>,  // fork's own appends, from the fork offset
    at:     StreamOffset,
}

impl<Store: DurableStreamStore> StreamIterator for ForkedReader<Store> {
    type D = Chunk;
    type P = ReaderProgress;
    // Serve prefix chunks until `at`, then switch to tail chunks, transparently.
}
```

The store side implements `fork` and the soft-delete/refcount in `DurableStreamStore::fork` and
`delete`. The reader only chains.

## 4. Tasks

- [ ] **T1 — `fork_at` reader.** A fork reads the prefix then its own tail as one stream. Test:
      `fork_reads_span_the_boundary_transparently`.
- [ ] **T2 — soft-delete.** Deleting a source with a fork returns `NotFound` on the source and
      keeps the fork readable; re-creating the source returns `Conflict`. Test:
      `soft_deleted_source_stays_readable_through_fork`.
- [ ] **T3 — cascade.** Deleting the last fork garbage-collects the source. Test:
      `deleting_last_fork_cascades_to_source`.
- [ ] **T4 — retention.** A read at a dropped offset returns `Trimmed`. Test:
      `retention_trims_by_refusing_not_gapping`.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; fork and retention tests live in `tests/`.
- [ ] Fork reads are byte-exact across the boundary (reuse the feature 02 test helper).
- [ ] `cargo check --target wasm32-unknown-unknown -p foundation_core` green.
