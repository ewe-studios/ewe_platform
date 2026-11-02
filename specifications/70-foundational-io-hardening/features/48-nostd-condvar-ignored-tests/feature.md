---
feature: "foundation_nostd: the two ignored condvar tests run"
status: "pending"
lane: "A"
model: "sonnet"
branch: "fix/io-41-50-lane-a"
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
