# Spec 65 Progress: Agent RPC Serving System

## Status: In Progress

Features 00–05 complete. Feature 06 (WASM Lazy Streaming) is in progress: T1–T4
committed, T5 blocked on a parking bug whose root cause is now pinned.

## Feature 06 — WASM Lazy Streaming

| Task | Status | Commit |
|------|--------|--------|
| T1 — parking `into_future_stream` bridge + pure-lens rename | ✅ | `59c4ef14d` |
| T2 — `run_future` → `block_on_collect`, single-threaded `FutureIterator` fix | ✅ | `35d27d89f` |
| T3 — lazy wasm `send_sse` + `SseProgress::Failed` error channel | ✅ | `4ce7ad6e5` |
| T4 — map `Failed` → `ModelState::Error`, wasm `sleep` no-op | ✅ | `89db93212` |
| T5 — verify `cf-cloud-provider` live streaming under `wrangler dev --local` | 🔧 in progress | — |

## T5 root cause (pinned 2026-10-20, hard facts)

`POST /generate` hangs / returns 0 bytes under `wrangler dev --local` because the
generate task is **kept active instead of parked** while the lazy SSE fetch is in
flight.

**Reproduction** — `foundation_core/tests/valtron/park_busy_loop.rs` (native,
single-threaded `execute` engine) drives a task that emits `Init → Pending → Wait`
forever and polls once:

- `forever_wait_parks_not_busy_loops` — **FAILED** (10 s `#[valtron_test]` timeout;
  `poll_next` never returned).
- `forever_depends_parks_not_busy_loops` — **PASSED** (parks, yields `Pending`).

**Mechanism** — the two park arms diverge in
`backends/foundation_core/src/valtron/executors/local.rs`:

- `State::Depends` (`local.rs:1635-1721`) registers `Sleepable::Readiness`, drops
  the task from active work, and the executor goes idle until the signal fires. ✅
- `State::Wait` (`local.rs:1624-1634`) unpacks the task, `push_front`s it back into
  `processing` still active, and returns `ProgressIndicator::Wait` — it **never
  parks** and never registers a sleeper, so the executor re-polls the same task
  forever (native: `yield_for` sleep-loop → the measured hang; wasm: `setTimeout`
  loop that never lets the isolate idle).

**Fault** — `backends/foundation_ai/src/agentic/agent_loop.rs:871-877` maps the
in-flight SSE to the wrong status:

```rust
Stream::Wait => TaskStatus::Wait   // "yield and re-poll me" — WRONG for I/O wait
```

`TaskStatus::Wait` means *yield, I'm still active*, not *park me until external
data arrives*. The lazy `spawn_local` fetch delivers its chunks but nothing wakes
the valtron task, because it was never parked on a readiness signal.

**Fix** — park the generate task on the SSE delivery queue's readiness: expose a
`QueueReadiness` from `LazySseIterator`'s delivery queue and return
`TaskStatus::Depends(signal)` from the `Stream::Wait` arm (the pattern
`steering.rs` already builds and netio's H1 client already uses via
`TaskStatus::Depends(SocketReadiness)`).

## Feature status

- **00-foundation** — ✅ complete
- **01-agent-rpc** — ✅ complete
- **02-manifest-installer** — ✅ complete
- **03-proxy-registration** — ✅ complete
- **04-registrar-sidecar** — ✅ complete
- **05-cloudflare** — ✅ complete
- **06-wasm-async-streaming** — 🔧 T5 in progress (root cause pinned, fix pending)

## Next steps

1. Expose the SSE delivery `QueueReadiness` from `LazySseIterator` /
   `WasmSseIterator`.
2. Change `agent_loop.rs:871` to return `TaskStatus::Depends(signal)` for the
   in-flight `Stream::Wait` case.
3. Re-run `cargo test -p foundation_core --profile uat --test mod valtron::park_busy_loop`
   and the OpenAI / Workers-AI stream tests.
4. `worker-build --release` + live smoke test under `wrangler dev --local`.
5. Remove the temporary `VALTRON-DBG` logs (scattered in `local.rs`,
   `wasm_bindgen/mod.rs`) in the same change; update `docs/bug-backlog.md`.

_Created: 2026-10-20_
