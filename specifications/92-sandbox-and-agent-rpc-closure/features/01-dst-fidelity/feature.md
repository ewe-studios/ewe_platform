---
feature: "DST fidelity: the real registry under the simulator"
status: "pending"
priority: "high"
model: "sonnet"
phase: "1"
depends_on: []
crates: ["foundation_ai", "foundation_core", "foundation_agent_rpc_dst"]
---

# Feature 01: DST fidelity — the real registry under the simulator

_Written in ASD-STE100 Simplified Technical English (STE)._

## 1. Goal

Make the deterministic simulation test (DST) `TestWorld` run the real
`LocalInstanceRegistry` and `AgentSession`, with `ScriptedCatalog` as the only fake, so the
session build, the turn guard, shutdown and destroy are under test (#908 part b).

## 2. The problem, in plain words

The turn emits events through a future stream. That stream is built with
`into_future_stream`, which **parks the caller's thread** when the loop waits
(`foundation_core/src/valtron/stream_future.rs:352`). On a multi-threaded engine another
worker makes progress, so parking is fine. On the single-threaded simulation engine the
producer needs the same thread that is parked, so no event ever arrives, and the first turn
hangs.

## 3. The fix

A cooperative event stream for the single-threaded path:

1. `poll_next` advances the loop with `ExecutionIterator::next(entry, engine, ctx)`
   (`foundation_core/src/valtron/task.rs:1369`).
2. A produced item returns `Ready`. `Progressed` and `Reschedule` loop again.
3. `Wait` returns `Pending`, with the waker registered on the loop's queue readiness.
   Register first, then re-check, so a push between the two cannot be lost.
4. `Depends(readiness)` registers the waker on that readiness and returns `Pending`.
5. `Pending(duration)` returns `Pending` and wakes at the deadline from the `Ctx` timer.
6. `Done` or `None` returns `Ready(None)`. `Panicked` or `SpawnFailed` returns one error item.

`TurnEvents` uses this stream when the build has no `multi`; the multi build keeps the
parking stream.

## 4. Tasks

| Task | Work | Closes |
|---|---|---|
| T1 | Write the failing test first: on `SimRuntime`, one turn over `LocalInstanceRegistry` completes | the red test |
| T2 | Add the cooperative stream in `foundation_core` next to `ExecutionIterator`, with a unit test for each `State` arm | — |
| T3 | Switch `foundation_ai`'s `TurnEvents` to it under `not(feature = "multi")` | — |
| T4 | Replace the `ScriptedRegistry` in `foundation_agent_rpc_dst` with the real registry; keep the 32-seed determinism test and the injected storage failure | #908 |

## 5. Tests

| Test | Oracle |
|---|---|
| `cooperative_stream_arms` (each state, one test) | example |
| `dst_create_enroll_one_turn_destroy_from_one_seed` with the real registry | expected outcome |
| `dst_same_seed_gives_identical_traces_for_32_seeds` | stateful model |
| `dst_injected_storage_failure_gives_a_typed_error` | fault |

## 6. Verify

```bash
cargo test -p foundation_core --features testing --profile uat sim
cargo test -p foundation_agent_rpc_dst --profile uat dst_
cargo check -p foundation_ai --no-default-features
cargo test -p foundation_agent_rpc --profile uat native_
```

## 7. Done when

The DST run uses the real registry and session, the 32 seeds give the same trace twice, and
#908 is closed.
