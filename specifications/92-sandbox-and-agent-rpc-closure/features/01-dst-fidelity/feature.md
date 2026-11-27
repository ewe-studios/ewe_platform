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
`into_future_stream` (`foundation_core/src/valtron/stream_future.rs:373`). It is not a
blocking call: it registers the caller's waker and returns `Pending`. The wait it registers
is the defect. For `Stream::Wait`, the engine's meaning is "the queue is empty; the executor
should yield and re-check". On a multi-threaded pool another worker advances the producer and
fires the wake. On the single-threaded simulation engine nobody else runs, so the wake is
never sent, and the first turn hangs.

## 3. The fix

The existing futures adapter is already async: `StreamParkingFutureStream::poll_next`
(`foundation_core/src/valtron/stream_future.rs:373`) advances the loop, returns `Ready` for
an item, and registers the caller's waker before it returns `Pending`. The type and its
callers stay.

The defect is one signal on one engine. `Stream::Wait` means "the queue is empty; the
executor should yield and re-check". On the multi-threaded pool another worker advances the
producer and fires the wake. On the single-threaded engine nobody else runs, so the adapter
waits for a wake that only the producer could send, and the first turn hangs.

| Stream signal | Multi-threaded | Single-threaded |
|---|---|---|
| `Next` | `Ready` | `Ready` |
| `Depends(readiness)` | register the waker | register the waker (unchanged) |
| `Wait` | register, `Pending` | **yield and re-poll** |
| `Pending(duration)` | `Pending` | `Pending`, with the injected timer |

The re-poll policy must not spin: a re-poll happens only when the loop made progress, or the
stream returns a readiness wait or a timer wait. A task that makes no progress stops after a
bounded number of immediate re-polls and returns `Pending` on a wake that the engine will
honour. The bound is a field, with a documented default.

If the turn still stalls after this change, the remaining suspect is the single-threaded
driver not honouring a `Depends` readiness wake (`SimRuntime`). That is a harness fix, not a
change to `ExecutionIterator`.

## 4. The evidence comes first

No production code changes until four failing tests prove the cause and the layer:

1. `wait_arm_never_repolls_single_threaded` — the adapter returns `Pending` on `Wait` while the producer still has items, and nothing re-polls it.
2. `wait_arm_completes_with_a_producer_thread` — the same stream completes when another thread advances the producer. The difference is the engine, not the adapter.
3. `dst_real_registry_turn_hangs_today` — one turn over the real `LocalInstanceRegistry` and `AgentSession` on `SimRuntime` does not finish. A watchdog makes it fail in seconds, never hang CI.
4. `sim_honours_depends_readiness` — a task waiting on an `EventReadiness` resumes when a push fires it. If this passes, the harness honours readiness and the `Wait` arm is the only defect; if it fails, the fix belongs in `SimRuntime`.

Each test has a watchdog. The failing outputs are recorded on the issue, and they decide which layer changes.

## 5. What the evidence proved (2026-11-27, branch `test/dst-hang-proof`)

The four tests ran before any production change:

| Test | Result | Conclusion |
|---|---|---|
| `wait_arm_never_repolls_single_threaded` | **fails** — the future stays `Pending`, the producer advanced 2 of 5 steps, one producer wake registered that only a producer could fire | the `Wait` arm is the defect |
| `wait_arm_completes_with_a_producer_thread` | passes | the adapter is sound when someone can fire the wake |
| `sim_honours_depends_readiness` | passes | `SimRuntime` honours a `Depends` wake; the harness is not the defect |
| `dst_real_registry_turn_hangs_today` | not written | the real registry is gated on `multi` or `worker`, and `SimRuntime` on `not(multi)`; both cannot exist in one build today |

Two extra facts for the tasks:

1. `LocalInstanceRegistry` is `cfg(any(feature = "multi", feature = "worker"))` (`foundation_agent_rpc/src/core/registry/mod.rs:14`), and `multi` pulls `foundation_core/multi`, which removes `SimRuntime`.
2. `foundation_ai`'s deterministic `testing` provider also needs `multi` (`foundation_ai/Cargo.toml:202`).

So T4 also needs `core::registry::local` and a deterministic model provider to compile without `multi`; otherwise the harness keeps its scripted world.

## 6. Tasks

The work has two stages. Stage A fixes the adapter. Stage B moves the real registry
under the simulator. Stage B needs code that does not compile today (section 5).

### Stage A: the `Wait` arm (now)

