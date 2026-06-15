# Progress - Foundation AI

_Last updated: 2026-06-15 (all 15 features complete — spec moved to completed/)_

## Overview

`07-foundation-ai` covers the `foundation_ai` backend crate and its supporting
infrastructure (storage, auth, providers) for unified AI model inference.
The spec was originally scoped to llama.cpp only; it has since expanded to
fifteen features spanning storage, auth, multiple inference providers, tool
calling formatting, and full OpenAI API compatibility (Chat Completions +
Responses API).

All 15 features are complete.

## Feature Status

| #  | Feature | Status | Tasks | Completion |
|----|---------|--------|-------|------------|
| 0a | [foundation-db](./features/00a-foundation-db/feature.md) | ✅ Complete | 32 / 32 | 100% |
| 0b | [auth-infrastructure](./features/00b-auth-infrastructure/feature.md) | ✅ Complete | 30 / 30 | 100% |
| 0c | [openai-provider](./features/00c-openai-provider/feature.md) | ✅ Complete | 45 / 45 | 100% |
| 0d | [state-store-streaming](./features/00d-state-store-streaming/feature.md) | ✅ Complete | 12 / 12 | 100% |
| 0e1 | [http-client-connection-pool](./features/00e-http-client-connection-pool/feature.md) | ✅ Complete | — | 100% |
| 0e2 | [state-store-query-filtering](./features/00e-state-store-query-filtering/feature.md) | ✅ Complete | — | 100% |
| 0f | [test-server-keepalive](./features/00f-test-server-keepalive/feature.md) | ✅ Complete | — | 100% |
| 0g | [openai-provider-enhancements](./features/00g-openai-provider-enhancements/feature.md) | ✅ Complete | 64 / 64 | 100% |
| 1  | [llamacpp-integration](./features/01-llamacpp-integration/feature.md) | ✅ Complete | 27 / 27 | 100% |
| 2  | [huggingface-gguf-provider](./features/02-huggingface-provider/feature.md) | ✅ Complete | 5 / 5 | 100% |
| 3  | [candle-integration](./features/03-candle-integration/feature.md) | ✅ Complete | 18 / 18 | 100% |
| 4  | [tool-calling-formatter](./features/04-tool-calling-formatter/feature.md) | ✅ Complete | 8 / 8 | 100% |
| 5  | [agentic-coding-reference](./features/05-agentic-coding-reference/feature.md) | ✅ Complete | documentary | 100% |
| 06 | [llama-server-testing](./features/06-llama-server-testing/feature.md) | ✅ Complete | 16 / 16 | 100% |
| 07 | [anthropic-provider](./features/07-anthropic-provider/feature.md) | ✅ Complete | 44 / 44 | 100% |
| 08 | [tool-arguments-refactor](./features/08-tool-arguments-refactor/feature.md) | ✅ Complete | 4 / 4 | 100% |

**Totals:** All features complete. ~285 tasks total.

Status key: ⬜ Pending 🔄 In Progress ✅ Complete

## What's Done

### 00a Foundation DB (100% ✅)
- All storage backends compile and dispatch uniformly through `StorageProvider`
  (KeyValueStore, QueryStore, RateLimiterStore, BlobStore)
- In-memory, Turso, libsql, JSON file, D1, R2 backends
- Encryption integration for sensitive columns
- Cleanup operations + integration tests
- `cargo test --package foundation_db` — 70 tests passing
- `cargo clippy --package foundation_db -- -D warnings` — zero warnings
- `foundation_auth` now uses `foundation_db::StorageProvider` for credential
  storage through a single `CredentialStorage` wrapper (the old
  `TursoCredentialStore` / `MemoryCredentialStore` split was useless — both
  wrappers were identical since `StorageProvider` already selects the backend)
- Local auth tests run against a real SQLite file via the Turso provider
- `StorageProvider::new` now auto-calls `init_schema()` for Turso/libsql

### 02 HuggingFace GGUF Provider (100% ✅)
- `HuggingFaceGGUFProvider` with HF Hub GGUF model discovery and download

