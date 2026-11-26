---
feature: "S3 and R2 VFS"
status: "planned"
priority: "high"
depends_on: ["01-durable-stream-types", "05-storage-backends"]
crates: ["foundation_nativeapis", "foundation_core"]
---

# Feature 09: S3 and R2 VFS

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Amazon Simple Storage Service (S3); Cloudflare R2 (R2, a product
name); compare and swap (CAS); Entity Tag (ETag); write-ahead log (WAL); Hypertext Transfer
Protocol (HTTP); JavaScript Object Notation (JSON); time to live (TTL); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Add a high-performance VFS for S3 and R2 in `foundation_nativeapis`. It writes locally first.
It batches locally. It pushes one object per batch. It uses conditional writes for multi-writer
safety. It reads with range GETs. The protocol core and `Ctx` do not change.

**User story.** As an operator, I want durable streams on S3 or R2, so that storage is cheap,
durable and safe for several writers.

**BDD scenarios.**

- Given a local spool and a bucket, when a writer appends, then the append is durable locally.
- Given a full batch, when the batcher flushes, then it sends one object PUT.
- Given two writers, when both update the head, then one CAS wins and the other retries.
- Given a process crash, when the writer restarts, then the spool replay loses no data.
- Given S3 and R2, when the same scenario runs, then the results match.

## 2. Requirements

1. **Provider trait.** One `ObjectStore` trait covers S3 and R2. It has conditional PUT, range
   GET, HEAD, LIST and DELETE. The provider is selected by configuration.
2. **Object layout.** Immutable segment objects and one head manifest per stream. The key scheme
   is private. Example: `<root>/<stream>/HEAD` and `<root>/<stream>/seg/<gen>-<seq>.log`.
3. **Head manifest.** The head has a version, the segment list, the tail length, producer state,
   `Stream-Seq`, closure and expiry. It is the CAS target.
4. **Immediate local write.** An append writes to a local spool file and fsyncs before it returns
   `Accepted`. A crash loses nothing that was acknowledged.
5. **Local batching.** The batcher groups appends by bytes and time. It seals one segment per
   batch. It sends one object PUT per batch. Large batches use multipart.
6. **CAS.** Create uses `If-None-Match: *`. Update uses `If-Match: <etag>`. A stale ETag returns
   `412 Precondition Failed`. The writer re-reads the head and retries.
7. **Multi-writer safety.** Only one writer wins a head CAS. A loser keeps its sealed segment and
   retries the head update. Producer and `Stream-Seq` reject a duplicate or a regression.
8. **Sealed acknowledgement.** `Accepted` means local fsync. `Sealed` means the segment and head
   are durable in the object store.
9. **Read path.** Read the head, then range GET the segments. Cache the head and hot segments
   locally. Stitch a page across segment boundaries.
10. **Crash recovery.** Replay the spool on start. Use an idempotent batch id. Delete orphan
    segments that no head references.
11. **Compaction.** Merge small segments into one object. Trim old segments by retention. Never
    copy a whole stream.
12. **Provider differences.** Detect conditional write support, multipart limits, checksum
    support and consistency. Use a capability record. If a provider lacks CAS, use a lease
    object and document the weaker guarantee.
13. **Performance.** Batch bytes, linger time, parallel PUT count, target segment size and cache
    bytes are configuration with documented defaults. The write path does not block the caller
    on an S3 round trip.
14. **Security.** TLS, server-side encryption and least-privilege credentials. Credentials come
    from the injected environment, never a global.
15. **Tests.** Each VFS runs the shared conformance scenarios and its own coverage tests. S3 and
    R2 run the same scenarios. Crash, CAS race, orphan and compaction tests are required.
16. **No runtime leak.** The VFS has no HTTP server, valtron or global runtime.

## 3. Design

```rust
pub trait ObjectStore: Send + Sync {
    fn put_if_none_match(&self, key: &str, bytes: &[u8]) -> ObjectFuture<'_, Version>;
    fn put_if_match(&self, key: &str, bytes: &[u8], version: &Version)
        -> ObjectFuture<'_, Version>;
    fn get_range(&self, key: &str, start: u64, len: usize) -> ObjectFuture<'_, Vec<u8>>;
    fn head(&self, key: &str) -> ObjectFuture<'_, ObjectMeta>;
    fn list(&self, prefix: &str) -> ObjectFuture<'_, Vec<ObjectMeta>>;
    fn delete(&self, key: &str) -> ObjectFuture<'_, ()>;
}

pub struct S3Vfs { store: Arc<dyn ObjectStore>, spool: Spool, batcher: Batcher }
pub struct R2Vfs { store: Arc<dyn ObjectStore>, spool: Spool, batcher: Batcher }
```

The write path:

1. Append to the spool and fsync.
2. The batcher seals a segment.
3. PUT the segment with `If-None-Match: *`.
4. CAS the head with `If-Match`.
5. On `412`, re-read the head, re-base the segment list and retry.
6. Return `Sealed` after the head CAS.

The read path:

1. Read the head.
2. Map the offset to a segment and a range.
3. Range GET each segment in the page.
4. Cache the head and hot segments.
5. Return a bounded page.

## 4. Tasks

- [ ] **T1 — `ObjectStore` trait.** S3 and R2 clients. Capability record. Tests:
      `s3_and_r2_share_the_contract`.
- [ ] **T2 — key layout and head manifest.** Segment keys, head key, version and JSON encoding.
      Test: `head_manifest_round_trips`.
- [ ] **T3 — local spool.** Append, fsync, replay and idempotent batch id. Tests:
      `spool_survives_crash`, `spool_replay_is_idempotent`.
- [ ] **T4 — batcher.** Byte and time limits. One PUT per batch. Multipart for large batches.
      Tests: `one_put_per_batch`, `large_batch_uses_multipart`.
- [ ] **T5 — CAS head.** `If-None-Match` create and `If-Match` update. Retry on `412`. Tests:
      `two_writers_one_cas_wins`, `cas_retry_preserves_both_segments`.
- [ ] **T6 — read path.** Head cache, range GET and cross-segment stitch. Tests:
      `read_spans_segments`, `head_cache_is_refreshed`.
- [ ] **T7 — crash recovery.** Replay and orphan cleanup. Tests:
      `restart_after_segment_put`, `orphan_segment_is_removed`.
- [ ] **T8 — compaction and retention.** Merge, trim and GC. Tests:
      `compaction_keeps_bytes_exact`, `retention_drops_old_segments`.
- [ ] **T9 — provider differences.** Conditional write probe, multipart limits and checksums.
      Tests: `provider_capabilities_are_recorded`, `weak_cas_fallback_is_documented`.
- [ ] **T10 — conformance and coverage.** Run the shared VFS scenarios and the coverage tests.
      Tests: `s3_vfs_conformance`, `r2_vfs_conformance`, `s3_vfs_coverage`, `r2_vfs_coverage`.
- [ ] **T11 — differential.** Run S3 and R2 against the same scenarios with a local emulator.
      Test: `s3_and_r2_agree`.
- [ ] **T12 — performance.** Measure batch latency, PUT count and throughput. Record the
      defaults and the result.

## 5. Success criteria

- [ ] `cargo test -p foundation_nativeapis` green.
- [ ] The S3 and R2 VFS pass the shared conformance and coverage suites.
- [ ] A crash after a segment PUT loses no acknowledged data.
- [ ] Two writers never lose a batch; one CAS wins and one retries.
- [ ] One batch sends one object PUT (or one multipart upload).
- [ ] Reads use range GETs and never materialise a whole stream.
- [ ] The VFS has no HTTP server, valtron or global runtime.
