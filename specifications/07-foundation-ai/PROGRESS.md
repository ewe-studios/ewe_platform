# Progress - Foundation AI

_Last updated: 2026-04-25 (after Candle integration complete)_

## Overview

`07-foundation-ai` covers the `foundation_ai` backend crate and its supporting
infrastructure (storage, auth, providers) for unified AI model inference.
The spec was originally scoped to llama.cpp only; it has since expanded to
seven features spanning storage, auth, and multiple inference providers.

## Feature Status

| #  | Feature | Status | Tasks | Completion |
|----|---------|--------|-------|------------|
| 0a | [foundation-db](./features/00a-foundation-db/feature.md) | ✅ Complete | 32 / 32 | 100% |
| 0b | [auth-infrastructure](./features/00b-auth-infrastructure/feature.md) | ⬜ Pending | 0 / 30 | 0% |
| 0c | [openai-provider](./features/00c-openai-provider/feature.md) | ⬜ Pending | 0 / 45 | 0% |
| 0d | [state-store-streaming](./features/00d-state-store-streaming/feature.md) | ⬜ Pending | 0 / 12 | 0% |
| 1  | [llamacpp-integration](./features/01-llamacpp-integration/feature.md) | 🔄 In Progress | 18 / 27 | 67% |
| 2  | [huggingface-gguf-provider](./features/02-huggingface-provider/feature.md) | ✅ Complete | 5 / 5 | 100% |
| 3  | [candle-integration](./features/03-candle-integration/feature.md) | ✅ Complete | 18 / 18 | 100% |

**Totals:** 68 / 174 tasks complete (~39%). 2 features complete, 1 in progress, 4 pending.

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

### 01 llama.cpp Integration (67%)
- Type extensions (`ModelOutput::Embedding`, `ChatMessage`, `LlamaConfig`,
  `SplitMode`, `KVCacheType`, `llama` on `ModelConfig`) — complete
- Error type extensions for llama.cpp errors — complete
- Sampler chain builder (`build_sampler_chain`) — complete + tested
- `LlamaBackendConfig` builder with defaults — complete
- `LlamaModels` struct with interior mutability — complete
- `Model::generate()` with tokenize/batch/decode loop, EOS/stop token
  detection, chat template application from `ModelInteraction` — complete
- Embedding generation via `ctx.encode()` + `embeddings_seq_ith()` — complete
- Recent `86c85840` rewired `backends/foundation_ai/src/backends/llamacpp.rs`
  (+264 lines) and touched infrastructure context params

## What's Next

### Immediate (finish in-progress features)

**01 llama.cpp Integration — 9 tasks remaining**
- Implement `Model::stream()` returning `LlamaCppStream`
- Create `LlamaCppStream` struct implementing `StreamIterator`
- Enable the `#[ignore]`d integration tests (model load, generation, chat,
  embeddings) once a test GGUF fixture is available
- Run full verification gate: `cargo check/clippy/test/fmt --package foundation_ai`

### Dependency-ordered queue after current work

1. **00d state-store-streaming** (12 tasks) — fixes all state stores to use
   `run_future_iter` for proper row streaming. 00a is done, so this is
   unblocked; should land before 00b so auth persistence streams cleanly.
2. **00b auth-infrastructure** (30 tasks) — JWT, OAuth 2.0 (PKCE S256),
   credential storage via foundation_db, auth state machine, 2FA.
   Unblocked — 00a is complete.
3. **00c openai-provider** (45 tasks) — OpenAI-compatible HTTP provider
   (OpenAI, llama.cpp server, vLLM, Ollama) using `simple_http` +
   `event_source` for SSE. Depends on 00b for credential handling.
4. **02 huggingface-gguf-provider** (5 tasks) — `HuggingFaceGGUFProvider`: HF Hub
   GGUF model discovery and download via `hf-hub`. Depends on 01. ✅ Complete.
5. **03 candle-integration** (18 tasks) — alternative pure-Rust inference
   backend via Candle with safetensors. Depends on 01. ✅ Complete.

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
    ├── 00c-openai-provider/       (0%   ⬜)
    ├── 00d-state-store-streaming/ (0%   ⬜)
    ├── 01-llamacpp-integration/   (67%  🔄)
    ├── 02-huggingface-provider/   (100% ✅) [HuggingFaceGGUFProvider]
    └── 03-candle-integration/     (100% ✅)
```

Each feature directory contains its own `PROGRESS.md` with the detailed
task breakdown.
