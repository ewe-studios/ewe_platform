---
feature: "The targeted wake test does not time out under a full parallel suite"
status: "dropped"
lane: "priority follow-up (found under feature 139)"
model: "sonnet"
branch: "spec70/targeted-wake-flake"
crates: ["foundation_core"]
catalogue_rows: []
---

# Feature 143: the targeted wake test is bounded by work, not by wall time

**Dropped on 2026-11-06.** The owner closed new feature opening for spec 70. The item is
now GitHub issue [#187](https://github.com/ewe-studios/ewe_platform/issues/187), severity
`medium`. This file stays as the design record; its backlog entry is deleted.

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **wake storm**: many wakes in a short time, made to prove that a wake for one task does
  not wake another task.
- **flake**: a test that fails on one run and passes on another, with the same code.

## Problem (found under feature 139 on 2026-11-06, backlog §4)

One full `cargo test --profile uat -p foundation_core --features multi` run timed out
`a_wake_for_another_task_does_not_make_a_parked_task_re_check`
(`backends/foundation_core/tests/valtron/targeted_wake.rs`) at its 20 s bound. The test
passed alone and in the next full run. The defect is not reproduced yet.

Two conditions can make the test slow under a full run:

1. The storm thread runs for a fixed 600 ms and spins on `published`, which the executor
   thread also locks. On a machine with more runnable tests than cores, the executor can
   lose that lock again and again.
2. The full run leaves the detached worker of
   `hung_body_fails_at_its_timeout_instead_of_hanging` sleeping for an hour, and other
   tests hold threads, so the scheduler runs this test late.

A test that passes or fails by thread scheduling is not a proof. The same module also
prints `B was woken {woken} times`, so a run that times out gives no count.

## Design

1. Reproduce first. Run the full suite at least 10 times with `--test-threads` set to the
   core count plus two, and record the failure rate and the wake count. Do not change the
   test before a failure is reproduced or the cause is measured.
2. Make the storm end on work, not only on time: stop after the storm has woken B
   `MAX_WAKES` times, with the 600 ms bound kept as a safety stop. Keep both assertions
   (`woken >= 100`, `checked <= MAX_CHECKS`).
3. Report the counts before the assertion runs, not after. A timed-out test must show
   how far the storm got; move the `eprintln!` before the wait for A, or print inside the
   storm thread every 100 wakes.
4. If the measured cause is lock contention on `published`, replace the `Mutex<Option<_>>`
   with an `AtomicPtr`/`ArcSwap`-style hand-off that the executor never blocks on.
5. Do not raise the 20 s timeout as the fix. If the work needs a larger bound after the
   reproduction, record why in the Result.

## Tests

The existing test is the test. The change must keep it red on the old storm loop:
`MAX_CHECKS` stays 20, and the storm still wakes B at least 100 times.

## Verify

```bash
cargo fmt --all --check
cargo clippy --profile uat -p foundation_core --all-targets --no-deps --features multi -- -D warnings
for i in $(seq 1 10); do cargo test --profile uat -p foundation_core --features multi --test mod targeted_wake || break; done
cargo test --profile uat -p foundation_core --features multi
scripts/ci/linux.sh foundation_core
```

## Done when

- [ ] The failure is reproduced or the cause is measured and recorded.
- [ ] The test stops the storm on work, not only on wall time.
- [ ] 10 consecutive full-suite runs pass.
- [ ] The backlog §4 entry is deleted.
- [ ] CI green.
