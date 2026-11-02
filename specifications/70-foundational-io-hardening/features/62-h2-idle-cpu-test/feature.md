---
feature: "foundation_http: idle_h2_connection_uses_no_cpu is reliable and measures the right thing"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
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

- [ ] The idle CPU user is named in the PR body; any busy wake fixed with its own test.
- [ ] 20/20 runs pass. §3 backlog entry deleted.
