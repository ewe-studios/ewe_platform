# Progress - Foundation AI

## Overview

The `07-foundation-ai` specification covers the `foundation_ai` backend crate for AI model inference, providing a unified abstraction layer over multiple model providers (Anthropic, OpenAI, Google, local llama.cpp, etc.).

## Features

### Feature 01: llamacpp-integration ✅ (Moved from 06-foundation-codegen)
- **Status:** Pending implementation
- **Description:** Complete integration of llama.cpp inference engine into foundation_ai backend
- **Location:** `specifications/07-foundation-ai/features/01-llamacpp-integration/feature.md`
- **Tasks:** 25 tasks across 10 task groups
- **Provides:**
  - Model Loading (local files, HuggingFace Hub)
  - Text Generation (autoregressive with configurable sampling)
  - Chat Completion (multi-turn with chat template support)
  - Streaming (token-by-token generation)
  - Hardware Acceleration (CUDA, Metal, Vulkan)
  - Embeddings (for RAG pipelines)

## Remaining Features

None planned — Feature 01 covers the complete llama.cpp integration.

## Summary

**Total Features:** 1 (llamacpp-integration)
**Total Tasks:** 25 (all pending)
**Implementation Status:** Not started

## Related Specifications

- `06-foundation-codegen` — Code generation and source scanning utilities
- `04-wasm-entrypoint-toolchain` — WASM binary generation (dependency)

## File Structure

```
specifications/07-foundation-ai/
├── PROGRESS.md           # This file
└── features/
    └── 01-llamacpp-integration/
        ├── feature.md    # Full specification
        └── start.md      # (optional starting point)
```
