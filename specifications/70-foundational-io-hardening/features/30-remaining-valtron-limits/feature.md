---
feature: "Remaining valtron limits become configuration (rule 8)"
status: "pending"
priority: "medium"
depends_on: "09-limits-as-config"
branch: "fix/io-30-remaining-valtron-limits"
crates: ["foundation_core"]
backlog: "docs/bug-backlog.md §4 Remaining hard-coded valtron limits (rule 8)"
---

# Feature 30: Remaining valtron limits become configuration

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

`CLAUDE.md` rule 8: size caps, timeouts, buffer and batch sizes are fields on a config with a
builder method and a documented default that names its source. Never a `const` in the code
path.

## 1. Goal

Feature 09 moved the yield, back-off, `ThreadedIterFuture` and pool limits onto configs. These
valtron limits stayed as `const` in code paths. Each becomes a field on the right config, with
the same default value and a doc comment that names its source.

## 2. Items

| # | Item | Where | Do |
|---|---|---|---|
| R1 | `READINESS_POLL_QUANTUM = 1 ms` | `executors/local.rs:880`, used at `:3052` and `:3071` in `block_on`'s `CanProgress` arm | A config field (the busy-spin guard when nothing is runnable). Default 1 ms, doc: why. |
| R2 | `DEFAULT_OP_READ_TIME = 100 ms` | `executors/constants.rs:10`, used at `executors/multi/mod.rs:1541` | A pool-config field. Default 100 ms. |
| R3 | `DEFAULT_NOTIFY_QUEUE_WAIT_TIMEOUT` | `executors/constants.rs:34,38`, used at `executors/future_task.rs:856` | A config field for `NotifyQueue::wait_for_item`'s timeout. Default unchanged. |
| R4 | `DEFAULT_NOTIFY_QUEUE_MAX_SPINS` | `executors/constants.rs:46,49`, used at `executors/local.rs:327,339` | A config field for the CondVar wait-retry bound. Default unchanged. |
| R5 | `DEFAULT_READINESS_WAIT = 10 ms` | `executors/constants.rs:56`, used at `executors/local.rs:913` (`Sleepers::with_max_readiness_wait`) | A config field for the readiness-signal wait. Default 10 ms. |
| R6 | `DEFAULT_DELIVERY_CAPACITY = 8` | `executors/constants.rs:64`, used at `executors/builders/non_sendable.rs:145,233,258` and `executors/builders/sendable.rs:145` | A config field for a `sequenced` task's delivery queue. Default 8. |
| R7 | `JS_WAIT_CHECK_INTERVAL = 4 ms` | `executors/wasm/mod.rs:47` (and the two re-exports), used at `executors/local.rs:2886,2989` | A config field for the wasm yield check interval. Default 4 ms. |
| R8 | `DEFAULT_PARK_DURATION = 1 ms`, `DEFAULT_WAIT_CYCLE = 300 µs` | `executors/constants.rs:21,25`, used as `wait_cycle.unwrap_or(..)` in `sendables.rs` and `non_sendables.rs` | Review. They are `unwrap_or` defaults for optional parameters, so a `Some` overrides them. Decide: keep as documented function-param defaults, or move them onto a config. Write the decision. |

## 3. Tasks

- [ ] T1. Branch from the base.
- [ ] T2. R1–R7: one config field each, default value unchanged, doc comment names its source.
      One commit (or one commit per item if the diff is large).
- [ ] T3. R8: the review and its written decision.
- [ ] T4. Tests in `foundation_core/tests/valtron/`, registered in `mod.rs`. For each new
      field: a test that builds the config with a non-default value and asserts the code path
      uses it (mirror `pool_config_overrides_yield_and_backoff`). If a path cannot be observed
      cheaply, test the default value and say why.
- [ ] T5. Delete the §4 backlog entry. Update `progress.md`, `learnings.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_core --profile dev
cargo check --workspace --all-targets --profile dev
rg -n 'const .*(Duration|_SIZE|_CAPACITY|_SPINS)' backends/foundation_core/src/valtron/executors backends/foundation_core/src/io/buffer_pool.rs
#   every remaining hit is a DEFAULT_… used only by a config Default impl, or is the R8 decision.
```

## 5. Done when

- [ ] All §4 commands pass; timing tests passed 20 runs in a row.
- [ ] Every item R1–R8 is done or has a written reason.
- [ ] No default value changed (no behaviour change).
- [ ] Each new field's doc comment names its source.
- [ ] CI green.

## 6. Review checklist (main session)

- [ ] Defaults unchanged in value.
- [ ] Each default's doc comment names its source.
- [ ] R8's decision is written and matches the code.
