---
feature: "The ptrace VFS suite runs and proves itself on a native amd64 kernel in GitHub CI"
status: "done"
lane: "after feature 67 merges; runs with feature 82"
model: "sonnet"
branch: "spec70/f83-ptrace-native-amd64-ci"
crates: ["foundation_nativeapis"]
catalogue_rows: []  # from docs/bug-backlog.md §16; not a §8 catalogue row
---

# Feature 83: ptrace spawn tests on native amd64 CI

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (measured 2026-11-05)

Feature 47 added 25 tests to `backends/foundation_nativeapis/tests/ptrace_vfs/mod.rs`.
8 of them spawn a child under `ptrace`:

`test_spawn_true_exits_zero`, `test_spawn_echo_with_args`, `test_spawn_false_exits_nonzero`,
`test_spawn_nonexistent_fails`, `test_spawn_pipe_chain`, `test_spawn_can_write_to_real_fs`,
`fork_child_first_stop_is_consumed`, `test_spawn_multiple_children`.

On the development Mac they fail. The Linux container is OrbStack on an amd64 virtual
machine (VM), and `PTRACE_GETREGS` returns `EIO` for every traced amd64 process. A small C
program shows the same result. The cause is the emulation, not the tracer. The suite passes
17 of 25 tests there.

Three gaps remain even on the GitHub CI runner:

1. **No named run.** The suite rides in the last composite `runs` entry of
   `ci/packages.toml`. A failure appears as "the full VFS feature set failed", not as
   "ptrace failed".
2. **An empty suite passes.** `ptrace_vfs/mod.rs` starts with
   `#![cfg(all(target_os = "linux", feature = "vfs-ptrace-nix", feature = "vfs-native"))]`.
   If the feature set is wrong, cargo builds an empty test binary and the job is green.
3. **The tests check exit codes, not interception.** Only
   `test_spawn_can_write_to_real_fs` looks at an effect, and that write passes through.
   No test proves that the traced child sees the virtual `MountTable`.

## Platform fact (read before planning macOS work)

The ptrace backend is Linux-only by construction:

- the suite is gated on `target_os = "linux"`;
- `src/native/vfs/ptrace/nix_backend.rs` uses `libc::PTRACE_*`, `user_regs_struct` and the
  Linux x86_64 system-call numbers in `syscall_dispatch.rs`.

There is no macOS ptrace backend in this workspace, and macOS does not provide a Linux
`ptrace` system call. A GitHub macOS runner therefore cannot run this suite, on Intel (amd64)
or Apple Silicon. GitHub's amd64 macOS label is `macos-13`; it runs Darwin, not Linux.

Native amd64 coverage comes from the Linux job instead. The `package` job in
`.github/workflows/check.yaml` runs on `ubuntu-latest`, which is a native x86_64 kernel, with
the linux/amd64 CI image as a container. That container shares the runner's kernel, so it is
not emulated. This is the kernel that the `EIO` failure needs. If the owner wants macOS
coverage for this code, it needs a separate macOS backend: a new feature, not this one.

## Design

1. **A probe target.** Add `backends/foundation_nativeapis/tests/ptrace_probe.rs`:
   - Gate: `#![cfg(all(target_os = "linux", feature = "vfs-ptrace-nix"))]`.
   - `fork`. The child calls `PTRACE_TRACEME`, then `raise(SIGSTOP)`. The parent waits and
     calls `PTRACE_GETREGS`.
   - Print one line: `ptrace probe: available` or
     `ptrace probe: unavailable (<errno name>: <number>)`. Map the errno to the cause:
     `EIO` = emulated kernel (OrbStack/qemu); `EPERM` = seccomp or `Yama`
     `ptrace_scope`; `ESRCH` = the tracee is gone.
   - With `EWE_REQUIRE_PTRACE=1`, `assert!` that the probe succeeded, so CI fails with the
     reason instead of skipping.
   - Expose the decision as a pure function (`fn ptrace_requirement(available: Result<(),
     i32>, required: bool) -> ProbeOutcome`) so a test on any host can assert both branches.
   This mirrors `foundation_iogate/tests/io_uring_probe.rs` (feature 78 E1).
