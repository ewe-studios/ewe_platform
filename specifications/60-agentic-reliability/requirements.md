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

### The test models (decision 04)

Four tiers — see `start.md` for the table. The default is the **committed
fixtures**, which serve BOTH backends on identical weights:

- `tiny-random-LlamaForCausalLM` — safetensors for candle, GGUF for llama.cpp
- `tiny-random-Gemma2ForCausalLM` — same, and covers a 256k vocab plus a real
  chat template

Generated weights (tier 1) cover architectures we hold no fixture for.
`SmolLM2-135M-Instruct` and Gemma 4 E2B stay behind `integration_tests` for
semantic quality only.

### What random-weight fixtures can and cannot prove

Random weights emit gibberish, and even a real 135M model will not reliably
emit tool calls or follow instructions. Pretending otherwise builds a flaky
suite. So the tiers divide by what each can actually establish:

| Tier | Proves | Examples |
|------|--------|----------|
| **Mock** (`MockModelProvider`) | Deterministic branch + flow coverage. Anything needing the model to emit something *specific*. | tool-call extraction/execution/results, `ErrorPolicy` decisions, `CircuitBreaker` fallback, `LoopDetector` escalation, budget exhaustion, iteration caps, steering interrupts, cancellation |
| **Fixtures** (candle + gguf) | The real provider seam that mocks cannot see — where every `docs/fixes/006` defect lived. Real tokenizers and real chat templates. | a turn completes end-to-end; `generate()` and `stream()` agree; a stream advances past its first token; the chat template is applied; a provider error propagates as `FailedAction` |
| **Real models** (`integration_tests`) | Semantic quality only. | the model answers a greeting coherently (workstream S9) |

Assertions in the candle tier are therefore **structural, not semantic** — that
text was produced, that streaming progressed, that records have the right shape
— never that the reply is a good answer. Semantic quality is workstream A's
problem, on the real model.

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

---

# Provider workstreams (absorbed from spec 61)

## A. Architecture coverage

### Current

```rust
pub enum CandleArchitecture {
    Llama,
    Custom(String),
}
```

`build_candle_model` dispatches `Llama` to `build_llama_model` and turns
`Custom(name)` into `UnsupportedArchitecture(name)`. Nothing else can load.

### Requirements

1. Replace the two-variant enum with explicit variants per supported
   architecture. `Custom(String)` either gains real dispatch or is removed — a
   variant that always errors is worse than no variant, because it implies an
   extension point that does not exist.
2. Architecture selection must be **derivable from the model**, not just
   configured. `config.json` carries `model_type` / `architectures`; a caller
   should not have to know that a repo is Qwen3 to load it. Explicit config
   overrides inference.
3. Each supported architecture needs its `build_*_model` loader, following the
   existing `build_llama_model` shape: config parse → `VarBuilder` over the
   safetensors → model construct → cache/state init.
4. An unsupported architecture must fail loudly at load with the detected name
   and the list of supported ones — never a silent fallback to Llama, which
   would produce garbage that looks like a bad model rather than a missing
   implementation.
5. The initial set is decision 02. Suggested first cut, chosen for coverage of
   what people actually run locally and for shared structure with Llama:
   **Llama, Qwen2, Qwen3, Mistral, Phi3, Gemma2/Gemma3**. Mamba/RWKV are
   deliberately excluded from the first cut — their state handling differs from
   the KV-cache shape the current `forward`/`CandleStream` assumes.

### Decision 06 — state handling moves into the model

`forward()` and `CandleStream` currently assume a Llama-style `cache` argument.
Architectures differ: Mistral/Qwen use similar KV caches, Gemma2 adds sliding
window/attention logit softcapping, and Mamba/RWKV carry recurrent state with no
KV cache at all.

**The per-architecture state is part of the model abstraction, not something the
stream assumes.** Concretely: the model owns its own state type and exposes a
uniform step interface; `CandleStream` drives that interface without knowing
which architecture it holds. No architecture may be forced into a Llama-shaped
cache it does not fit.

