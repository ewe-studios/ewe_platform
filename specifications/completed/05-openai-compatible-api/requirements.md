---
description: "Create an OpenAI-compatible API module in foundation_ai that supports both Chat Completions and Responses API (reasoning models), using foundation_core's simple_http client for HTTP communication."
status: "retired"
priority: "high"
created: 2026-03-08
retired: 2026-04-26
retired_reason: "Superseded by specifications/07-foundation-ai/features/00c-openai-provider (Chat Completions) and features/00g-openai-provider-enhancements (Responses API + full API surface)"
author: "Main Agent"
metadata:
  version: "1.0"
  last_updated: 2026-03-08
  estimated_effort: "large"
  tags:
    - openai-api
    - chat-completions
    - responses-api
    - reasoning-models
    - streaming
  skills:
    - specifications-management
    - rust-patterns
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: false
builds_on: "specifications/02-build-http-client"
related_specs:
  - "specifications/01-fix-rust-lints-checks-styling"
  - "specifications/03-wasm-friendly-sync-primitives"
  - "specifications/07-foundation-ai"
features:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# ⚠️ This specification has been RETIRED

**Retired:** 2026-04-26

**Replaced by:** `specifications/07-foundation-ai/features/`
- **00c-openai-provider** — Chat Completions API, streaming, retry, embeddings (45/45 tasks, 100% complete)
- **00g-openai-provider-enhancements** — Responses API, output format, sampling params, tool_choice, multimodal, logprobs (pending)

## Why

This spec was created as a standalone specification for OpenAI-compatible API
types and client implementation. The functionality was subsequently implemented
as part of the broader `07-foundation-ai` specification under the existing
OpenAI provider (`openai_provider.rs`). The remaining gaps (Responses API,
structured output, advanced parameters, etc.) have been captured as
`00g-openai-provider-enhancements` within spec 07.

## Where to look instead

| This spec concept | → Go to |
|---|---|
| Chat Completions types | `07-foundation-ai/features/00c-openai-provider/feature.md` — implemented in `backends/foundation_ai/src/backends/openai_provider.rs` |
| Chat Completions client | `07-foundation-ai/features/00c-openai-provider/feature.md` — implemented in `backends/foundation_ai/src/backends/openai_provider.rs` |
| Streaming support | `07-foundation-ai/features/00c-openai-provider/feature.md` — `OpenAIStream` in `openai_provider.rs` |
| Responses API | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 5 |
| Integration tests | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 5 |
| Error types | `07-foundation-ai/features/00c-openai-provider/feature.md` — `OpenAIError` in `openai_provider.rs` |
| Output format (JSON mode) | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 1 |
| Advanced sampling params | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 2 |
| Tool choice | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 3 |
| Multimodal content | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 4 |
| Logprobs | `07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` — Task Group 6 |

## What to do

- **Do not implement features from this spec.** All features listed below have been moved to spec 07.
- **Agents looking here:** Navigate to `specifications/07-foundation-ai/features/` for the active specification.
- **This spec's files are kept for historical reference only.**

---

_Originally created: 2026-03-08_
_Retired: 2026-04-26 — superseded by 07-foundation-ai_
