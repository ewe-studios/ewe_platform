# 61: candle-multi-model

Make `foundation_ai`'s candle backend a real, multi-architecture provider —
one that can run the models candle already implements, prompts them correctly,
samples them properly, and can be driven in tests without downloading anything.

## North star

**Any causal-LM architecture candle supports, we can run — prompted with the
model's own chat template, sampled through candle's own sampler, and testable
offline with no weights download.**

## Why this spec exists

Spec 60 needed a small in-process model to exercise the agentic surface. Picking
one surfaced that the candle backend is a thin slice of what candle offers, and
that the slice has defects mirroring the ones just fixed in the llama.cpp path
(`foundation_ai/docs/fixes/006`).

The review below is the finding, not a plan.

### What candle offers vs what we expose

`candle-transformers` 0.10 ships **123 model modules**, of which ~40 are causal
LMs: llama, mistral, mixtral, qwen2, qwen3, qwen2_moe, qwen3_moe, phi, phi3,
gemma, gemma2, gemma3, falcon, mpt, olmo, olmo2, granite, glm4, chatglm,
deepseek2, bigcode, starcoder2, stable_lm, mamba, mamba2, rwkv v5/v6/v7,
persimmon, helium, recurrent_gemma, llama2_c, …

We expose exactly one:

```rust
match architecture {
    CandleArchitecture::Llama => build_llama_model(...),
    CandleArchitecture::Custom(name) => Err(ModelErrors::UnsupportedArchitecture(name)),
}
```

`Custom(String)` reads like an extension point but is a hardcoded error, so
`CandleArchitecture` is a two-variant enum where one variant works and the other
always fails. **Candle is not the constraint; our wrapper is.**

### Defects found in the existing slice

1. **No chat template is applied.** `build_prompt(_tokenizer: &Tokenizer, …)`
   hand-assembles a prompt string — note the tokenizer parameter is unused. This
   is the same class of defect as the llama.cpp streaming bug in docs/fixes/006:
   a modern instruct model prompted without its turn structure has nothing
   coherent to answer into.
2. **Sampling is hand-rolled and incomplete.** We implement `sample_token`,
   `argmax`, `sample_top_k`, `sample_from_logits` ourselves, supporting only
   temperature and top-k. Candle ships
   `candle_transformers::generation::LogitsProcessor` with `ArgMax`, `All`,
   `TopP`, `TopK`, `TopKThenTopP` and `GumbelSoftmax`. We use none of it.
3. **No `top_p`, no `repeat_penalty`.** `ModelParams` carries knobs the candle
   path silently ignores.
4. **No seed.** Sampling is unseeded, so candle generation is not reproducible —
   which blocks using it for deterministic tests.

## Scope

| # | Workstream | Outcome |
|---|-----------|---------|
| A | Architecture coverage | A real registry of supported architectures; `Custom` stops being a lie |
| B | Prompt construction | The model's own chat template applied, not a hand-rolled string |
| C | Sampling | Candle's `LogitsProcessor`; `top_p`, `repeat_penalty`, and a seed |
| D | Offline test models | Load and run without downloading weights |
| E | Tests | Every supported architecture loads; the seam is covered |

## Progress

| Workstream | Status | Notes |
|-----------|--------|-------|
| A: architecture coverage | Not started | Initial set is decision 02 |
| B: prompt construction | Not started | Blocked on decision 03 (template source) |
| C: sampling | Not started | Straightforward — adopt LogitsProcessor |
| D: offline test models | Not started | Synthetic weights; spike needed |
| E: tests | Not started | Feeds spec 60 workstream D |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 00 | Candle backend is a first-class provider, not a fallback — it is the in-process test provider for spec 60 | Resolved |
| 01 | `CandleArchitecture::Custom(String)` must not remain a hardcoded error — either implement dispatch or remove the variant | Resolved |
| 02 | Which architectures ship in the first cut | **Open** |
| 03 | Where chat templates come from — `tokenizer_config.json` needs a Jinja renderer; candle has none, llama.cpp's minja is C++ | **Open** |
| 04 | Whether synthetic random-weight models are the default test tier (needs the spike) | **Open** |
| 05 | Whether `ModelParams` gains a `seed` field or candle takes it from config | **Open** |

## Non-goals

- Multimodal (llava, pixtral, paligemma, qwen3_vl) — text-generation first.
- Embedding/vision/audio models — a separate surface.
- Replacing llama.cpp. Candle is the pure-Rust, in-process path; llama.cpp
  remains the GGUF/quantized production path. Both stay.
- Training or fine-tuning. Inference only.

## Related

- `specifications/60-agentic-reliability` — consumes this; its workstream D needs a fast offline provider
- `backends/foundation_ai/docs/fixes/006_root_cause_stream_never_creates_context.md` — the chat-template and silent-failure defects this spec mirrors in the candle path
- `backends/foundation_ai/src/backends/candle.rs` — 1074 lines, the surface under review
- `backends/foundation_ai/src/backends/huggingface_candle_provider.rs` — the downloading provider; `CandleBackend` itself loads from local paths
