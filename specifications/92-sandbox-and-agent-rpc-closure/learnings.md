# Spec 92 Learnings

## Why this spec exists (2026-11-27)

Specs 77 and 83 merged with three kinds of debt:

1. **Proof debt.** The real Cloudflare container path was red, and no live run had used the
   owner's account. The real AWS and GCP KMS paths had never run.
2. **Fidelity debt.** The DST harness used a scripted registry, so the production registry
   code was not under simulation.
3. **Small correctness debt.** Eleven findings stayed open because each feature could land
   without them.

This spec exists to pay those debts. A feature of this spec is done when its issues are
closed, not when its code compiles.

## The DST turn hang is a `Wait` re-drive (2026-11-27)

`Stream::Wait` is not `Pending`. It means "run the producer and check again". On a
multi-threaded pool another worker sends the wake. On the single-threaded simulation engine
nobody else runs, so a parked `Wait` never wakes. The layer was the stream-to-future
adapter, not the readiness harness: `sim_honours_depends_readiness` passed, and the control
test passed with a producer thread.

The fix is a signal, not a new engine. `drives_its_producer()` is `true` for a stream whose
`next()` runs the engine. The adapter re-polls such a stream with a bounded budget. A stream
that never makes progress still parks after it wakes itself. The wasm budget is 1, so the
adapter yields to the event loop at once.

Two traps:

1. A wrapper that forwards `wake`/`wake_send` but not `drives_its_producer` or
   `can_make_progress` hides the fix or the regression. Every forwarding combinator now
   forwards both.
2. A `Wait` does not always mean "run again". The single-threaded driver also returns `Wait`
   after it sleeps out its wait budget for a parked task. Re-driving that `Wait` 100 times
   cost 10 s and broke `depends_on_wake_signal_parks_then_resumes`. The driver now answers
   `can_make_progress()`, and the adapter re-drives only when the engine has runnable work.
3. `foundation_core`'s default and `multi` CI steps were already red (#1108, #1109): the
   `sim_tests` target imported a module gated on `testing`, and `SimRuntime::start` had no
   `# Panics` section. Run clippy on the crate you change, not only on your own targets.

## The real registry needs two more driving-stream fixes (2026-11-28)

Stage B made the DST run the real `LocalInstanceRegistry` and `AgentSession`. Feature wiring
was the small part: `testing-mock` separates the `foundation_ai` mock substrate from `multi`,
and `foundation_agent_rpc/dst` builds the registry, the guarded turn and the mocks with no
thread pool, SQLite or TLS.

Two defects then appeared, and both come from the same rule: on a single-threaded engine the
consumer must drive the producer.

1. **A progress item is not a reason to park.** `StreamParkingFutureStream` parked on
   `Stream::Pending` and `Stream::Init` from a driving stream. The agent loop reports progress
   between ready records, so the turn stalled after its first hint. `foundation_ai` now wraps
   the turn's driven iterator in `TurnItems`, which skips progress and re-drives `Wait` with
   the adapter budget. The root fix belongs in the adapter (#1129).
2. **The engine counts its own caller as runnable.** `ExecutorState::has_runnable_work` counts
   the task that is executing (a parked slot stays in `active_slots`). On the RPC client path
   the response reader is an engine task, and the turn is driven inside it. When the turn ends
   the engine reports `NoWork` with one active task, and `run_until` loops forever (#1128).
   The DST calls the real registry directly for a turn to keep the client reader out of the
   engine. The rest of the calls still go through the transport.

## Stage C: the nested drive and the control states (2026-11-28)

- `EntryList::park` takes the task value out of its slot but keeps the slot, so
  `active_slots()` counts the task that is being polled. `run_until` read that count to
  decide "no work", so a drive that starts inside a task never stopped. Fix:
  `total_active_tasks` subtracts `parked_slots()` as well as sleepers, and `run_until`
  stops when `total_tasks() == total_parked_tasks()`. A sleeper is not packed, so a timer
  still waits. Subtract packed before sleepers, because a stale sleeper can otherwise hide
  a real runnable task.
- `Stream::Pending` and `Stream::Init` are control states for a stream that drives its own
  producer. The adapters re-drive them with the same `DEFAULT_WAIT_REDRIVE_BUDGET` as
  `Wait`. `StreamPendingFuture` is the exception: `Pending` is its result, so it returns
  `Ready` and re-drives only `Init` and `Wait`. `Delayed` is a timer request and keeps
  parking.
- With the adapter fixed, `TurnItems` had no job left: it was a copy of the re-drive policy
  in the wrong crate. Remove it and use `StreamParkingFutureStream` directly.
- The real ConnectRPC client spawns its reader task on the engine. The turn is driven
  inside that task, which is the nested case. `drive()` still polls the client future
  outside the engine and pumps the engine with `run_until_complete`.

Two traps for the next stage:

1. `SimRuntime::block_on` is not needed. A plain driver that polls the client future and calls
   `SimRuntime::run_until_complete` between polls shows the engine's failure directly and does
   not hide it behind an extra task.
2. A trace of the agent loop states is the fastest way to separate a producer stall (no
   `Ready` records) from a consumer stall (`Ready` records but no `TURNITEMS` item). Both
   hangs look the same from the outside.
