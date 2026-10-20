---
workspace_name: "ewe_platform"
spec_directory: "specifications/65-agent-rpc-serving-system"
feature_directory: "specifications/65-agent-rpc-serving-system/features/06-wasm-async-streaming"
this_file: "specifications/65-agent-rpc-serving-system/features/06-wasm-async-streaming/start.md"
created: 2026-10-20
---

# Start: WASM Lazy Streaming

## Agent Workflow

1. Read `feature.md` (root cause + requirements + changes table + tasks).
2. Read the prior work this extends:
   - `../00-foundation/feature.md` (the wasm readiness this fixes at runtime)
   - `../../LEARNINGS.md` (the T6 hang root cause + T1–T5 resolutions)
3. **Identify language stack** — Rust. Read `.agents/skills/rust-clean-code/skill.md`
   and `.agents/skills/rust-valtron-usage/skill.md` (the parking bridges
   `into_next_stream` / `into_future_stream` / `StreamNextStream`, the
   `WakeableStream` trait, the raw lens `StreamAsFutureStream`, and the wasm
   single-threaded executor).
4. Read `.agents/AGENTS.md` — especially "async all the way" and "stream bodies,
   never buffer them".
5. Work on **ONE item at a time**, in task order T1 → T5:
   - **T1 — bridge fix + parking tests (foundation_core).** Rename the raw lens
     `into_future_stream()` → `into_pure_future_stream()` (still forwards
     `Stream<D, P>` as-is, never parks). Make `into_future_stream()` an
     await-friendly full-enum lens that parks on `Wait`/`Pending`/`Init`/`Delayed`
     via `WakeableStream::wake` (self-wake fallback). Add parking tests for
     `into_future_stream` / `into_next_stream` / `into_collect_future` /
     `into_ready_future` asserting `Poll::Pending` on control states and
     resume-on-wake. Update existing raw-lens callers to
     `into_pure_future_stream`. Compile-check native + wasm.
   - **T2 — `run_future` audit (foundation_core).** Rename `run_future` →
     `block_on_future` and document it wasm-unsafe (native-only); it has no
     native callers today, only the three wasm traps removed in T3/T4. Fix the
     single-threaded `ThreadedIterFuture` / `run_future_iter` wasm path (polls
     inline with a no-op waker and drops the future on first `Pending`). wasm
     compile-check.
   - **T3 — lazy `send_sse` + error channel (foundation_netio).** Add
     `SseProgress::Failed(String)` (drops `Copy`). Make wasm `send_sse` return a
     lazy `LazySseIterator`: first poll spawns `do_fetch` via `spawn_local`, yields
     `Pending(Connecting)`/`Wait` until it resolves, then flattens
     `WasmSseIterator`; a fetch failure surfaces as `Pending(Failed(msg))` then
     `None`. `impl WakeableStream for WasmSseIterator` and for the fetch stage
     (a `WakeRegistry` fired on data/close). `send_sse` drops `run_future`;
     `send_sse_async` now uses the parking `into_future_stream`. Compile-check
     native + wasm.
   - **T4 — map error + fix `sleep` (foundation_ai).** `OpenAIStream` /
     `WorkersAiStream` map `Stream::Pending(SseProgress::Failed(msg))` →
     `Stream::Pending(ModelState::Error(msg))`. Fix wasm `backend_utils::sleep`
     (currently `run_future(sleep_async)` — a wasm deadlock on the retry path).
     Zero change to the `Model` trait: `stream()` keeps its signature and becomes
     non-blocking on wasm once `send_sse` is lazy. Run the existing OpenAI /
     Workers-AI stream tests before and after to prove behaviour is preserved.
   - **T5 — verify `cf-cloud-provider` (examples).** No structural change needed:
     `stream()` is now lazy; the existing `into_next_stream()` drives it. Verify
     `POST /generate` streams a live OpenRouter answer under `wrangler dev --local`
     without blocking the isolate (`GET /` still responds after a generate).
6. Track the T6 hang root cause — the wasm-unsafe `run_future` / single-threaded
   `ThreadedIterFuture` sync `collect()` loop — in `docs/bug-backlog.md`
   (CLAUDE.md rule #9) in the same change, and delete each entry in the task that
   fixes it.
7. Update `../../LEARNINGS.md` after each completed task/milestone.

**Start with T1 (the bridge fix)** — every later task builds on the parking
bridge and the removal of `run_future`.

---

**Workflow:** feature.md → 00-foundation → LEARNINGS → Rust skills → AGENTS.md →
ONE ITEM AT A TIME (T1 → T2 → T3 → T4 → T5) → compile-check each → report →
review.

---

_Created: 2026-10-20_
