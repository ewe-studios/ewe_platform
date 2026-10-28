---
feature: "foundation_nativeapis VFS storage: blob deltas list and chunk, shim uses D1/R2, Turso delta, lazy fjall scans, async document store, NFS readdir, streamed copies, SyncFs boundary"
status: "pending"
priority: "medium"
depends_on: ["17-db"]
branch: "fix/io-19-vfs-storage"
crates: ["foundation_nativeapis", "foundation_db"]
backlog: "docs/bug-backlog.md §13 (10 of 18 entries: storage side); issue ewe-studios/ewe_platform#142"
---

# Feature 19: VFS storage side

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** virtual file system (VFS); Network File System (NFS); input/output
(I/O); WebAssembly (wasm); JavaScript (JS); pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry. Feature 17 E4 (streamed blobs, ordered listing) makes E1 and E2 simpler; build
on it.

## T0. Check that every entry is still open (table in the PR body)

## E1. Blob-backed D1/R2 deltas cannot list

- **Where:** `src/native/vfs/blob_delta.rs` (`BlobDelta` behind `D1Delta`/`R2Delta`): `list_async`
  and `list_whiteouts_async` return nothing; `reset_async` is unsupported.
- **Fix:** use feature 17's `BlobStore::list_after(prefix, …)` (ordered) for R2, and D1's
  `list_keys` for D1. Implement `reset_async`.
- **Tests:** `blob_delta_lists_files_and_whiteouts`, `blob_delta_reset_clears_everything` (D1 and R2 mocks).

## E2. A blob-delta write rewrites the whole file (O(n²))

- **Fix:** chunked storage (finish or replace the unused `d1_delta::chunking` module); chunk size
  on config (default 64 KiB; document the choice). `write_at_async` rewrites only touched chunks.
- **Test:** `blob_delta_small_writes_touch_one_chunk` (write 1 MiB in 4 KiB pieces; assert bytes
  written to the store ≈ 1 MiB + overhead, not ~128 MiB).

## E3. The VFS shim never uses the D1/R2 deltas

- **Where:** `src/native/vfs/shim/mod.rs` logs "impls pending" and falls back to the memory delta.
- **Fix:** build them from the shim config: `DynFs::from_async(OverlayFileSystem::new(base, d1_delta))`.
- **Test:** `shim_config_d1_uses_d1_delta` (mock D1; a write lands in D1).

## E4. No delta store on the local Turso engine (#142)

- **Fix:** rebuild `TursoDelta` on the `turso` crate as an async-only store, with the
  `vfs-turso` feature and the shim backend. Reuse the delta contract tests.
- **Tests:** the delta contract suite runs on `TursoDelta`.

## E5. `FjallDocumentStore` scans buffer their rows

- **Where:** `scan`, `scan_all`, `scan_from` (`src/shared/vfs/fjall_document_store.rs`) collect a
  `Vec`; `scan_all` is unbounded.
- **Fix:** walk the fjall index range lazily as a stream. **Test:** `fjall_scan_yields_before_reading_all`.

## E6. `FjallDocumentStore` uses the sync VFS traits

- **Fix:** `V: AsyncVfsFileSystem` and await. **Test:** existing document-store tests on an async VFS.

## E7. `InodeFs` / `InodeDelta` ignore their path

- **Where:** `src/shared/vfs/fjall_fs/`: in-memory; `open(path)` and `open_with_config` ignore
  both; `FjallFs`/`FjallDelta` aliases suggest disk.
- **Fix:** rename constructors (`InodeFs::new()`), drop the aliases, add the async face like
  `MemoryFs`. **Test:** the async VFS contract suite on `InodeFs`.

## E8. NFS `readdir` ignores `start_after`; times are 0

- **Where:** `VfsNfs::readdir` (`src/native/vfs/nfs/mod.rs`).
- **Fix:** skip entries up to `start_after`; map `VfsMetadata` times.
- **Tests:** `nfs_readdir_pages_without_repeats` (a 1000-entry directory, page 100); `nfs_attrs_have_mtime`.

## E9. Whole-file reads in VFS defaults (rule 4)

- **Where:** `read_file`, `copy`, `copy_async` default methods.
- **Fix:** `copy`/`copy_async` stream in chunks (configured size). `read_file` stays (its contract
  is a whole file) but is documented as such and not used by `copy`.
- **Test:** `vfs_copy_streams_in_chunks` (a counting VFS: no single read larger than the chunk size).

## E10. `SyncFs` is not gated as a blocking boundary

- **Where:** `exec_async` (`src/shared/vfs/exec_async.rs`) uses `collect_one` when pending, with
  no `blocking_boundary!`.
- **Fix:** gate `SyncFs` and `exec_async` with `blocking_boundary!` so they do not compile on
  JS-hosted wasm. **Verify:** the wasm build of `foundation_nativeapis` (if it has one) passes and
  does not contain `SyncFs`.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_nativeapis --profile dev
cargo test -p foundation_nativeapis --profile dev --features vfs-native,vfs-d1,vfs-r2,vfs-fjall,vfs-nfs,vfs-turso
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted; #142 can be closed (say so in the PR).
- [ ] New tests fail on the base code where they can; 20-run flake check passes.
- [ ] CI green.
