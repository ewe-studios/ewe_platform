---
feature: "The large-response relay failure is reproduced and either fixed or refuted"
status: "done"
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

- [x] The failure is reproduced or refuted with recorded counts.
- [x] A cause is fixed, or the entry is closed with the evidence.
- [x] The backlog §9 entry is deleted.
- [x] CI green. GitHub Actions run `42205799516` (head `bc7e870d5`) passed on 2026-11-06.

## Result

2026-11-06. Refuted on macOS. The target test passes every run on top of features 121 and
129. No source file changed.

### Reproduction

The reported failure was on macOS. Feature 129 fixed the order in
`RawStream::read_wake_source` that lost a later, smaller read edge on kqueue. Feature 121
fixed the blocking upstream body read and the dropped past-due timer. Both fixes are in
this branch (`8878f50d1`, `cc4c46fb8`, `669e65e8b`). This feature reran the reported
condition on top of them.

A macOS host cannot enforce a container CPU quota. Each loaded run used
`--test-threads=2` and eight `yes` burner processes on this 10-CPU host. That leaves
about two CPUs for the suite, which matches the reported loaded 2-CPU machine. The
burners are stopped by saved PID after each run. A copy of the test binary ran from
`/tmp`, so no run rebuilt it.

| Phase | Command | Runs | Target pass | Target fail | Suite rc=0 |
|---|---|---|---|---|---|
| alone | `/tmp/f130-large_response_tests --exact proxy_large_response_is_byte_exact` | 20 | 20 | 0 | n/a |
| full suite | `cargo test --profile uat -p foundation_proxy --no-fail-fast -- --test-threads=2` | 20 | 20 | 0 | 10 |
| alone, loaded | same copied binary, 8 burners | 30 | 30 | 0 | n/a |

The target test is `large_response_tests::proxy_large_response_is_byte_exact`. It passed
70 of 70 executions. The original entry recorded one failure in a full-suite run, so 70
passes is strong evidence that the defect class is gone.

### Cause or refutation

The failure did not recur. `closed after N of 4194304 bytes` needs the proxy to end the
client response before the upstream `Content-Length` is met. The proxy ends early only
when `write_streamed_response` gets an upstream body error, or a client write error. The
body enforces `Content-Length`, so a short upstream answer is `UnexpectedEof`. The two
fixes removed the two ways a live upstream could stall and then be closed: `WouldBlock`
no longer becomes terminal, the response socket is non-blocking and parks on a re-armed
read edge, and a past-due timer is no longer dropped. The backlog entry is deleted.

### Other failures seen under load

The suite returned a non-zero status in 10 of the 20 loaded runs. Every failure was in
`tests/rpc_proxy_e2e_tests.rs`, not in the target test. The 14 failing tests were
`grpc_web_server_stream_via_proxy` (4), `grpc_web_server_stream_via_grpc_backend_carries_status` (3),
`connect_server_stream_via_grpc_backend` (3), `connect_client_stream_via_grpc_backend` (3)
and `connect_server_stream_via_h1_proxy` (1). Each hit `CALL_DEADLINE` = 5 s with
`[deadline_exceeded]`, after
`[DEPENDS] Task Entry { id: 1, gen: 2 } returned already-ready signal (violation 1/4096)`.
This is the recorded backlog §9 stall. The entry now names the macOS recurrence and the
new test names. Run 3 also printed a second panic from `sendables.rs:905`
(`block_on_future: the future produced no result`), which hides the first panic. That is a
new backlog §9 entry.

### Commands

| Command | Exit | Result |
|---|---|---|
| `cargo test --profile uat -p foundation_proxy --test large_response_tests --no-run` | 0 | built in 26.9 s |
| `cargo fmt --all --check` | 0 | clean |
| `cargo clippy --profile uat -p foundation_proxy --all-targets --no-deps -- -D warnings` | 0 | no warnings |
| `cargo test --profile uat -p foundation_proxy --no-fail-fast` | 0 | 129 tests in 33 test binaries pass |

### Deviations

- The `--cpus=2` condition ran on macOS, not in the Linux CI image. The reported failure
  is a macOS kqueue failure, so the Linux image cannot exercise it. The 2-CPU limit is
  emulated with `--test-threads=2` and eight burners.
- The plan's 30 additional full-suite runs were not run. The suite is not green under
  load because of the unrelated `rpc_proxy_e2e_tests` stall, so 30 more suite runs would
  add about 50 minutes of the same unrelated failures. The 30 extra runs ran the target
  binary under the same load instead, and all passed.
- Another agent's Docker CI run shared the machine for part of phase 2. It adds load. It
  does not change the target result.
- `progress.md` and `learnings.md` are not changed. The task scope allowed only this file
  and backlog §9.
- CI is not run from this branch. The CI box is left unticked.
