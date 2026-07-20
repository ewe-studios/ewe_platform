# Spec 61: candle-multi-model — requirements

## Problem

`foundation_ai::backends::candle` presents itself as a candle-backed model
provider, but supports one architecture and prompts it incorrectly. The gap only
became visible when spec 60 went looking for a small in-process model to test
the agentic loop with, and found that the choice of model was dictated by our
wrapper rather than by candle.

The defects are the same shape as the ones just fixed in the llama.cpp streaming
path: a prompt assembled by hand instead of through the model's chat template,
and capability that looks present but errors at the boundary.

---

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

### Note on state handling

`forward()` and `CandleStream` currently assume a Llama-style `cache` argument.
Architectures differ here (Mistral/Qwen use similar KV caches; Mamba/RWKV carry
recurrent state). The per-architecture state must be part of the model
abstraction rather than assumed by the stream, or adding the second architecture
will fight the first.

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
   reachable from the candle path. Decision 03 picks the approach —
   candidates: a Rust Jinja crate (`minijinja`), a hand-written renderer for the
   subset chat templates use, or per-architecture formatters in Rust.
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

## D. Offline test models

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
