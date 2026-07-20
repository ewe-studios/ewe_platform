# Spec 60: agentic-reliability — requirements

## Problem

The agentic loop shipped without coverage of its own primary API. `run_turn` had
no test at any level and nothing exercised `Model::stream`; every provider suite
exercised `generate()`. Seven distinct defects lived in that gap, including a
use-after-free, and the user-visible symptom was an agent that replied
`(no response)` to everything (see `docs/fixes/006`).

Those defects are fixed. What remains is the reason they survived: we cannot
cheaply run the agentic surface end-to-end. The llama.cpp path needs multi-GB
weights, takes seconds per turn, and floods stderr — so it is not something a
developer runs on every change, and CI cannot gate on it.

This spec buys a fast, in-process, deterministic-enough way to exercise every
critical path in the session and loop, and closes the remaining known defects.

---

## A. Generation quality — why does the model answer `"."`?

### Observed

With the streaming path repaired, the local Gemma 4 E2B model answers
"Reply with a single short greeting." with degenerate output. Across runs:

- `"."` (single token, most common)
- `" \nHello!"` — correct
- `"**Hello!**"` — correct but marked up
- `"{\n\"greeting\": \"Hello!\"\n}"` — JSON, unprompted

So the model *can* answer; it frequently does not. Both `generate()` and
`stream()` show it, which points at shared prompt construction or sampling
rather than at either path's plumbing.

### Requirements

1. Determine the root cause. Candidate areas, in the order they should be ruled
   out:
   - **Chat template application** — is the rendered prompt well-formed for
     Gemma 4? Dump the exact string fed to `str_to_token`. The model card's
     expected turn structure is the reference.
   - **Sampler configuration** — `build_sampler_chain` and the defaults in
     `ModelParams`. A mis-ordered or mis-parameterised chain (temperature, top-k,
     top-p, repeat penalty) can collapse to a degenerate token.
   - **BOS handling** — `str_to_token(&prompt, AddBos::Always)` adds BOS while
     the chat template likely already emits one. A doubled BOS is a known cause
     of degenerate first tokens.
   - **`max_tokens`** — confirm `effective_max_tokens` is not clamping to ~1.
   - **EOG handling** — Gemma 4 has several EOG tokens (`<eos>`, `<turn|>`,
     `<|tool_response>`); check the stream is not stopping on the first token.
2. Fix it, with the root cause written up as a `docs/fixes/` entry.
3. Strengthen assertions. `!output.is_empty()` passes on `"."`; it is not a
   quality assertion. Tests must assert something a degenerate reply fails.
   Decision 07 governs how strict this can be.

---

## B. llama.cpp logging

### Observed

llama.cpp writes the full model-loader dump, per-tensor `repack:` lines, and
graph-reservation output straight to stderr on every load. In the REPL this
buries the prompt and the app's own output.

### Requirements

1. Route llama.cpp + ggml output into `tracing` via
   `infrastructure_llama_cpp::send_logs_to_tracing`, so it becomes filterable
   events rather than raw stderr writes. The plumbing already exists
   (`logs_to_trace`, per-module `State`, `LogOptions`), including a `void_logs`
   silencer.
2. **Silent by default.** A default-configured app must show none of it.
3. **Enabled by an explicit filter directive** — the user opting in with a
   tracing directive (e.g. `llamacpp=debug`, `llamacpp=trace`) gets the output
   at that level, through the same `EnvFilter` that governs everything else. The
   target name must be documented, since `log.rs` derives targets from llama.cpp
   module prefixes.
4. Initialise once, from a single place (backend init), not per model load.
5. No output may bypass the filter — including the `repack`/loader lines that
   are emitted during `load_from_file`.

---

## C. Known warts

1. **Cold-start cache race.** `LlamaBackends::load_model` checks the cache,
   releases the lock, then loads. Concurrent first-callers therefore all miss and
   all load — measured 2 full disk loads for 6 concurrent calls. Wasteful, not
   incorrect. Fix so exactly one load happens per key while others wait.
