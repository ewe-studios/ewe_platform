---
feature: "foundation_nostd: the two ignored condvar tests run"
status: "done"
lane: "A"
model: "sonnet"
branch: "spec70/f47-f50-lane-a-mechanical"
crates: ["foundation_nostd"]
catalogue_rows: [12]
---

# Feature 48: ignored condvar tests

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Problem

`backends/foundation_nostd/tests/primitives_condvar_tests.rs`:
`test_condvar_wait_timeout_long_with_notification` (~line 217) and
`test_condvar_concurrent_notify_one` (~line 620) are `#[ignore = "Requires threading
support"]` under `#[cfg(feature = "std")]`. With `std`, threads exist, so the reason is
wrong. Either the tests hang/fail (a real condvar bug) or they were ignored without cause.

## Work

1. Remove the ignores. Run each 20 times with `timeout 120` and `--features std`.
2. If they pass: done; keep them un-ignored.
3. If they fail or hang: find the cause in the condvar (`condvar_comp` / the nostd condvar
   used by `NotifyQueue`). A lost notify or a missed timeout here affects valtron. Fix it;
   add a smaller regression test that shows the bug; keep both tests un-ignored.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_nostd --profile dev
rg -n '#\[ignore' backends/foundation_nostd/tests   # no output, or each with a backlog entry
```

## Done when

- [ ] Both tests run in CI and pass 20/20. Any bug found is fixed with its own test.

## Result

2026-11-05. Both `#[ignore]`s are removed. The tests pass with the `std` feature, so the
ignore reason ("Requires threading support") was wrong. There was no condvar bug.

- `test_condvar_wait_timeout_long_with_notification`: 20 of 20 runs pass.
- `test_condvar_concurrent_notify_one`: 20 of 20 runs pass.
- `cargo test -p foundation_nostd --profile uat --features std`: all suites pass.
- `rg -n '#\[ignore' backends/foundation_nostd/tests`: no output.

The crate has 19 ignored doc examples in `src/primitives/noop.rs` and `src/macros.rs`.
They are a separate limitation and are backlog entry §5.