Doing this **before** workstream A is deliberate — adding the second
architecture on top of the current Llama-shaped assumption would entrench it,
and each subsequent architecture would fight the first.

---

## B. Prompt construction

### Current

```rust
fn build_prompt(_tokenizer: &Tokenizer, interaction: &ModelInteraction) -> String
```

The tokenizer parameter is unused. The function concatenates `system_prompt` and
`soul` and formats messages into a hand-rolled string.

### Requirements

1. Apply the **model's own chat template**. An instruct model prompted without
   its turn structure has no shape to answer into — this is exactly what
   docs/fixes/006 diagnosed on the llama.cpp side.
2. The template lives in `tokenizer_config.json` (`chat_template`) as a Jinja
   string. Rendering it needs a Jinja engine: `tokenizers` does not do chat
   templates, and llama.cpp's minja is C++ behind our FFI shim, so it is not
   reachable from the candle path.

   **Decision 03: use `minijinja`.** It is already a workspace dependency —
   `foundation_packager` uses `minijinja = { version = "2.0.0" }` for its file
   generation stack (`files.rs`, `TemplateKind::Jinja`), so the engine is proven
   here and adds no new vendor surface. Pure Rust, so unlike minja it works on
   every target including wasm.

   Note the committed fixtures make this testable offline:
   `tiny-random-Gemma2ForCausalLM` ships a real `tokenizer_config.json` with a
   real chat template.
3. Whatever is chosen, `generate()` and `stream()` must use the **same** prompt
   construction. The llama.cpp bug was precisely that they diverged.
4. Tool definitions must be carried into the prompt (`ToolShed`), as the
   llama.cpp path does via `TextBasedFormatter`.
5. When a model ships no chat template, fall back to a documented plain format —
   and say so in a log, not silently.

---

## C. Sampling

### Current

Hand-rolled `sample_token`, `argmax`, `sample_top_k`, `sample_from_logits`.
Supports temperature and top-k. No top-p, no repeat penalty, no seed.

### Requirements

1. Adopt `candle_transformers::generation::LogitsProcessor`, which implements
   `ArgMax`, `All { temperature }`, `TopP`, `TopK`, `TopKThenTopP` and
   `GumbelSoftmax`. Delete the bespoke samplers — they are strictly less
   capable and separately maintained.
2. Honour `ModelParams` fully: `temperature`, `top_p`, `top_k`,
   `repeat_penalty`. A knob that is accepted and ignored is a silent failure.
3. **Seeded sampling** (decision 05). `LogitsProcessor::new` takes a seed;
   without one, candle generation is not reproducible, which blocks its use for
   deterministic assertions in spec 60.
4. `temperature <= 0` must map to `ArgMax` (greedy), which is also the natural
   deterministic mode for tests.

---

## D. Offline test models — three tiers (decision 04)

All three, because each covers what the others cannot:

| Tier | What | Size | Covers |
|------|------|------|--------|
| 1 | **Generated** at test run | KB | Every architecture uniformly, zero repo cost; the default for breadth |
| 2 | **`tiny-random-LlamaForCausalLM`**, committed | 5.7 MB | A **real tokenizer** — which generated weights can never provide |
| 3 | **`tiny-random-Gemma2ForCausalLM`**, committed | 48 MB | A **large-vocab** architecture (256k) and a real chat template |

Tiers 2 and 3 live in `artefacts/test-models/`, deliberately version-controlled
with the `*.safetensors` ignore rule negated for that directory. The ~54 MB is an
accepted trade: it buys real tokenizers and real chat templates offline, which is
precisely the surface `docs/fixes/006` proved we were getting wrong.

Tier 3 exists because Gemma2's 256k vocab makes the embedding matrix dominate —
no *published* "tiny" Gemma can be small, so committing one is the only way to
cover a large-vocab architecture without a download.

