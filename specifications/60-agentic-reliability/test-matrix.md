# Spec 60 — test matrix

Every interaction, workflow and process that must be covered. This is the
checklist S7 fills to green, and the reference the coverage report is read
against: a line covered by no row here is either dead code or a missing row.

## Legend

**Tier** — what drives the test:

| Tier | Meaning |
|------|---------|
| `mock` | `agentic::testing::MockModelProvider` — deterministic, instant |
| `candle` | tiny-random fixture via `CandleBackend` (safetensors) |
| `gguf` | same fixture converted to GGUF via `LlamaBackends` |
| `both` | must pass on candle **and** gguf — identical weights, so a divergence is ours |
| `gen` | generated synthetic weights (tier 1) |
| `real` | SmolLM2-135M-Instruct / Gemma 4 E2B, `integration_tests` only |

**Status** — `done` / `todo` / `n/a`.

---

## 1. AgentLoop — state machine

Every state and every edge out of it.

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 1.1 | `Initializing` emits `Init` and moves to `OuterBoundary` | mock | todo |
| 1.2 | `OuterBoundary` with priority messages → drains, resets cancel, `InnerAssemble` | mock | todo |
| 1.3 | `OuterBoundary` with follow-up messages → drains, `InnerAssemble` | mock | todo |
| 1.4 | `OuterBoundary` with both → **priority drains first** | mock | **done** |
| 1.5 | `OuterBoundary` with empty queues → `Ending` | mock | todo |
| 1.6 | `OuterBoundary` past `max_outer_iterations` → `Ending` | mock | todo |
| 1.7 | `InnerAssemble` with priority pending → front-injects, stays in assemble | mock | todo |
| 1.8 | `InnerAssemble` with exhausted budget → `Ending` + `BudgetExhausted` | mock | todo |
| 1.9 | `InnerAssemble` router failure → `handle_error`, no panic | mock | **done** |
| 1.10 | `InnerAssemble` builds interaction carrying system prompt + messages + toolshed | both | todo |
| 1.11 | `InnerGenerate` pumps stream, collects messages | both | **done** |
| 1.12 | `InnerGenerate` on stream end with no tool calls → `OutputProcessing` | mock | todo |
| 1.13 | `InnerGenerate` on stream end with tool calls → `InnerToolCalls` | mock | **done** |
| 1.14 | `InnerGenerate` on provider error → `FailedAction`, `run_turn` returns `Err` | both | **done** |
| 1.15 | `InnerToolCalls` extracts calls into `InnerExecuting` | mock | **done** |
| 1.16 | `InnerExecuting` drives each call, collects results | mock | **done** |
| 1.17 | `InnerExecuting` tool failure → recorded, loop continues per policy | mock | **done** |
| 1.18 | `InnerExecuting` cancellation signal → aborts in-flight tools | mock | todo |
| 1.19 | `InnerEmitResults` emits results and returns to `InnerAssemble` | mock | **done** |
| 1.20 | `InnerAssemble` past `max_inner_iterations` → breaks out | mock | todo |
| 1.21 | `OutputProcessing` fires memory triggers and persists | mock | todo |
| 1.22 | `Ending` emits `Summary` with accurate `message_count` + usage | mock | **done** |
| 1.23 | `Done` yields `None` and the task completes | mock | **done** |
| 1.24 | Full happy path: user msg → assistant reply, one outer iteration | both | **done** |

## 2. Guards and limits

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 2.1 | `max_outer_iterations` honoured exactly (n, not n±1) | mock | **done** |
| 2.2 | `max_inner_iterations` honoured exactly | mock | **done** |
| 2.3 | `TokenLedger` exhaustion stops generation | mock | todo |
| 2.4 | `effective_max_tokens` clamps `ModelParams.max_tokens` | mock | todo |
| 2.5 | Context-pressure threshold triggers ephemeral layer | mock | todo |
| 2.6 | Preflight compression threshold triggers compression | mock | todo |
| 2.7 | Usage accounting accumulates across turns | mock | todo |
| 2.8 | Cost accounting is **per session**, not merged across agents sharing a model | both | todo |