2. **Audit remaining silent failures.** The stream's paths were converted to
   `ModelState::Error`; the same audit has not been done for `generate()`,
   `apply_chat_template`, the embedding path, or the provider/router layer. Any
   place returning a success-shaped value on failure is in scope.
3. **Dead field `AgentLoop::pending_user_messages`** — written by
   `push_user_message`, never read. Either it should feed context assembly or it
   should go; decide which and act.
4. **`println!` in `transition_inner_assemble`** ("Router failed to get model")
   violates the house tracing rule and should be `tracing::error!`.

---

## D. Candle test suite — the bulk of the work

### Why candle

The mock provider (`agentic::testing::MockModelProvider`) is excellent for
branch coverage and should carry most of it: it is instant, scriptable, and can
force error paths that a real model will not produce on demand.

But mocks are exactly what let this class of defect through — every bug in
`docs/fixes/006` lived *below* the mock seam, in the real provider. So the suite
needs both:

- **mocks** for exhaustive flow/branch/error coverage of the loop
- **candle**, in-process with a small real model, for the provider seam: that
  `generate()` and `stream()` agree, that a real turn completes, that streaming
  advances, that errors surface

Candle is chosen over llama.cpp for the real-provider tier because it is pure
Rust, in-process, needs no C++ build, and no multi-GB weights.

### Coverage requirements

Target ~90% of critical logic in `backends/foundation_ai/src/agentic/`. Every
one of these must be exercised:

**`AgentLoop` state machine** — all ten states and every transition edge:
`Initializing`, `OuterBoundary`, `InnerAssemble`, `InnerGenerate`,
`InnerToolCalls`, `InnerExecuting`, `InnerEmitResults`, `OutputProcessing`,
`Ending`, `Done`.

**Guards and limits** — `max_outer_iterations`, `max_inner_iterations`, budget
exhaustion via `TokenLedger`, context-pressure threshold, preflight compression
threshold.

**Steering** — priority queue interrupt mid-assemble, follow-up continuation,
cancel signal (`CancelCode::PauseForPriority`, `Abort`), queue drain ordering
(priority before follow-up).

**Tools** — call extraction, execution, results emitted back, tool failure,
tool cancellation.

**Errors** — `ErrorPolicy` decisions, `CircuitBreaker` open/fallback-model
selection, `LoopDetector` escalation, a provider error becoming a
`FailedAction`, and `run_turn` returning `Err` for it.

**Memory** — hydrate/assemble, memory triggers on `OutputProcessing`,
persistence through `MessageApi`.

**Session** — `run_turn` / `run_turn_stream`, `steer`, `follow_up`, `end`,
resume/rehydrate, multi-turn continuity, the prompt being persisted.

**Provider seam (candle)** — `generate()` and `stream()` produce equivalent
text for the same interaction; a stream advances beyond its first token; a
provider failure propagates as an error rather than an empty success.

### Constraints

- Tests live in `tests/` per house rules; no bespoke test machinery — anything
  reusable is elevated into the crate.
- No network at test time. If the candle model must be fetched, it is fetched
  once behind the existing `integration_tests` gate and cached; the default
  suite must run offline.
- The suite must be fast enough to run on every change. Mock-tier tests are
  milliseconds; the candle tier should stay in seconds.
- Coverage is measured, not asserted by eye (decision 06). Exclusions are listed
  and justified, not blanket-ignored.

---

## E. llama.cpp version bump

Bump the vendored `tools/llama.cpp` and update
`infrastructure/llama-bindings` + `infrastructure/llama-cpp` as needed. Lands
**last**, so that any behaviour change is attributable against a suite that is
already green. Includes the upstream `-Wunused-function` noise in the vendored
jinja headers, which should be resolved by the bump or by a build-level decision
recorded here — not by editing vendored sources.

---

## Definition of done

1. The local model answers a greeting coherently, with a `docs/fixes/` entry for
   the root cause.
2. A default app run shows no llama.cpp output; `llamacpp=debug` shows it.
3. Every wart in section C is closed or has a recorded decision not to.
4. ~90% coverage of critical agentic logic, measured, with mock + candle tiers.
5. The full suite runs offline and fast enough to run on every change.
6. llama.cpp bumped with the suite still green.