**Status: tiers 2 and 3 are landed and verified** — a smoke test loads the Llama
fixture through `CandleBackend` offline in 0.21s. Tier 1 (the generator) is not
started and is the remaining work here.

### Tier 1 — the generator

### Why

Spec 60 wants the agentic suite to run on every change, offline. `CandleBackend`
already supports this shape: `get_model_by_spec` → `load_from_local` reads
`config.json`, `tokenizer.json`, and `model.safetensors` from any directory, with
no HuggingFace involvement (that lives in `HuggingFaceCandleProvider`).

What candle's `.rs` model modules provide is the **architecture** — the layers
and forward pass. They contain no trained parameters, so weights are always
required. But weights do not have to be *downloaded*: they can be generated.

### Requirements

1. Provide a test-support way to construct a tiny model on disk: a synthetic
   `config.json` (e.g. 2 layers, hidden 64, 4 heads, vocab 512), random weights
   written as safetensors with the exact tensor names the loader expects, and a
   programmatically-built `tokenizer.json`.
2. Total size in the hundreds of KB, load time in milliseconds, no network.
3. Deterministic under a fixed seed (depends on C.3).
4. **Spike first** (decision 04). The synthetic `config.json` must satisfy
   candle's `llama::Config` schema and the safetensors must carry exactly the
   tensor names `VarBuilder` looks up. That is an assumption to prove before the
   approach is committed to.
5. This tier proves **structural** behaviour only — that tokens are produced,
   that streaming advances, that shapes are right. Random weights emit
   gibberish. Semantic behaviour needs a real model, which is why the
   `integration_tests` tier keeps one (SmolLM2-135M-Instruct).
6. Test-support code lives in the crate (behind the `testing` feature), not
   hand-rolled per test file — per the house rule that tests only use foundation
   capabilities.

---

## E. Tests

1. Every supported architecture has a load test. Where a real model is too large
   to be routine, a synthetic model of that architecture covers the loader.
2. `generate()` and `stream()` agree in structure for the same interaction — the
   assertion that would have caught docs/fixes/006.
3. Seeded sampling is reproducible: the same seed and prompt produce the same
   token sequence twice.
4. Sampling knobs demonstrably change behaviour — a test that passes whether or
   not `top_p` is wired is not a test of `top_p`.
5. An unsupported architecture fails with a clear error naming the detected
   architecture, and does not fall back.
6. A model with no chat template still loads and generates, taking the
   documented fallback.

---

## Definition of done

1. More than one architecture loads and generates; the supported set is explicit
   and documented.
2. `Custom(String)` is either functional or gone.
3. Prompts go through the model's chat template, shared by `generate()` and
   `stream()`.
4. Sampling runs through candle's `LogitsProcessor`, honouring every
   `ModelParams` knob, with a seed for reproducibility.
5. The agentic suite (spec 60 workstream D) can run against candle with no
   network access.

---

## F. Candle version bump (decision 07)

Bump `candle-core` / `candle-nn` / `candle-transformers` from **0.10.2** to
**0.11.0** (latest stable, released 2026-06-26).

1. Land this **before** workstreams A and B, not after: adding architectures and
   a chat-template renderer on top of 0.10 and then bumping would mix API
   churn with new behaviour, making a regression hard to attribute.
2. Candle takes breaking changes across minor versions; expect `VarBuilder`,
   model constructor, and `Cache` signatures to move. The existing Llama loader
   and `CandleStream` are the blast radius.
3. The tier-2 fixture smoke test is the gate: it must still load the Llama
   fixture offline after the bump.

---

## Definition of done

1. The local model answers a greeting coherently, with a `docs/fixes/` entry for
   the root cause.
2. A default app run shows no llama.cpp output; `llamacpp=debug` shows it.
3. Every wart in section C is closed or has a recorded decision not to.
4. ~90% coverage of critical agentic logic, measured, with mock + candle tiers.
5. The full suite runs offline and fast enough to run on every change.
6. llama.cpp bumped with the suite still green.