## 3. Steering and cancellation

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 3.1 | `steer()` injects priority, interrupts current work | mock | **done** |
| 3.2 | `follow_up()` queues, processed after current work | mock | **done** |
| 3.3 | Priority drains before follow-up in the same boundary | mock | **done** |
| 3.4 | `CancelCode::PauseForPriority` set on priority push | mock | todo |
| 3.5 | `CancelCode::Abort` terminates the loop | mock | todo |
| 3.6 | `reset_cancel()` clears the signal after handling | mock | todo |
| 3.7 | Queues are shared (push before schedule is seen by the loop) | mock | todo |
| 3.8 | Multi-turn: follow-up continues the same session context | both | todo |

## 4. Tools

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 4.1 | `ToolShed` definitions reach the prompt | both | todo |
| 4.2 | Tool call parsed from model output | mock | **done** |
| 4.3 | Tool executed with parsed arguments | mock | **done** |
| 4.4 | Tool result emitted as a record | mock | **done** |
| 4.5 | Tool result fed back into the next assemble | mock | todo |
| 4.6 | Tool failure surfaces without killing the turn | mock | **done** |
| 4.7 | Unknown tool name → error, not panic | mock | **done** |
| 4.8 | Malformed tool arguments → error, not panic | mock | todo |
| 4.9 | Multiple tool calls in one turn all execute | mock | **done** |
| 4.10 | Tool cancellation mid-flight | mock | todo |

## 5. Errors, circuit breaker, loop detection

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 5.1 | Provider error → `FailedAction` record | both | **done** |
| 5.2 | `run_turn` returns `Err` on terminal `FailedAction` | both | todo |
| 5.3 | `ErrorPolicy` retry decision honoured | mock | todo |
| 5.4 | `CircuitBreaker` opens after `circuit_breaker_threshold` failures | mock | **done** |
| 5.5 | Open breaker selects the next `fallback_models` entry | mock | todo |
| 5.6 | Breaker resets on success | mock | todo |
| 5.7 | `LoopDetector` detects repetition and escalates | mock | todo |
| 5.8 | Escalation terminates rather than looping forever | mock | todo |
| 5.9 | A failure NEVER presents as an empty successful turn (docs/fixes/006) | both | **done** |

## 6. Memory and persistence

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 6.1 | `hydrate_sync` loads working memory into context | mock | todo |
| 6.2 | Context assembled in documented order (system → working → reflection → recent) | mock | todo |
| 6.3 | Memory triggers fire on `OutputProcessing` | mock | todo |
| 6.4 | User prompt persisted via `MessageApi` | both | **done** |
| 6.5 | Assistant reply persisted | mock | **done** |
| 6.6 | `MessageApi::recent(n)` returns the last n | mock | todo |
| 6.7 | `flush()` writes buffered records | mock | todo |
| 6.8 | Session resume rehydrates prior state | mock | todo |
| 6.9 | `end()` drains queues and persists remaining messages | mock | **done** |

## 7. AgentSession API

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 7.1 | `run_turn` returns assistant reply | both | **done** |
| 7.2 | `run_turn_stream` yields records progressively | both | **done** |
| 7.3 | `run_turn` on a failing provider returns `Err` | both | **done** |
| 7.4 | Second turn answers and reuses loaded weights | both | **done** |
| 7.5 | Concurrent sessions on one model do not corrupt each other | both | **done** |
| 7.6 | Builder rejects invalid config (preflight) | mock | todo |
| 7.7 | `session_id` round-trips | mock | todo |

## 8. Provider seam — both backends, identical weights

