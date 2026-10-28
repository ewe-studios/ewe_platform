---
feature: "foundation_nativeapis process side: LD_PRELOAD shim, FUSE in CI, all VFS features in CI, non-blocking file calls, async IPC with request ids, async searchers, ptrace warnings"
status: "pending"
priority: "medium"
depends_on: ["09-limits-as-config"]
branch: "fix/io-20-vfs-process-ipc"
crates: ["foundation_nativeapis", "foundation_iogate"]
backlog: "docs/bug-backlog.md §13 (8 of 18 entries: process, IPC, search, build)"
---

# Feature 20: VFS process side

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** virtual file system (VFS); Filesystem in Userspace (FUSE);
inter-process communication (IPC); continuous integration (CI); input/output (I/O); pull request
(PR); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Check that every entry is still open (table in the PR body)

## E1. The `LD_PRELOAD` shim does not compile

- **Where:** `src/native/vfs/shim/mod.rs` with `vfs-preload`: `open` is defined with fixed
  arguments (libc's `open` is variadic); `DeltaBackend::Sqlite` calls `initialize_pool` without
  an import.
- **Fix:** export `open`/`openat` from a small C file (built with `cc`) that forwards to Rust, or
  use `c_variadic` only if the toolchain allows it on stable (it does not on stable today; prefer
  the C file). Add the import.
- **Verify:** `cargo build -p foundation_nativeapis --features vfs-preload`; a test that preloads
  the shim into `cat` on a memory delta file and reads its content.

## E2. FUSE adapter cannot build in CI

- **Fix:** install `libfuse-dev` (or `libfuse3-dev`, per `fuser`'s needs) in the CI image; add a
  `vfs-fuse` build run. **Verify:** the CI plan lists it; it builds.

## E3. CI does not build most VFS features

- **Fix:** add a run to `ci/packages.toml` with
  `vfs-native,vfs-search-fff,vfs-sqlite,vfs-ipc,vfs-fjall,vfs-nfs,vfs-ptrace,vfs-d1,vfs-r2,vfs-arrow`.
- **Verify:** `python3 scripts/ci/ci.py run foundation_nativeapis` includes it and passes.

## E4. `NativeFs` and `DirectoryDelta` block for each system call

- **Fix:** on Linux, the crate's `uring` layer for file reads and writes; elsewhere a bounded
  offload pool (threads and queue size on config) that the async methods await. Small metadata
  calls may stay inline if measured under 50 µs; write the measurement.
- **Test:** `native_fs_large_read_does_not_hold_worker` (a 64 MiB read on a 2-worker pool; a ticker
  keeps ticking).

## E5. The IPC bus is sync; its timeouts are constants

- **Where:** `src/native/vfs/ipc_bus.rs`: `ClientTransport::request` waits on the calling thread;
  `run_daemon_loop` waits per dispatch through `exec_async`; join timeout 30 s, receive poll 1 s
  (rule 8).
- **Fix:** async receive on the bus (readiness, feature 02/05 style); both timeouts on a transport
  config with documented defaults.
- **Test:** `ipc_request_awaits_without_blocking`; `ipc_timeouts_come_from_config`.

## E6. The VFS IPC protocol has no request ids

- **Fix:** a request id in `VfsRequest` and `VfsResponse`; match by id; drop the "count released
  handles" skip logic and the lock held across send and receive. `ReadAt` sizes become `u64`
  (check every user).
- **Tests:** `ipc_out_of_order_responses_match_by_id`; `ipc_concurrent_requests` (16 at once).

## E7. CLI and fff searchers block; limits are constants

- **Where:** `src/shared/vfs/search.rs`: `CliSearcher` waits for `rg`/`grep` on the calling
  thread; `FffSearcher::new` waits up to 30 s; grep limits 50 per file, 200 per page, 5 s budget.
- **Fix:** async process wait (`foundation_shell` or the crate's process helper; read stdout as a
  stream); `SearchConfig` with documented defaults.
- **Tests:** `cli_search_streams_results`; `search_limits_come_from_config`.

## E8. ptrace warnings

- **Fix:** remove unused imports, unused constants and the unreachable statement after `execvp`
  (`src/native/vfs/ptrace/`). **Verify:** `cargo check -p foundation_nativeapis --features vfs-ptrace 2>&1 | rg warning` is empty.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_nativeapis --profile dev
cargo build -p foundation_nativeapis --profile dev --features vfs-preload
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] New tests fail on the base code where they can; 20-run flake check passes.
- [ ] CI green with the new feature runs.
