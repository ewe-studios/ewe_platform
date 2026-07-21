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

## Current state (default offline suite)

- agentic/*: ~86% line mean; several at 90–100%.
- Committed: base error module (`errors/mod.rs`) Display+From now covered.
- **Remaining 0% / low:**

| File | Line | Path to cover |
|------|-----:|---------------|
| `models/generator.rs` | 0% | Offline: request/URL/header/body construction for each provider endpoint. Live (external-service): actual send against OpenRouter (OpenAI-compatible) + local llama-server. |
| `models/providers/mod.rs` | 0% | Provider registry glue — unit-testable. |
| `errors/llama.rs` | 0% | Display + every From conversion; construct source errors from `infrastructure_llama_cpp` (or via a real llama failure). |
| `errors/mod.rs` | ~done | DONE — base_errors_tests. |
| `toolbox/llama_server_harness.rs` | 0% | Exercised when llama-server tests run (external-service or live). |
| `openai_responses_provider.rs` | 8% | Offline: request build/parse. Live: OpenRouter/llama-server. |
| `huggingface_gguf_provider.rs` | 26% | Live-model: SmolLM pull + load. |
| `huggingface_candle_provider.rs` | 36% | Live-model: SmolLM safetensors pull + load. |
| `openai_provider.rs` | 60% | Offline: streaming SSE parse, tool-call extraction. Live: llama-server. |
| `anthropic_messages_provider.rs` | 73% | Offline: request/response mapping; Live: OpenRouter Anthropic. |
| `harness/agents.rs` | 66% | Offline: remaining session bridges + candle preset session. |
| `types/base_types.rs` | 38% | Large shared module — cover the agentic-relevant types; exclude unrelated by review. |

## Tasks

- [ ] `errors/llama.rs` — Display + all From conversions (offline unit).
- [ ] `models/generator.rs` — offline request-construction tests per endpoint.
- [ ] `models/generator.rs` — external-service send tests (OpenRouter + llama-server) under `external-service-tests`.
- [ ] `models/providers/mod.rs` — registry unit tests.
- [ ] `openai_provider.rs` / `openai_responses_provider.rs` — offline SSE/tool-call parse; live send.
- [ ] `anthropic_messages_provider.rs` — offline mapping; live OpenRouter.
- [ ] `huggingface_*_provider.rs` — live-model SmolLM load tests (F03).
- [ ] `harness/agents.rs` — cover the remaining session builders offline.
- [ ] Re-measure; list any excused regions with justification.

## Done when

Coverage report shows no critical file below the reviewed bar; 0% files
eliminated; external/live paths have real tests behind their gates.
