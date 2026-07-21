---
feature: "F04 — Drive coverage to 100% of critical logic"
status: "in-progress"
priority: "high"
depends_on: ["F01", "F02"]
---

# F04 — 100% coverage of critical logic

## Goal

100% coverage on everything that is critical logic (per review, trivial
accessors and unreachable cfg branches may be excused with a note). Nothing at
0%. External/live paths get real tests behind their feature gates.

## Measured — full feature list (2026-07-21)

Run: `LLAMA_TEST_MODEL_FILE=$PWD/artefacts/models/qwen2.5-0.5b-instruct-q4_k_m.gguf
cargo llvm-cov --profile uat -p foundation_ai --features "testing live-model-tests
external-service-tests" -- --test-threads=1` (HF_TOKEN set → live GGUF/candle ran;
no OpenRouter key → those self-skip). 1096 tests green.

**foundation_ai aggregate: lines 77.02%, regions 75.09%, functions 72.06%**
(baseline: lines 71.02% / regions 69.60% / functions 66.32% → +6pp lines). 1157
tests green, no failures.

Per-file lifts: `harness/agents` 66%→**99%**, `errors/llama` 68%→**100%**,
`models/generator` 51%→**80%**, `types/base_types` 38%→**77%**,
`openai_provider` 64%→69%, `huggingface_gguf` 55%→60%.

Gotcha: the `test_llama_server_*` tests need `LLAMA_TEST_MODEL_FILE` to point at
the real `artefacts/` (plural) path; a stale `artefact/` (singular) env value
makes them panic and llvm-cov aborts the report. Coverage can still be pulled
from the collected profdata with `cargo llvm-cov report --profile uat`.

### Remaining low files (all offline surface now covered)

The four files still <70% are dominated by **live HTTP / download execution
machinery and its error branches** — reachable only against a real server or via
fault injection (malformed responses, network/FS failures), not pure offline
tests. The offline surface (config, parse, builders, helpers) is covered.

| File | Line | What's left |
|------|-----:|-------------|
| `huggingface_candle_provider.rs` | ~50% | Live download/load/inference + Llama/Gemma arch branches + error paths |
| `huggingface_gguf_provider.rs` | ~60% | Live download/load error branches |
| `openai_responses_provider.rs` | ~67% | Streaming/send machinery, retry loops |
| `openai_provider.rs` | ~69% | Send/stream/retry/embeddings machinery |

Next lever for these: fault-injection tests via `TestHttpServer` returning error
statuses / malformed bodies to exercise retry + error-mapping branches.

### Done this batch

- `errors/llama.rs` 68% → **100%** — all From conversions + ApplyChatTemplate Display.
- `types/base_types.rs` 38% → **77%** — enum From<str/String>, is_context_overflow, json arg-type branches, LlamaConfig builders, quant to_filename_format (via HF tests).
- `openai_responses_provider.rs` — config builders, retry helpers, constructors.
- HF GGUF + candle — offline parse_model_id/quant-pattern/config-builder/Clone.
- Fixed 2 real test bugs: flaky `SessionId::from_name` equality (mint once); brittle 0.5B "Hello" substring assertion.

## Tasks

- [x] `errors/llama.rs` — Display + all From conversions (offline unit).
- [x] `openai_responses_provider.rs` — config builders, retry helpers, constructors.
- [x] `huggingface_*_provider.rs` — offline parse/config/quant + live-model SmolLM load (F03).
- [x] `types/base_types.rs` — enum conversions, is_context_overflow, formatter arg-types, LlamaConfig.
- [ ] `models/generator.rs` — external-service send tests (OpenRouter + llama-server) under `external-service-tests`.
- [ ] `huggingface_candle_provider.rs` — Gemma vs Llama arch branch + error paths (needs both fixtures live).
- [ ] `harness/agents.rs` — cover the remaining session builders offline.
- [ ] Re-measure; list any excused regions with justification.

## Done when

Coverage report shows no critical file below the reviewed bar; 0% files
eliminated; external/live paths have real tests behind their gates.
