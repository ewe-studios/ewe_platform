---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/02-huggingface-provider"
this_file: "specifications/07-foundation-ai/features/02-huggingface-provider/feature.md"

feature: "HuggingFace Model Provider - LlamaCpp Wrapper"
description: "Wrap LlamaBackends with HuggingFace model downloading capability using existing HFClient from foundation_deployment"
status: pending
priority: medium
depends_on:
  - "01-llamacpp-integration"
  - "foundation_deployment (huggingface module)"
estimated_effort: "small"
created: 2026-03-17
last_updated: 2026-04-22
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# HuggingFace Model Provider - LlamaCpp Wrapper

## Overview

Implement `HuggingFaceProvider` as a **thin wrapper** around `LlamaBackends` (from feature 01) that adds automatic model downloading from HuggingFace Hub.

This provider:
1. Reuses the existing `HFClient` from `foundation_deployment::providers::huggingface`
2. Downloads GGUF files on-demand when a model is requested
3. Delegates all inference to `LlamaBackends` after download
4. Caches downloaded models locally to avoid re-downloading

**Key insight:** This is NOT a separate `ModelProvider` implementation. It's `LlamaBackends` + download capability.

## Existing Infrastructure

### HFClient (Already Implemented)

`foundation_deployment::providers::huggingface` already provides:
- `HFClient` - HTTP client for HuggingFace Hub API
- `HFRepository` - Repository handle for file operations
- `repository::download_file()` - Download files with caching
- Token management, authentication, rate limiting

All implemented with `simple_http` (no tokio, no reqwest).

### LlamaBackends (Already Implemented)

`foundation_ai::backends::llamacpp` already provides:
- `LlamaBackends` enum (CPU/GPU/Metal) implementing `ModelProvider`
- `LlamaBackendConfig` with builder pattern
- `LlamaModels` implementing `Model` trait
- GGUF model loading and inference

## Requirements

1. **HuggingFaceProvider struct** - Wraps `LlamaBackends` + `HFClient`, implements `ModelProvider`
2. **HuggingFaceConfig** - Configuration: HF token, cache directory, preferred quantization, backend config
3. **Model Download on Request** - When `get_model()` is called, download GGUF if not cached, then delegate to `LlamaBackends`
4. **GGUF Filename Resolution** - Map `ModelId` (e.g., `TheBloke/Llama-2-7B-GGUF:q4_k_m`) to repo + filename
5. **Cache Management** - Store downloaded models in `~/.cache/huggingface` or config-specified directory

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    HuggingFaceProvider                       │
├─────────────────────────────────────────────────────────────┤
│  - HFClient (from foundation_deployment)                     │
│  - LlamaBackends (delegated inference)                       │
│  - CacheDirectory                                            │
├─────────────────────────────────────────────────────────────┤
│  get_model(ModelId) →                                        │
│    1. Parse ModelId → (repo_id, gguf_filename, quantization)│
│    2. Check cache for GGUF file                              │
│    3. If missing: HFClient.download(repo, file, cache_dir)   │
│    4. LlamaBackends.get_model(cached_path)                   │
│    5. Return LlamaModels                                     │
└─────────────────────────────────────────────────────────────┘
```

## ModelId Syntax

Support HuggingFace-specific model identification:

```
# Full specification with quantization
TheBloke/Llama-2-7B-GGUF:q4_k_m
mistralai/Mistral-7B-Instruct-v0.2-GGUF:q5_k_m

# Without quantization (use default or list available)
TheBloke/Llama-2-7B-GGUF

# With revision (branch/tag/commit)
TheBloke/Llama-2-7B-GGUF:main:q4_k_m
```

## Task Breakdown

### Task 1: Create HuggingFaceProvider struct

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

```rust
pub struct HuggingFaceProvider {
    hf_client: HFClient,
    llama_backends: LlamaBackends,
    cache_dir: PathBuf,
    default_quantization: Option<String>,
}

impl ModelProvider for HuggingFaceProvider {
    type Config = HuggingFaceConfig;
    type Model = LlamaModels; // Returns LlamaModels after download
    
    fn create(...) -> Result<Self, ModelProviderErrors> { ... }
    fn get_model(&self, model_id: ModelId) -> Result<Self::Model, ModelErrors> { ... }
    fn describe(&self) -> ModelProviderDescriptor { ... }
}
```

**Verification:** Compiles, implements `ModelProvider` trait.

### Task 2: Implement ModelId parsing for HuggingFace syntax

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

Parse `ModelId::Name("owner/repo:quantization", revision)` into:
- `repo_id: String` (e.g., `"TheBloke/Llama-2-7B-GGUF"`)
- `quantization: Option<String>` (e.g., `Some("q4_k_m")`)
- `revision: Option<String>` (e.g., `Some("main")`)

**Verification:** Unit tests for parsing various ModelId formats.

### Task 3: Implement download-on-request logic

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

In `get_model()`:
1. Check if GGUF exists in cache
2. If not, call `HFRepository::download_file()` with cache directory
3. On success, pass cached path to `LlamaBackends::get_model()`

**Verification:** Integration test downloads a small GGUF and loads it.

### Task 4: Add GGUF filename resolution

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

Given a quantization name (e.g., `q4_k_m`), construct the GGUF filename:
- `llama-2-7b.Q4_K_M.gguf`
- `mistral-7b-instruct-v0.2.Q5_K_M.gguf`

Or use `repository::list_tree()` to find matching files.

**Verification:** Unit tests for filename construction.

### Task 5: Add integration test

**File:** `backends/foundation_ai/tests/huggingface_provider.rs`

```rust
#[test]
#[ignore = "requires HF_TOKEN and downloads model"]
fn test_huggingface_provider_download_and_generate() {
    // 1. Create provider with HF token
    // 2. Request a small GGUF model (e.g., TinyLlama)
    // 3. Verify it downloads and caches
    // 4. Generate a short completion
    // 5. Verify model is cached (second call is instant)
}
```

**Verification:** Test passes when run with `HF_TOKEN`.

## Dependencies

**Crates:**
- `foundation_deployment` (feature: `huggingface`) - For `HFClient`, `HFRepository`
- `infrastructure_llama_cpp` - For `LlamaBackends`, `LlamaModels`

**No new external dependencies** - reuses existing `hf-hub` via `foundation_deployment`.

## Success Criteria

1. `HuggingFaceProvider` implements `ModelProvider` trait
2. Can download GGUF models from HuggingFace Hub
3. Caches downloaded models locally
4. Delegates inference to `LlamaBackends`
5. All existing `foundation_ai` tests pass
6. Integration test demonstrates end-to-end flow

## Related Features

- **01-llamacpp-integration** - Provides `LlamaBackends` that this wraps
- **00c-openai-provider** - Another `ModelProvider` implementation (HTTP-based)
- **foundation_deployment:huggingface** - Provides the underlying `HFClient`
