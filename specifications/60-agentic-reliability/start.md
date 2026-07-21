# 60: agentic + provider reliability

Prove the agentic stack correct, end to end, against real providers — with
coverage measured rather than assumed.

Supersedes and absorbs former spec 61 (candle-multi-model): the candle backend
is not a separate concern, it is the provider the agentic suite runs on.

## North star

**Every interaction, workflow and process in the agentic stack is exercised by a
test that runs offline, and code coverage tells us where we are still blind.**

Concretely:

- one committed fixture pair — **tiny-random-Llama** and **tiny-random-Gemma2** —
  drives *both* the candle and llama.cpp suites, so the two backends are tested
  on identical weights
- synthetic generated weights cover breadth (architectures we have no fixture
  for) where a real tokenizer is not what is under test
- coverage is reported, and the gaps it finds become tests
- ~90% of critical logic; trivial accessors may be excluded by review, flow and
  error paths may not

## Why this spec exists

The agent replied `(no response)` to every prompt. The chain of defects is in
`backends/foundation_ai/docs/fixes/006_root_cause_stream_never_creates_context.md`
— a stream that never created its context, a `Clone`+`Drop` use-after-free,
tokens never decoded back, a prompt that ignored the conversation, ~10 failure
paths reporting success, and the model reloaded from disk every turn.

All are fixed. The reason they survived is the point of this spec: **`run_turn`
had no test at any level, and nothing exercised `Model::stream`.** Every provider
suite called `generate()` — a different code path.

Reviewing candle for a test provider then found the same shapes there: one
architecture of ~40, a prompt built by hand with the tokenizer parameter unused,
hand-rolled sampling ignoring `top_p`/`repeat_penalty`, and no seed.

## Stages

Each stage unblocks the next. Nothing later starts before its predecessor is
green.

| Stage | Name | Unblocks | Why here |
|-------|------|----------|----------|
| **S0** | Coverage harness | everything | Without measurement, "90%" is a feeling. Must exist before we claim progress. |
| **S1** | Candle 0.11 bump | S2 | Land API churn alone, so later regressions are attributable. |
| **S2** | Model state into the model | S3, S6 | `forward`/`CandleStream` assume a Llama-shaped cache. Fix before a second architecture entrenches it. |
| **S3** | Sampling via `LogitsProcessor` | S5 | Cheap, and the **seed** is what makes every later assertion deterministic. |
| **S4** | GGUF fixture conversion | S5 | llama.cpp is GGUF-only; convert the committed fixtures so both backends test the same weights. |
| **S5** | Chat templates via minijinja | S6, S7 | Shared by `generate()` and `stream()`. The defect class of docs/fixes/006. |
| **S6** | Architecture coverage | S7 | Now safe: state is abstracted (S2) and prompting is correct (S5). |
| **S7** | The test matrix | — | Fill `test-matrix.md` to green, driven by S0's coverage report. |
| **S8** | Generated weights (tier 1) | — | Breadth for architectures with no fixture. Last: a convenience, not a blocker. |
| **S9** | Generation quality | — | Why the real model answers `"."`. Independent of the rest. |

## Test model tiers

| Tier | What | Size | Used for |
|------|------|------|----------|
| 1 | Generated at test run | KB | Architectures with no fixture; pure breadth |
| 2 | `tiny-random-LlamaForCausalLM` — safetensors **and** GGUF | 5.7 MB + GGUF | **Both** backends: real tokenizer, real loading |
| 3 | `tiny-random-Gemma2ForCausalLM` — safetensors **and** GGUF | 48 MB + GGUF | **Both** backends: 256k vocab, real chat template |
| 4 | SmolLM2-135M-Instruct / Gemma 4 E2B | 270 MB / 2.9 GB | `integration_tests` only: semantic quality (S9) |

Tiers 2 and 3 are the default. Candle-only concerns (architecture dispatch,
`VarBuilder`) use candle alone; everything testable on both runs on both.

## Progress

| Stage | Status | Notes |
|-------|--------|-------|
| S0 coverage harness | DONE | cargo-llvm-cov + llvm-tools installed; baseline captured |
| S1 candle 0.11 | DONE | bumped, zero breaking changes |
| S2 state into model | Not started | Decision 06 |
| S3 sampling + seed | Not started | Adopt `LogitsProcessor` |
| S4 GGUF fixtures | Not started | `tools/llama.cpp/convert_hf_to_gguf.py` is vendored |
| S5 chat templates | Not started | minijinja (decision 03) |
| S6 architectures | Not started | Decision 02 |
| S7 test matrix | Partial | 42/119 rows; agent_loop 21%->74%, session 70%->89%, candle 28%->72% |
| S8 generated weights | Not started | Spike required |
| S9 generation quality | DONE | conditional BOS (docs/fixes/007); model answers coherently |

## Decisions

| # | Decision | Status |
|---|----------|--------|
| 00 | Candle is a first-class provider and the in-process test provider | Resolved |
| 01 | `CandleArchitecture::Custom(String)` must not remain a hardcoded error | Resolved |
| 02 | First architecture cut: Llama, Qwen2, Qwen3, Mistral, Phi3, Gemma2/3 (Mamba/RWKV excluded — recurrent state) | Resolved |
| 03 | Chat templates render with **minijinja** — already a workspace dep (`foundation_packager`) | Resolved |
| 04 | Four model tiers; committed fixtures are the default and serve **both** backends | Resolved |
| 05 | Sampling seed lives on `ModelParams` so a caller can force determinism per request | Resolved |
| 06 | Per-architecture state moves **into** the model abstraction | Resolved |
| 07 | Bump candle 0.10.2 → 0.11.0, before the architecture work | Resolved |
| 08 | Coverage tool: **`cargo-llvm-cov`** (source-based, workspace-aware, lcov + html) | Resolved |
| 09 | Whether coverage gates CI or only reports | **Open** |
| 10 | Whether GGUF fixtures are committed or generated at test time from the safetensors | **Open** |

## Non-goals

- Rewriting the agent loop's state machine. Bugs get fixed; the design stands.
- Multimodal, embedding, vision, audio models. Text generation first.
- Replacing llama.cpp. Candle is the in-process path; llama.cpp is the
  GGUF/quantized production path. Both stay, and both get tested.
- Training or fine-tuning.

## Related

- `test-matrix.md` — every interaction, workflow and process to be covered
- `backends/foundation_ai/docs/fixes/006_root_cause_stream_never_creates_context.md`
- `backends/foundation_ai/docs/fixes/005_root_cause_ffi_pointers_not_send.md`
- `backends/foundation_ai/tests/agentic/integrations/session_turn.rs` — the seed of S7
- `artefacts/test-models/` — the committed fixtures
