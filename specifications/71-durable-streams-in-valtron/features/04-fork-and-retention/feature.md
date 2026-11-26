---
feature: "Fork, TTL and retention"
status: "planned"
priority: "medium"
depends_on: ["02-resumable-reader", "03-idempotent-producer"]
crates: ["foundation_core"]
---

# Feature 04: Fork, TTL and retention

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** time to live (TTL); JavaScript Object Notation (JSON); AeroSpace
and Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Add fork without copy, the fork TTL table, soft-delete and retention. All live in the core.
The valtron reader sees one stream across the fork boundary.

## 2. Requirements

1. **Fork creation.** `Stream-Forked-From` is a path on the same server. `Stream-Fork-Offset`
   defaults to the source tail. Beyond the tail returns `BadRequest`. Malformed returns
   `BadRequest`.
2. **Sub-offset.** `Stream-Fork-Sub-Offset` is a non-negative integer. Default `0`. It counts
   JSON messages or binary bytes. Overshoot returns `BadRequest`. It requires
   `Stream-Forked-From`, even when `0`.
3. **Content type.** A fork inherits the source type when omitted. A mismatch returns
   `ContentTypeMismatch`.
4. **Closed source.** A closed source can be forked. The fork starts open.
5. **Errors.** Missing source returns `NotFound`. Soft-deleted source returns `Conflict` at the
   adapter. Target in use returns `ConfigConflict`.
6. **Idempotent creation.** Matching config returns `CreateOutcome { created: false }`. The match
   includes `Forked-From`, `Fork-Offset` and `Fork-Sub-Offset`.
7. **Fresh writer state.** A fork does not inherit producer or `Stream-Seq` state.
8. **Read stitching.** Reads stitch source and fork data without offset translation. Source
   appends after the fork are invisible to the fork.
9. **Live modes.** Long-poll and SSE on a fork wait only for fork appends.
10. **Soft-delete.** `GET`, `HEAD`, `POST` and `DELETE` on a soft-deleted stream return `Gone`
    (`410`). `PUT` returns `ConfigConflict` (`409`). A fork from it returns `Conflict` (`409`).
    Forks keep reading. The last fork delete cascades cleanup. Cleanup may be asynchronous.
11. **TTL validation.** `Stream-TTL` is a non-negative decimal integer with no leading zeros,
    plus signs, decimals or scientific notation. `Stream-Expires-At` is RFC 3339. Both headers
    return `BadRequest`.
12. **Sliding TTL.** Each read and write resets the window. `HEAD` does not reset it. Live reads
    reset it when the server starts the request. A stream with active live readers does not
    expire.
13. **Expiry.** Expiry with forks soft-deletes the stream. Expiry without forks deletes it.
14. **Fork TTL table.** Use the table in section 2.1.
15. **Retention.** A store may drop old data. A read at a dropped offset returns `Trimmed`
    (`410`). Retention is optional.

### 2.1 Fork TTL table

| Source | Fork request | Fork gets |
|---|---|---|
| No expiry | No expiry | No expiry |
| No expiry | TTL | Own TTL |
| No expiry | Expires-At | Own Expires-At |
| TTL | No expiry | Inherit source TTL |
| TTL | TTL | Requested TTL |
| TTL | Expires-At | Requested Expires-At |
| Expires-At | No expiry | Inherit source Expires-At |
| Expires-At | TTL | Requested TTL |
| Expires-At | Expires-At | Requested Expires-At |

## 3. Design

`fork` is store bookkeeping plus a reader combinator. The store keeps the fork source, fork
offset, sub-offset, reference count and soft-delete state. The core reader resolves positions
through the chain. The valtron reader only consumes the core pages.

```rust
pub struct ForkSpec {
    pub source: StreamName,
    pub at: Option<StreamOffset>,   // None = source tail
    pub sub_offset: u32,            // 0 = absent
}
```

## 4. Tasks

- [ ] **T1 — fork creation.** Head, specific offset, zero offset, default offset, closed source
      and empty source. Tests: `fork_at_tail`, `fork_at_offset`, `closed_source_fork_is_open`.
- [x] **T2 — sub-offset.** JSON and binary counts, `0` equals absent and overshoot. The adapter
      enforces the missing `Forked-From` case. Tests: `json_sub_offset_counts_messages`,
      `binary_sub_offset_counts_bytes`, `sub_offset_requires_source`.
- [x] **T3 — read stitching.** Read across the boundary. Test:
      `fork_reads_span_the_boundary_transparently`.
- [x] **T4 — idempotent fork.** Matching and mismatching config. Test:
      `fork_create_is_idempotent`.
- [x] **T5 — soft-delete and cascade.** Source stays readable through the fork, direct access is
      `Gone`, re-create is `ConfigConflict`, last fork delete cascades. Tests:
      `soft_deleted_source_is_gone`, `deleting_last_fork_cascades_to_source`.
- [ ] **T6 — TTL.** Sliding reset, `HEAD` no reset, both headers, fork table and expiry with
      forks. Tests: `ttl_slides_on_read_and_write`, `head_does_not_reset_ttl`,
      `fork_ttl_follows_table`, `expired_source_with_fork_is_soft_deleted`.
- [ ] **T7 — retention.** A dropped offset returns `Trimmed`. Test:
      `retention_trims_by_refusing_not_gapping`.
- [ ] **T8 — valtron boundary.** The valtron reader sees one stream across the fork. Test:
      `valtron_reader_spans_fork_boundary`.
- [ ] **T9 — oracles.** Metamorphic, model and property oracles. Tests:
      `fork_read_is_source_prefix_plus_tail`, `ttl_model_matches`, `fork_offsets_are_unique`.

## 5. Success criteria

- [ ] `cargo test -p foundation_core` green; fork and TTL tests live in `tests/durable/`.
- [ ] Fork reads are byte-exact across the boundary.
- [ ] Soft-delete returns `Gone`, and cascade cleanup works.
- [ ] The valtron reader sees the fork as one stream.
