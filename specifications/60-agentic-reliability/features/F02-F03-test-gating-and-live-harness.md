---
feature: "F02–F03 — Test-feature gating + live-model SmolLM harness"
status: "in-progress"
priority: "high"
depends_on: ["F01"]
---

# F02 — Test-feature gating; F03 — live-model SmolLM harness

## F02 — Two gates, cleanly separated

| Gate | Runs | Needs |
|------|------|-------|
| default | offline: mocks, committed tiny fixtures (Llama/Gemma2 safetensors+GGUF), synthetic | nothing |
| `live-model-tests` | real model semantics via candle/llama.cpp on a small **downloaded** model (SmolLM2) | one-time download, cached |
| `external-service-tests` | anything hitting a **network service**: OpenRouter, OpenAI, Anthropic, HF Hub download, cloud HTTP generators | network + credentials |

Rules:
- The default `cargo test` must run fully offline and never hit the network.
- `live-model-tests` may download a model once (cached in `artefacts/models`),
  then run offline against it; self-skips when the model is absent AND no network.
- `external-service-tests` requires explicit credentials (`OPENROUTER_API_KEY`,
  etc.) and self-skips when absent — never fails a run for a missing key.
- `integration_tests` stays a back-compat alias for `live-model-tests`.

### Tasks
- [ ] Add `external-service-tests` feature to `foundation_ai`.
- [ ] Move OpenRouter/OpenAI/Anthropic/HF-download tests under it.
- [ ] Keep local-model (candle/llama.cpp on SmolLM/qwen) under `live-model-tests`.
- [ ] Each external test self-skips (with an eprintln) when its credential is unset.

## F03 — SmolLM pull-on-demand harness

We already download SmolLM2 via `foundation_testing::huggingface::TestHarness`
(`get_smollm_model`). Formalize it as **the** live-model model:

- One helper that returns the SmolLM2 path, pulling+caching it on first use
  (under `live-model-tests`), and returns `None` (skip) when absent and offline.
- Both backends use it: candle (safetensors) and llama.cpp (GGUF) — the same
  small model, so live-model runs are fast and consistent.
- The user has an OpenRouter token; SmolLM covers the *model* tier, OpenRouter
  covers the *service* tier — they are independent.

### Tasks
- [ ] `live_model::smollm_gguf()` / `smollm_safetensors()` helpers (pull+cache+skip).
- [ ] Point the existing gated tests at these helpers.
- [ ] A `live-model-tests` run with the model cached executes the full
      candle+llama.cpp semantic suite offline.

## Done when

Three clean tiers; default is fully offline; `live-model-tests` runs the SmolLM
semantic suite (pulling once); `external-service-tests` runs the OpenRouter/cloud
suite when credentials are present and skips cleanly otherwise.
