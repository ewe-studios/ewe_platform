# Spec 65 Progress: Agent RPC Serving System

## Status: In Progress

Feature 06 (WASM Lazy Streaming) is complete. Feature 00 (foundation) is 6/7:
only T7 (deploy to Cloudflare with a live key) is open. Features 01–05 have not
been started. An earlier version of
this file said "Features 00–05 complete"; the feature docs and the code say
otherwise (no agent-rpc / installer / registrar crates exist yet).

## Feature 06 — WASM Lazy Streaming

| Task | Status | Commit |
|------|--------|--------|
| T1 — parking `into_future_stream` bridge + pure-lens rename | ✅ | `59c4ef14d` |
| T2 — `run_future` → `block_on_collect`, single-threaded `FutureIterator` fix | ✅ | `35d27d89f` |
| T3 — lazy wasm `send_sse` + `SseProgress::Failed` error channel | ✅ | `4ce7ad6e5` |
| T4 — map `Failed` → `ModelState::Error`, wasm `sleep` no-op | ✅ | `89db93212` |
| T5 — `cf-cloud-provider` live streaming under `wrangler dev --local` | ✅ | this branch (see git log) |

## T5 — what was actually wrong (two hangs, in order)

Smoke test: `wrangler dev --local` + a mock OpenRouter (`OPENROUTER_BASE_URL`)
that streams 7 SSE chunks 300 ms apart.

1. **The first turn froze before any fetch.** `OpenAIProvider::get_model` misses
   its cache on the first turn and did a sync `GET models/{name}` through
   `HttpClient::send`, which on wasm was `block_on_collect`: a nested executor
   loop inside the running agent task. The running task's slot counts as active,
   so that loop never saw "nothing runnable" and spun on `CanProgress(None)`. The
   JS event loop never ran, so the fetch never left the isolate (the mock saw zero
   requests). Fixed by removing `block_on_collect` from every wasm path: wasm
   `HttpClient::send` now returns an error pointing at `send_async`, and wasm
   `get_model` skips the optional metadata lookup (it already fell back on error).
2. **Then the generate task busy-looped on `Wait`.** With the fetch in flight the
   SSE stream yields `Stream::Wait`; the agent loop returned `TaskStatus::Wait`,
   which keeps the task active and pushes a `Wait` item into the output queue.
   Each push woke the consumer, whose re-poll started another `setTimeout`
   chain, so timer chains multiplied (~1,700 wakeups/s, workerd ~100% CPU, still
   zero fetches). Fixed by parking:
   - `BoxedSseIterator` and `ModelStreamBox` are now `WakeableStream`s; the SSE
     model streams forward `wake` to the SSE stream (wasm `LazySseIterator` /
     `WasmSseIterator` fire on fetch resolve / every chunk / close).
   - On `Stream::Wait` the agent loop registers a `WakeSignal` callback and
     returns `TaskStatus::Depends(signal)` (falls back to `Wait` when the stream
     reports `NotSupported`).
   - JS wasm `run_until` returns to the event loop when every task is parked on
     readiness (no spin, no timer).
   - The wake is event-driven: `WakeSignal`'s callback calls
     `request_executor_resume()`, which queues one coalesced executor run as a
     microtask (`spawn_local` on wasm-bindgen; a new `schedule_microtask` host
     import → `queueMicrotask` on foundation_wasm).

Result: `POST /generate` streams all chunks in ~8.5 s (7 × 0.3 s per model call),
`GET /` answers right after, workerd at ~1.6% CPU.

Also fixed on the way: `OPENROUTER_BASE_URL` doubled the version segment
(`/api/v1/v1/chat/completions`); foundation_wasm ran scheduled callbacks under the
registry lock (re-entrant lock on reschedule) and never removed fired timeouts;
temporary `VALTRON-DBG` logs removed.

Also fixed: **agent turns over HTTP backends tripped loop detection.** The HTTP
SSE streams send snapshots (each chunk re-sends the text so far), which the agent
loop's `coalesce_turn`/`join_text_chunks` already handle, but `LoopDetector` ran
on the raw chunks and saw near-duplicates: a one-line answer took 4 model calls
and ended in `LoopDetected(FuzzyLoop)`, natively and on wasm. Loop detection now
runs on the coalesced turn, and `Turn::text()`/`Answer` join each run of chunks
with the same rule (they concatenated every snapshot). Regression tests:
`session_mock_tests::snapshot_stream_*` (fail without the fix).

Also fixed: native single-threaded drivers re-polled a task that returns
`TaskStatus::Wait` forever without handing back to the caller
(`park_busy_loop::forever_wait_parks_not_busy_loops`, which was red). `Wait`
now reaches the executor as `State::Wait` on all targets and the stream driver
returns to its caller on it (native); other native `run_until` loops keep
re-polling.

Found and recorded in `docs/bug-backlog.md`: readiness signals that never request
a resume stall on wasm; nested blocking helpers spin on wasm; wasm SSE parser
errors are swallowed; interval callbacks run under the registry lock.

## Post-T5 fix — a failed turn streams its error, not an empty 200

The deployed examples answered a failed turn with an empty 200 (the "zero
response"). The failure was dropped at four layers: the providers (lost the
partial text and the error), `lift_model_item` (mapped a spread error to
`Initializing`), the agent loop (kept only the first spread item), and
`into_text_stream` (dropped the `Failed` event). All four are fixed and proven by
tests — see LEARNINGS.md "Zero response root cause" for the full list.

Green under `--profile uat --features testing`: `provider_tests` 249 passed,
`agentic_tests` 324 passed. The `docs/bug-backlog.md` "`TurnTextStream` drops the
terminal `Failed` event" entry is removed.

## Feature status

- **00-foundation** — 🔧 6/7: T1–T6 done (T6:
  `foundation_toolings/tests/cf_examples.rs`, both examples green under
  miniflare with mocked vendors); T7 (deploy to Cloudflare, live key) open.
- **01-agent-rpc** — ⏳ not started (0/10)
- **02-manifest-installer** — ⏳ not started (0/9)
- **03-proxy-registration** — ⏳ not started (0/8)
- **04-registrar-sidecar** — ⏳ not started (0/7)
- **05-cloudflare** — ⏳ not started (0/6)
- **06-wasm-async-streaming** — ✅ complete (5/5)

## Next steps

1. Feature 01 (agent-rpc server), then 02 → 05 in order.
2. Feature 00 T7: deploy + live OpenRouter run (needs a Cloudflare account and
   `OPENROUTER_API_KEY`; not available in this sandbox).
3. Spec 68 leftovers: the owned (`js-foundation-wasm`) path is blocked on
   `foundation_compact`'s wasm-bindgen dependency (backlog §5); browser run.

_Created: 2026-10-20_