2. **A named run.** In `ci/packages.toml`, add the entry
   `"vfs-native,vfs-ptrace"` to `runs`, with a comment that it is the ptrace spawn suite.
   Set `env = { EWE_REQUIRE_IO_URING = "1", EWE_REQUIRE_PTRACE = "1" }`. CI must not lose
   the suite silently.
3. **A probe step in the workflow.** In `.github/workflows/check.yaml`, add a `ptrace probe`
   step in both the `package` and `package-artifact` jobs, with
   `if: matrix.package == 'foundation_nativeapis'`:
   `cargo test --profile "$CI_PROFILE" -p foundation_nativeapis
   --features vfs-native,vfs-ptrace --test ptrace_probe -- --nocapture`.
   It runs before the test step, so a failed job states the kernel result first. Use
   `docker exec ci ...` in `package-artifact`.
4. **Stronger spawn tests.** Add these tests to `ptrace_vfs/mod.rs`:
   - `the_traced_child_reads_the_virtual_mount`: mount a `MemoryFs` at `/virtual` with a
     file; spawn `/bin/sh -c "cat /virtual/<name> > <real tmp file>"`; assert the real file
     holds the virtual file's content. This proves the syscall loop is engaged, not a plain
     fork and exec.
   - `the_traced_child_writes_to_the_virtual_mount`: the child redirects output into the
     virtual mount; the parent reads the content back through the `MountTable`.
   - `the_traced_child_stdout_arrives_whole`: a child writes a known string of at least
     64 KiB to a real file through the tracer; assert its length and checksum. This catches
     file descriptor or pipe corruption in the syscall loop.
   If a test fails because the spawn path does not consult the mount table, fix the
   dispatch and record the gap in `docs/bug-backlog.md`. This feature is not done with a
   failing test that is ignored.
5. **Record the skip rule.** Add `ptrace_missing(what)` next to `io_uring_missing` in
   `tests/common/mod.rs`, for a suite that must skip on a non-native kernel. The spawn tests
   keep failing instead of skipping, because `EWE_REQUIRE_PTRACE=1` is set in CI.

## Tests

- `ptrace_probe` itself, on a native amd64 Linux kernel and on this Mac (the emulated kernel
  prints `EIO`). Record both outputs.
- The pure decision function test (runs on any host, in the probe binary).
- The 3 new spawn tests above, plus the 8 existing ones, all `ok` on native amd64.

## Verify

Local (this Mac cannot run the spawn tests; expect the emulated kernel result):

```bash
cargo test --profile uat -p foundation_nativeapis --features vfs-native,vfs-ptrace \
  --test ptrace_probe -- --nocapture
cargo test --profile uat -p foundation_nativeapis --features vfs-native,vfs-ptrace \
  --test ptrace_vfs -- --nocapture   # 17/25 here; that is the recorded limitation
```

On GitHub (native amd64), in the `foundation_nativeapis` job:

- the `ptrace probe` step prints `ptrace probe: available`;
- the `vfs-native,vfs-ptrace` run reports every test in `ptrace_vfs` as `ok`, with no
  `ignored` and no empty binary.

Attach the job URL and the probe line to the report. A green local emulated run does not
prove this feature.

## Done when

- [x] CI has a named ptrace run, a probe step that fails when ptrace is missing, and
      `EWE_REQUIRE_PTRACE=1`.
- [x] The 3 new interception tests pass on the native amd64 CI job.
- [x] The CI log shows `ptrace probe: available` and 28 `ptrace_vfs` tests `ok`
      (job 127187028997, `vfs-native,vfs-ptrace` run: 28 passed, 0 failed, 0 ignored).
- [x] The backlog entry "The ptrace spawn tests cannot run under amd64 emulation" is
      rewritten to say where the tests now run, or deleted when the entry no longer applies.
- [x] `learnings.md` and `progress.md` record the platform fact (Linux-only suite; native
      amd64 comes from `ubuntu-latest`, not macOS).