The rows that matter most: every `docs/fixes/006` defect lived here.

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 8.1 | `generate()` produces text | both | **done** (real) |
| 8.2 | `stream()` produces text | both | **done** (real) |
| 8.3 | `generate()` and `stream()` agree structurally for one interaction | both | todo |
| 8.4 | Stream advances past its first token (KV cache progresses) | both | todo |
| 8.5 | Stream terminates on EOG | both | todo |
| 8.6 | Stream terminates on `max_tokens` | both | todo |
| 8.7 | Inference context created lazily on the polling thread | gguf | todo |
| 8.8 | Context is never cloned/double-freed (UAF regression guard) | gguf | todo |
| 8.9 | `interaction.messages` reach the prompt (not just system) | both | todo |
| 8.10 | Chat template applied when the model ships one | both | todo |
| 8.11 | Documented fallback + log when no chat template | gen | todo |
| 8.12 | Model weights cached — second load is a cache hit | both | todo |
| 8.13 | Cache key distinguishes differing configs | both | todo |
| 8.14 | Cold-start race loads once, not once per caller | both | todo |
| 8.15 | Sampling: `temperature<=0` → greedy/argmax | both | todo |
| 8.16 | Sampling: `top_k` changes output | candle | todo |
| 8.17 | Sampling: `top_p` changes output | candle | todo |
| 8.18 | Sampling: `repeat_penalty` changes output | candle | todo |
| 8.19 | Seeded sampling reproducible across two runs | candle | todo |
| 8.20 | Unsupported architecture fails loudly, no silent Llama fallback | candle | todo |
| 8.21 | Architecture detected from `config.json` when unconfigured | candle | todo |
| 8.22 | Each supported architecture loads | candle/gen | todo |
| 8.23 | Large-vocab model (Gemma2, 256k) loads and tokenizes | both | todo |
| 8.24 | Missing/corrupt safetensors → clear error | candle | todo |
| 8.25 | Missing `config.json` → clear error | candle | todo |

## 9. Harness, router, presets

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 9.1 | `RouterMix::primary` resolves the primary model | both | todo |
| 9.2 | `into_agent_builder` produces a working session | both | **done** |
| 9.3 | Router `get_model` for an unknown id errors clearly | mock | todo |
| 9.4 | Fallback model resolution on primary failure | mock | todo |
| 9.5 | Memory-model routing distinct from primary | mock | todo |
| 9.6 | Preset (`Gemma4E2b`, etc.) builds its provider | real | todo |
| 9.7 | `serves()` asymmetry rules hold across providers | mock | todo |

## 10. Toolbox

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 10.1 | Tool registration and lookup | mock | todo |
| 10.2 | Schema generation for a tool | mock | todo |
| 10.3 | Argument deserialization | mock | todo |
| 10.4 | Tool result serialization | mock | todo |
| 10.5 | Async tool driven to completion | mock | todo |
| 10.6 | Tool panic contained, not process-fatal | mock | todo |

## 11. Cross-cutting

| # | Behaviour | Tier | Status |
|---|-----------|------|--------|
| 11.1 | Worker-thread tracing reaches the subscriber | mock | todo |
| 11.2 | llama.cpp logs silent by default | gguf | todo |
| 11.3 | llama.cpp logs enabled by `llamacpp=debug` directive | gguf | todo |
| 11.4 | No `println!` on library paths | — | todo |
| 11.5 | Fixtures load offline with no network | both | **done** (candle) |
| 11.6 | Whole default suite runs with network disabled | both | todo |

---

## Coverage

`cargo-llvm-cov` (decision 08), workspace-aware, lcov + html.

Read the report against this matrix:

- **covered line, no matrix row** → either dead code (delete it) or a missing row
- **matrix row, uncovered lines** → the test is asserting less than it claims
- **uncovered error path** → the highest-value gap; every defect in
  docs/fixes/006 was an unexercised error path

Target ~90% of `backends/foundation_ai/src/agentic/` and the provider seam.
Exclusions are listed and justified per file, never blanket.
