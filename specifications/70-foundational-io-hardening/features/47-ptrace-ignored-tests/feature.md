---
feature: "foundation_nativeapis: the seven ignored ptrace fork-tracing tests run again, or are tracked with a precise cause"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
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

- [ ] Cause named. Tests un-ignored and pass 20/20, or each remaining ignore has a backlog
      entry with the kernel and the evidence.
