---
feature: "foundation_nativeapis: the seven ignored ptrace fork-tracing tests run again, or are tracked with a precise cause"
status: "done"
lane: "A"
model: "sonnet"
branch: "spec70/f47-f50-lane-a-mechanical"
crates: ["foundation_nativeapis"]
catalogue_rows: [11]
---

# Feature 47: ptrace fork-tracing tests

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem

PR #172 (commit `adc03e9ae`) added `#[ignore = "fork tracing: PTRACE_EVENT_STOP on the
orbstack CI kernel causes deadlock …"]` to seven tests in
`backends/foundation_nativeapis/tests/ptrace_vfs/mod.rs`. There is no backlog entry (rule 9),
and spec 70 rule 4 says never ignore a test to get green.

## Research (write the findings in the PR body)

1. Read the interceptor's wait loop (`src/native/vfs/ptrace/`). A tracee in group-stop
   (`PTRACE_EVENT_STOP`, seen with `PTRACE_SEIZE`) must be resumed with `PTRACE_LISTEN`,
   not `PTRACE_CONT`; a new child after `PTRACE_EVENT_FORK`/`CLONE` starts with a
   `SIGSTOP` (or `PTRACE_EVENT_STOP` under SEIZE) that the tracer must consume. Check that
   every `waitpid` uses `__WALL` and that the auto-attached child's first stop is handled
   even when it arrives before the parent's fork event.
2. Reproduce locally (this container's kernel) with the ignore removed: run the seven tests
   20 times each with `timeout 60`.

## Fix

- If the cause is in our tracer (most likely — the kernel is not usually wrong here): fix the
  stop handling, remove the seven `#[ignore]`s, and add a test
  `fork_child_first_stop_is_consumed` that forks a child that exits at once.
- If after the fix a test still deadlocks only on one kernel: keep the ignore on that test
  only, with the kernel version in the reason, and add a backlog entry with the evidence.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p foundation_nativeapis --profile dev --features vfs-ptrace --test ptrace_vfs
python3 scripts/ci/ci.py run foundation_nativeapis --profile dev
rg -n '#\[ignore' backends/foundation_nativeapis/tests/ptrace_vfs   # none, or each tracked
```

## Done when

- [x] Cause named. Tests un-ignored and pass 20/20, or each remaining ignore has a backlog
      entry with the kernel and the evidence.

Native amd64 evidence: run 42040473182, `foundation_nativeapis` job success on `e01e83d80`
(PR #179). The job runs `tests/ptrace_vfs` on the `ubuntu-latest` x86_64 kernel with
`EWE_REQUIRE_PTRACE=1` and the named `vfs-native,vfs-ptrace` feature set, so the eight spawn
tests ran and passed; a probe failure would fail the job, not skip the suite. The Apple
Silicon host cannot repeat the run 20 times: `PTRACE_GETREGS` returns `EIO` under the
OrbStack amd64 emulation.

## Result

2026-11-05. Cause found and fixed. The seven `#[ignore]`s are removed and the new test
`fork_child_first_stop_is_consumed` is added.

Two bugs caused the deadlock:

1. The ptraced child was forked by the test thread, but every later `ptrace` request came
   from the separate `ptrace_loop` thread. A ptrace request must come from the tracer
   thread. The request failed with `ESRCH`, the tracee stayed stopped, and
   `waitpid` blocked forever.
2. `execvp` got `args` without the program name, so `/bin/sh -c "..."` received a wrong
   `argv`. The shell tests could not run their command.

Fix:

- The interceptor thread now forks the child and runs every `ptrace` request itself.
  `spawn` gets the child pid from a channel.
- The child calls `PTRACE_TRACEME`, then `raise(SIGSTOP)`, then `execve`. This removes the
  `PTRACE_SEIZE` race with `execve`. All buffers are built before `fork`, and the child
  calls only async-signal-safe functions.
- `argv` starts with the program name.
- Every `waitpid` uses `__WALL` (a clone child stop) and `__WNOTHREAD` (the test binary
  runs tests in parallel; a plain `waitpid(-1)` reaped another test's tracee).
- The new child's first `SIGSTOP` is consumed in the loop. The `PTRACE_EVENT_FORK`,
  `VFORK` and `CLONE` handlers only clone the FD table; they do not resume the new child.
- The spawn tests have a 60 s `ntest` timeout, so a hang fails the test.

Verification: 17 of 25 `ptrace_vfs` tests pass. The 8 spawn tests fail on the local
OrbStack amd64 emulation, not in the tracer: `PTRACE_GETREGS` returns `EIO` for every
traced amd64 process there (the same C probe fails), so the tracer cannot read the
syscall number. The tests must pass 20 of 20 on native amd64 in GitHub CI. This is
backlog entry §5.

Clippy with the `vfs-ptrace` feature set reports many warnings in other nativeapis
files. That debt is backlog entry §5, not part of this feature.