### 03 Candle Integration (100% ✅)
- `CandleBackend` enum (CPU/CUDA/Metal) implementing `ModelProvider`
- `CandleBackendConfig` + `HuggingFaceCandleConfig` with builder pattern, `AuthProvider` impl, manual `Clone`
- `AuthProvider` trait on all provider configs — `create()` no longer takes credential param
- `HuggingFaceCandleProvider` wrapper using `foundation_deployment::providers::huggingface` for safetensors download
- `CandleModels` struct with interior mutability, architecture dispatch (LLaMA family)
- Text generation, streaming (`CandleStream`), embeddings, chat template application
- `sample_token` handles variable logits ranks (1D/2D/3D)
- 15 unit tests passing (`candle_backend.rs`), 3 integration tests (`huggingface_candle_provider.rs`)
- Bug fix: `repository.rs` `Stream::Next` vs `Stream::Done` body extraction
- All tests run with `--profile uat` (LLVM backend; cranelift fails with `pulp` inline asm)

### 00c OpenAI Provider (100% ✅)
- `OpenAIProvider` implementing `ModelProvider` trait — connects to OpenAI, llama.cpp server, vLLM, Ollama, OpenRouter
- `OpenAIConfig` with builder pattern (base_url, timeout, retries, proxy, streaming)
- Chat completions via `/v1/chat/completions` — request/response types, message parsing
- Embeddings via `/v1/embeddings` — `generate_embeddings()` method on `OpenAIModel`
- Model discovery via `/v1/models` — list models, filter by capability, cache with TTL
- SSE streaming via `OpenAIStream` + `ReconnectingEventSourceTask` — token-by-token, tool call delta accumulation
- Retry with exponential backoff for 429/5xx — `Retry-After` header parsing
- Error mapping: OpenAI JSON errors → `GenerationError` variants
- Usage tracking: prompt/completion/total tokens from API responses
- `AuthProvider` trait on `OpenAIConfig` — `create()` without credential param
- SSE parse failures return `Stream::Next` with error detail instead of silent `Ignore`
- 20 unit tests + 3 integration tests (mock) passing
- 5 llama-server integration tests passing (real server, gated behind `#[ignore]`)

### 01 llama.cpp Integration (100% ✅)
- Type extensions (`ModelOutput::Embedding`, `ChatMessage`, `LlamaConfig`,
  `SplitMode`, `KVCacheType`, `llama` on `ModelConfig`) — complete
- Error type extensions for llama.cpp errors — complete
- Sampler chain builder (`build_sampler_chain`) — complete + tested
- `LlamaBackendConfig` builder with defaults — complete
- `LlamaModels` struct with interior mutability — complete
- `Model::generate()` with tokenize/batch/decode loop, EOS/stop token
  detection, chat template application from `ModelInteraction` — complete
- `Model::stream()` returning `LlamaCppStream` implementing `StreamIterator` — complete
- Embedding generation via `ctx.encode()` + `embeddings_seq_ith()` — complete
- 7 integration tests passing (`llamacpp_integration.rs`)
- Hardware acceleration: CUDA, Metal, Vulkan offloading support via features

### 06 llama-server Testing Infrastructure (100% ✅)
- `build.rs` conditionally builds llama-server via CMake when `LLAMA_SERVER_BUILD=1`
- mise tasks: build, clean, version, start, stop, status, test-model:download/path
- Test runners: test:llama-server, test:llama-server:chat, test:llama-server:responses
- 5 Chat Completions integration tests against real llama-server (all passing)
- 2 Responses API integration tests against real llama-server (all passing)
- Centralized model path config in `mise.toml` `[env]` section
- `multi` feature gating fixed: no longer forced on by default via `foundation_deployment`

