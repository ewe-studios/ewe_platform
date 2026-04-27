# Progress - Foundation AI

_Last updated: 2026-04-27 (01-llamacpp-integration & 06-llama-server-testing marked 100%)_

## Overview

`07-foundation-ai` covers the `foundation_ai` backend crate and its supporting
infrastructure (storage, auth, providers) for unified AI model inference.
The spec was originally scoped to llama.cpp only; it has since expanded to
ten features spanning storage, auth, multiple inference providers, and
full OpenAI API compatibility (Chat Completions + Responses API).

## Feature Status

| #  | Feature | Status | Tasks | Completion |
|----|---------|--------|-------|------------|
| 0a | [foundation-db](./features/00a-foundation-db/feature.md) | ✅ Complete | 32 / 32 | 100% |
| 0b | [auth-infrastructure](./features/00b-auth-infrastructure/feature.md) | ⬜ Pending | 0 / 30 | 0% |
| 0c | [openai-provider](./features/00c-openai-provider/feature.md) | ✅ Complete | 45 / 45 | 100% |
| 0d | [state-store-streaming](./features/00d-state-store-streaming/feature.md) | ⬜ Pending | 0 / 12 | 0% |
| 0e1 | [http-client-connection-pool](./features/00e-http-client-connection-pool/feature.md) | ⬜ Pending | 0 / ? | 0% |
| 0e2 | [state-store-query-filtering](./features/00e-state-store-query-filtering/feature.md) | ⬜ Pending | 0 / ? | 0% |
| 0f | [test-server-keepalive](./features/00f-test-server-keepalive/feature.md) | ⬜ Pending | 0 / ? | 0% |
| 0g | [openai-provider-enhancements](./features/00g-openai-provider-enhancements/feature.md) | 🔄 In Progress | 2 / 54 | ~4% |
| 1  | [llamacpp-integration](./features/01-llamacpp-integration/feature.md) | ✅ Complete | 27 / 27 | 100% |
| 2  | [huggingface-gguf-provider](./features/02-huggingface-provider/feature.md) | ✅ Complete | 5 / 5 | 100% |
| 3  | [candle-integration](./features/03-candle-integration/feature.md) | ✅ Complete | 18 / 18 | 100% |
| 4  | [tool-calling-formatter](./features/04-tool-calling-formatter/feature.md) | ⬜ Pending | 0 / ? | 0% |
| 5  | [agentic-coding-reference](./features/05-agentic-coding-reference/feature.md) | ⬜ Pending | 0 / ? | 0% |
| 06 | [llama-server-testing](./features/06-llama-server-testing/feature.md) | ✅ Complete | 16 / 16 | 100% |
| 07 | [anthropic-provider](./features/07-anthropic-provider/feature.md) | ⬜ Pending | 0 / ? | 0% |

**Totals:** 143 / ~270 tasks complete (~53%). 7 features complete, 2 in progress, 6 pending.

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

### 00g OpenAI Provider Enhancements (Partial)
- `ResponsesProvider` implemented for OpenAI Responses API (`/v1/responses`)
- SSE streaming for Responses API with proper error propagation
- 2 llama-server integration tests for Responses API (generate + stream)
- Remaining: output format control, advanced sampling params, multimodal, logprobs, tool_choice, etc.

## What's Next

### Immediate (finish in-progress features)

**00g openai-provider-enhancements — 52 tasks remaining**
- Output format control (text, json_object, json_schema)
- Advanced sampling params (presence_penalty, frequency_penalty, seed)
- Multimodal input (image_url, image_content blocks)
- Logprobs support (top_logprobs, per-token breakdown)
- tool_choice (auto, required, specific function)
- Run full verification gate: `cargo check/clippy/test --package foundation_ai`

### Dependency-ordered queue after current work

1. **00d state-store-streaming** (12 tasks) — fixes all state stores to use
   `run_future_iter` for proper row streaming. 00a is done, so this is
   unblocked; should land before 00b so auth persistence streams cleanly.
2. **00b auth-infrastructure** (30 tasks) — JWT, OAuth 2.0 (PKCE S256),
   credential storage via foundation_db, auth state machine, 2FA.
   Unblocked — 00a is complete.
3. **04 tool-calling-formatter** (18 tasks) — plugin-based ToolFormatter for
   OpenAI, Anthropic, llama.cpp, open-source tool calling formats.
   Depends on 00c (complete).
4. **07 anthropic-provider** (32 tasks) — Anthropic Messages API provider with
   streaming, tool use, extended thinking, multimodal.
   Depends on 00c (complete) and 00g (in progress).
5. **05 agentic-coding-reference** (reference only) — documentary analysis of
   pi-mono and hermes-agent agentic coding patterns. No code tasks.
   Depends on 04.

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
    ├── 00b-auth-infrastructure/   (0%   ⬜)
    ├── 00c-openai-provider/       (100% ✅)
    ├── 00d-state-store-streaming/ (0%   ⬜)
    ├── 00e-http-client-connection-pool/
    ├── 00e-state-store-query-filtering/
    ├── 00f-test-server-keepalive/
    ├── 00g-openai-provider-enhancements/ (~4% 🔄)
    ├── 01-llamacpp-integration/   (100% ✅)
    ├── 02-huggingface-provider/   (100% ✅)
    ├── 03-candle-integration/     (100% ✅)
    ├── 04-tool-calling-formatter/ (0%   ⬜)
    ├── 05-agentic-coding-reference/(0%   ⬜)
    ├── 06-llama-server-testing/   (100% ✅)
    └── 07-anthropic-provider/     (0%   ⬜)
```

Each feature directory contains its own `PROGRESS.md` with the detailed
task breakdown.
