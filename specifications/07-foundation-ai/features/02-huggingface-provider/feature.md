---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/02-huggingface-provider"
this_file: "specifications/07-foundation-ai/features/02-huggingface-provider/feature.md"

feature: "HuggingFace Model Provider - LlamaCpp Wrapper"
description: "Wrap LlamaBackends with HuggingFace model downloading capability using existing HFClient from foundation_deployment"
status: complete
priority: medium
depends_on:
  - "01-llamacpp-integration"
  - "foundation_deployment (huggingface module)"
estimated_effort: "small"
created: 2026-03-17
last_updated: 2026-04-22
author: "Main Agent"

tasks:
  completed: 5
  uncompleted: 0
  total: 5
  completion_percentage: 100%
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
2. **HuggingFaceConfig** - Configuration with builder pattern: API token, cache directory, preferred quantization, **llama.cpp backend variant (CPU/GPU/Metal)**
3. **Model Download on Request** - When `get_model()` is called, download GGUF if not cached, then delegate to `LlamaBackends`
4. **ModelId Resolution** - Map `ModelId` (e.g., `TheBloke/Llama-2-7B-GGUF:q4_k_m`) to repo + filename
5. **Cache Management** - Store downloaded models in `~/.cache/huggingface` or config-specified directory

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    HuggingFaceProvider                       │
├─────────────────────────────────────────────────────────────┤
│  - hf_client: HFClient (from foundation_deployment)          │
│  - llama_backend: LlamaBackends (CPU/GPU/Metal)              │
│  - cache_dir: PathBuf                                        │
│  - default_quantization: Option<String>                      │
├─────────────────────────────────────────────────────────────┤
│  get_model(ModelId) →                                        │
│    1. Parse ModelId → (repo_id, quantization, revision)      │
│    2. Check cache for GGUF file                              │
│    3. If missing: HFRepository.download_file()               │
│    4. self.llama_backend.get_model_by_spec(cached_path)      │
│    5. Return LlamaModels                                     │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Details

**`HuggingFaceProvider` struct:**
```rust
pub struct HuggingFaceProvider {
    hf_client: HFClient,
    llama_backend: LlamaBackends,  // CPU, GPU, or Metal
    cache_dir: PathBuf,
    default_quantization: Option<String>,
}
```

**`HuggingFaceConfig` with builder:**
```rust
let config = HuggingFaceConfig::builder()
    .token("hf_...")
    .cache_dir("~/.cache/huggingface")
    .default_quantization("q4_k_m")
    .llama_backend(LlamaBackends::LLamaGPU)  // Use GPU
    .n_gpu_layers(32)  // Offload 32 layers
    .build();
```

**ModelId syntax:**
```text
# Full specification with quantization
TheBloke/Llama-2-7B-GGUF:q4_k_m

# With revision and quantization
TheBloke/Llama-2-7B-GGUF:main:q5_k_m

# Without quantization (uses default)
TheBloke/Llama-2-7B-GGUF
```

## Task Breakdown

### Task 1: Create HuggingFaceProvider struct [COMPLETED]

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

**Implementation:**
```rust
pub struct HuggingFaceProvider {
    hf_client: HFClient,
    llama_backend: LlamaBackends,  // Configurable: CPU, GPU, or Metal
    cache_dir: PathBuf,
    default_quantization: Option<String>,
}

impl ModelProvider for HuggingFaceProvider {
    type Config = HuggingFaceConfig;
    type Model = LlamaModels;
    
    fn get_model(&self, model_id: ModelId) -> ModelProviderResult<Self::Model> {
        // 1. Parse ModelId
        // 2. Download or find cached GGUF
        // 3. Delegate to self.llama_backend.get_model_by_spec()
    }
}
```

**Verification:** Compiles, implements `ModelProvider` trait, passes clippy.

### Task 2: Implement ModelId parsing for HuggingFace syntax [COMPLETED]

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

**Implementation:** `parse_model_id()` method handles:
- `"owner/repo:quantization"` → `repo_id`, `quantization`, `revision="main"`
- `"owner/repo:revision:quantization"` → all three fields
- `"owner/repo"` → uses `default_quantization` from config

**Verification:** Unit test `test_huggingface_provider_parsing` verifies parsing.

### Task 3: Implement download-on-request logic [COMPLETED]

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

**Implementation:** `download_model()` method:
1. Check cache with `find_cached_file()`
2. If miss, use `HFRepository::repo_download_file()` 
3. Return cached path to `llama_backend.get_model_by_spec()`

**Verification:** Integration test `test_huggingface_provider_download_tiny` downloads TinyLlama.

### Task 4: Add GGUF filename resolution [COMPLETED]

**File:** `backends/foundation_ai/src/backends/huggingface_provider.rs`

**Implementation:**
- `quantization_to_filename_pattern()` converts `q4_k_m` → `*.Q4_K_M.gguf`
- `find_gguf_file_in_repo()` uses `repo_list_tree()` to find matching files
- Falls back to any `.gguf` file if exact quantization not found

**Verification:** Integrated into download flow.

### Task 5: Add integration test [COMPLETED]

**File:** `backends/foundation_ai/tests/huggingface_provider.rs`

**Tests:**
- `test_huggingface_provider_parsing` - Unit test for ModelId parsing
- `test_huggingface_provider_download_smollm` - Downloads SmolLM2-360M-Instruct GGUF using TestHarness
- `test_huggingface_provider_with_gpu` - Tests GPU backend with SmolLM2 model (ignored)
- `test_huggingface_provider_describe` - Verifies provider descriptor
- `test_huggingface_provider_with_smollm_inference` - Downloads SmolLM2-360M-Instruct GGUF model and performs text generation (ignored by default)

**Verification:** Tests compile and pass when run with `HF_TOKEN`.

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