| Task | Work | Closes |
|---|---|---|
| T1 | Write the failing test first: a task iterator that returns `Wait` twice, then `Ready(7)`, over the real `DrivenStreamIterator` on `SimRuntime`, never completes today | the red test |
| T2 | Fix the `Wait` arm of the five parking adapters in `stream_future.rs` with the redrive policy of section 3; unit tests for `Next`, `Depends`, `Wait` and `Pending(duration)`, a spin-guard test and a proptest model | — |
| T3 | `TurnEvents` and `ExecutionIterator` stay as they are; prove the multi-threaded path is unchanged by its existing tests | — |

### Stage B: the real registry

| Task | Work | Closes |
|---|---|---|
| T4 | Compile `core::registry::local` without `multi`, and add a deterministic model provider without `multi`; then replace the `ScriptedRegistry` in `foundation_agent_rpc_dst` with the real registry; keep the 32-seed determinism test and the injected storage failure | #908 |

### The redrive policy in code

The adapter cannot see the engine. It can see whether the stream drives its own
producer: `next()` runs the executor until the task reaches a state. That is the fact
the fix uses. `WakeableStream` gains a defaulted method:

```rust
/// True when `next()` itself runs the producer, so a `Wait` means "run it
/// again", not "wait for another thread". `DrivenStreamIterator` returns
/// `true`; every other stream keeps the default `false`.
fn drives_its_producer(&self) -> bool {
    false
}
```

The two `DrivenStreamIterator` types (sendable and non-sendable) return `true`.
Every wrapper that forwards `wake` and `wake_send` must also forward this method.

On `Wait` from a driving stream the adapter re-polls, and the budget of section 3
bounds the re-polls. When the budget is spent, the adapter registers the wake,
self-wakes and returns `Pending`, so the driver yields once and then re-polls. On
`Wait` from any other stream the adapter keeps today's register-and-park.

The budget is a field `wait_redrive_budget` on each adapter, with the builder
`with_wait_redrive_budget`. Its default is `DEFAULT_WAIT_REDRIVE_BUDGET` in
`executors/constants.rs`: 1 on wasm (yield to the event loop at once) and 100 on
native. The native value matches `DEFAULT_NOTIFY_QUEUE_MAX_SPINS`, the executor's
existing bound on how often a consumer yields to the producer before the driver
acts.

## 7. Tests

| Test | Oracle |
|---|---|
| `wait_redrives_a_driving_stream` (a task iterator over `DrivenStreamIterator` returns `Wait`, `Wait`, `Ready(7)`) | example: the stream completes in one poll |
| `wait_parks_a_non_driving_stream` | the adapter returns `Pending`, and a producer wake still completes it |
| `wait_redrive_is_bounded` | the adapter re-polls exactly the budget, registers the wake, self-wakes and returns `Pending` |
| `wait_redrive_resets_on_progress` | stateful model: only a no-progress `Wait` spends the budget |
| `wait_redrive_property` (proptest over `Wait`/`Next`/`Ignore` sequences) | the adapter yields the `Next` payloads in order and terminates |
| a wrapper forwards `drives_its_producer` | identity |
| the multi-threaded suites, unchanged | no regression |
| `dst_create_enroll_one_turn_destroy_from_one_seed` with the real registry | expected outcome (stage B) |
| `dst_same_seed_gives_identical_traces_for_32_seeds` | stateful model (stage B) |
| `dst_injected_storage_failure_gives_a_typed_error` | fault (stage B) |

## 8. Verify

```bash
# Stage A: the single-threaded tree (the DST engine) and the pool tree.
cargo test -p foundation_core --profile uat valtron::stream_future
cargo test -p foundation_core --profile uat --features multi valtron::stream_future
cargo test -p foundation_core --profile uat --features multi valtron::
cargo test -p foundation_core --profile uat --features testing valtron::
cargo clippy -p foundation_core --profile uat --all-targets -- -D warnings
cargo clippy -p foundation_core --profile uat --all-targets --features multi -- -D warnings
# Stage B adds:
cargo test -p foundation_agent_rpc_dst --profile uat dst_
cargo check -p foundation_ai --no-default-features
cargo test -p foundation_agent_rpc --profile uat native_
```

## 9. Done when

Stage A: the redrive tests pass, the pool suites are unchanged, and the evidence file
`tests/valtron/dst_hang_proof.rs` holds the positive regression tests instead of the
red test. Stage B: the DST run uses the real registry and session, the 32 seeds give the
same trace twice, and #908 is closed.
