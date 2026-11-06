---
feature: "The large-response relay failure is reproduced and either fixed or refuted"
status: "pending"
lane: "priority follow-up (backlog §9)"
model: "sonnet"
branch: "spec70/f130-large-response-reproduction"
crates: ["foundation_proxy"]
catalogue_rows: ["9"]
---

# Feature 130: the large-response relay failure

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as
they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

Glossary:

- **relay**: the proxy's two pumps that copy bytes between the client and the backend.
- **byte exact**: the body the client receives is identical to the body the server sent.

## Problem (recorded 2026-11-06, backlog §9)

One full `cargo test --profile uat -p foundation_proxy` run on macOS failed at
`tests/large_response_tests.rs:138` with `closed after N of 4194304 bytes`. The same test
passed alone and in two later full runs.

Two facts point at the feature 121 defect class:

- the run used a tree that also had an uncommitted `foundation_reactor` change;
- feature 121 then found and fixed a blocking body read and a dropped past-due timer, both
  of which close a relay early under load.

The entry was never retested after feature 121 landed.

## Design

This is a reproduction feature, not a design change.

1. Run the test twenty times alone, and twenty times inside the full suite with
   `--cpus=2` (a loaded machine is the reported condition), on the current branch.
2. If all runs pass, run the full suite thirty more times. Record the totals, close the
   feature and delete the backlog entry.
3. If any run fails, capture the trace for that run
   (`RUST_LOG=foundation_proxy=trace,foundation_netio=trace`), find the exact close, and
   fix it. The likely places are the response-body pump in `src/forward.rs` and the pipe
   close path in `foundation_core/src/valtron/pipe.rs`.
4. If the failure is a test defect (the client closing before the body is drained), fix
   the test and say so.

## Tests

No new test unless the reproduction finds a deterministic cause; then add one that fails
on the old code, in `backends/foundation_proxy/tests/large_response_tests.rs`.

## Verify

```bash
cargo fmt --all --check
cargo test --profile uat -p foundation_proxy
```

Then the loops named above, in the Linux CI image for the `--cpus=2` runs.

## Done when

- [ ] The failure is reproduced or refuted with recorded counts.
- [ ] A cause is fixed, or the entry is closed with the evidence.
- [ ] The backlog §9 entry is deleted.
- [ ] CI green.

## Result

2026-11-06. Plan only. Not started. Recorded while feature 123 ran the full proxy suite.