### 07 Anthropic Messages Provider (100% ✅)
- `AnthropicMessagesProvider` implementing `ModelProvider` trait — connects to Anthropic's `/v1/messages` endpoint
- `AnthropicConfig` with builder pattern (base_url, api_version, timeout_secs, max_retries, proxy_url, streaming, auth)
- Native Anthropic protocol: `x-api-key` + `anthropic-version` headers (no Bearer token)
- `MessagesRequest` with all fields: model, system (string or blocks), messages, max_tokens, temperature, top_p, top_k, stream, stop_sequences, tools, tool_choice, thinking config
- `AnthropicContentBlock`: tagged serde enum with Text, Image, ToolUse, ToolResult, Thinking, RedactedThinking
- `AnthropicToolChoice`: Auto, Any, Tool { name } variants
- SSE streaming: `StreamEvent` enum with named events (message_start, content_block_start/delta/stop, message_delta/stop, ping)
- `AnthropicDelta`: TextDelta, ThinkingDelta, InputJsonDelta
- `AnthropicModel` implementing `Model` trait with generate() and stream()
- `AnthropicStream` iterator accumulating text, thinking, and tool call arguments from SSE events
- `build_anthropic_request`: maps `ModelInteraction` → `MessagesRequest` with proper content block conversion
- Stop reason mapping: end_turn→Stop, stop_sequence→Stop, max_tokens→Length, tool_use→ToolUse
- Error handling: parse_anthropic_error, format_http_error, retry with exponential backoff, Retry-After header parsing
- `parse_response` returns `Vec<Messages>` (one per content block)
- `build_final_messages` emits thinking, text, and tool calls as separate messages
- 5 mock integration tests passing (TestHttpServer: generate, streaming text, tool calls, thinking, multimodal)
- 5 llama-server integration tests (#[ignore]-gated): generate, streaming, multi-turn, max_tokens, resolve

### 00g OpenAI Provider Enhancements (100% ✅)
- `ResponsesProvider` / `ResponsesModel` implementing `ModelProvider` for `/v1/responses`
  (reasoning models: o1, o3, o1-pro)
- `ResponseRequest`, `ResponseInput`, `ResponseInputItem`, `ResponseOutputItem`,
  `ResponseEvent` — full request/response/streaming types
- SSE streaming for Responses API with named event parsing
- `OutputFormat` (text, json_object, json_schema) wired through `build_chat_request`
- Advanced sampling: `frequency_penalty`, `presence_penalty`, `logit_bias` in `ModelParams`
- `ToolChoice` (auto, none, required, function) wired through `build_chat_request`
- Multimodal: `OpenAIMessageContent` (text/parts), `OpenAIContentPart`, `OpenAIImageUrlObject`
- Logprobs: `OpenAILogProbs`, `ContentLogProb`, `TopLogProbEntry`, `RefusalLogProb`
  → `GenerationMetadata::LogProbs`
- `GenerationMetadata` enum: LogProbs, SystemFingerprint, Timing, RefusalReason
- `metadata: Option<Vec<GenerationMetadata>>` on `Messages::Assistant`
- Refusal: parsed from `OpenAIMessage.refusal` → `GenerationMetadata::RefusalReason`
- `StopReason::Message(String)` for unknown/custom finish reasons
- 2 llama-server integration tests for Responses API (generate + stream)
- 10 unit tests for Responses API types, 20+ unit tests in openai_provider.rs

## What's Next

No remaining features. This spec is complete.

### Parallel Cleanup Effort

**14-zero-warnings-workspace** — Workspace-wide lint cleanup (~1,500 warnings).
This is a separate spec (`specifications/14-zero-warnings-workspace/`) running
in parallel. When complete, `foundation_ai` (~224 warnings) and `foundation_core`
(~601 warnings) will be clippy-clean. See that spec for details.

## Iron Laws (spec-wide)

1. No tokio, no async-trait in `foundation_db` / `foundation_auth` — Valtron only
2. Turso sync backend (no libsql for the async-sensitive paths)
3. Valtron-only async (`TaskIterator` / `StreamIterator`, no `.await`)
4. Zero warnings, zero `#[allow(...)]` suppression
5. Errors: `derive_more::From` + manual `Display`, no `thiserror`

## File Structure

```
specifications/07-foundation-ai/
├── PROGRESS.md            # This file (spec-level)
├── requirements.md        # Full spec, iron laws, feature index
├── LEARNINGS.md
├── VALTRON_CAPABILITIES.md
├── start.md
└── features/
    ├── 00a-foundation-db/         (100% ✅)
    ├── 00b-auth-infrastructure/   (100% ✅)
    ├── 00c-openai-provider/       (100% ✅)
    ├── 00d-state-store-streaming/ (100% ✅)
    ├── 00e-http-client-connection-pool/ (100% ✅)
    ├── 00e-state-store-query-filtering/ (100% ✅)
    ├── 00f-test-server-keepalive/ (100% ✅)
    ├── 00g-openai-provider-enhancements/ (100% ✅)
    ├── 01-llamacpp-integration/   (100% ✅)
    ├── 02-huggingface-provider/   (100% ✅)
    ├── 03-candle-integration/     (100% ✅)
    ├── 04-tool-calling-formatter/ (100% ✅)
    ├── 05-agentic-coding-reference/ (✅ documentary)
    ├── 06-llama-server-testing/   (100% ✅)
    ├── 07-anthropic-provider/     (100% ✅)
    └── 08-tool-arguments-refactor/ (100% ✅)
```

Each feature directory contains its own `PROGRESS.md` with the detailed
task breakdown.
