---
feature: "foundation_http: idle_h2_connection_uses_no_cpu is reliable and measures the right thing"
status: "done"
lane: "A"
model: "sonnet"
branch: "spec70/f77-f62-f78-readiness-ci"
crates: ["foundation_http"]
catalogue_rows: [34]
---

# Feature 62: idle HTTP/2 CPU test

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem (found during feature 41)

`tests/h2_parking_tests.rs` `idle_h2_connection_uses_no_cpu` fails about half the runs on the
dev machine, on master too: 54 ms of process CPU in 1 s against a 50 ms cap. The test measures
the whole process, so any thread's work counts.

## Work

1. Find what uses CPU while the connection is idle. Sample per-thread CPU time
   (`/proc/self/task/<tid>/stat`, or `clock_gettime` per thread id) during the idle second and
   name the threads that use it (valtron workers, the reactor, the timer thread, the test's own
   client).
2. If a component wakes in a loop while idle (a periodic poll, a timer that re-arms), that is a
   bug: fix it (it belongs to spec 70's goal: idle means parked) and add a test for it.
3. Then make the test measure what it claims: the CPU of the server's threads (worker and
   reactor threads), not the whole process. Keep a strict cap, and give the cap's reason in a
   comment. Do not just raise the cap.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
for i in $(seq 1 20); do timeout 300 cargo test -p foundation_http --profile dev --test h2_parking_tests idle_h2_connection_uses_no_cpu -- --exact || { echo "FAILED on run $i"; exit 1; }; done
python3 scripts/ci/ci.py run foundation_http --profile dev
```

## Done when

- [x] The idle CPU user is named in the PR body; any busy wake fixed with its own test.
- [x] 20/20 runs pass. §3 backlog entry deleted.

## Result (2026-11-05)

The test now measures only the server's threads. It takes a thread snapshot before
`initialize_pool`, and the server set is the threads that appear after it (the pool workers and
the background workers) plus the shared `reactor-drain` thread by name. The measure works on
Linux (`/proc/self/task/<tid>/schedstat`) and macOS (`proc_pidinfo` with
`PROC_PIDLISTTHREADS` and `PROC_PIDTHREADINFO`). A parallel test cannot disturb it, because its
threads are not in the set.

Measured on the macOS host, over one idle second:

| Thread | CPU |
|---|---|
| two pool workers | 0 us each |
| `bg-worker-0` to `bg-worker-3` | 0 us each |
| `reactor-drain` | 218 to 233 us |

The reactor drain thread wakes on its 100 ms poll timeout (`ReactorConfig::drain_poll_timeout`,
`DEFAULT_DRAIN_POLL_TIMEOUT`); 10 wake-ups per second cost about 0.2 ms. The workers are parked.
There is no busy loop to fix. The old 54 ms process-wide reading counted the idle worker
re-checks that feature 75 and feature 77 removed, and threads of other tests.

The cap is 8 ms for the server threads, down from 50 ms for the whole process. The reason is
in the test comment: 0.6 to 1.2 ms per idle valtron worker on Linux, four workers plus the
reactor, and headroom for a loaded CI host. A 10 ms poll would exceed it.

Checks: `idle_h2_connection_uses_no_cpu` passed 20 of 20 runs with `--exact`, and the full
`h2_parking_tests` target passed 20 of 20 runs. The default-feature target (which compiles the
multi-gated helpers out) passes. Clippy reports no warning in this test file in either feature
set.

Backlog: the §3 entry for this test is deleted. A new §3 entry records the pre-existing
foundation_http clippy debt.
