# Starting Point: llama.cpp Foundation AI Integration

## Context

This feature integrates llama.cpp as a first-class inference backend in the `foundation_ai` crate. The integration enables local execution of GGUF-format models from HuggingFace and other sources.

## Current State

**Existing Infrastructure:**
- `infrastructure/llama-cpp/` — Safe Rust wrapper around llama.cpp (already implemented)
- `infrastructure/llama-bindings/` — Low-level FFI bindings via bindgen (already implemented)
- `backends/foundation_ai/` — Target crate for integration

**Existing Types:**
- `backends/foundation_ai/src/types/mod.rs` — `Model`, `ModelBackend`, `ModelProvider` traits
- `backends/foundation_ai/src/errors/mod.rs` — Error types to extend
- `backends/foundation_ai/src/backends/llamacpp.rs` — Stub implementation with `todo!()`

## Goal

Complete the integration by:
1. Implementing `LlamaBackends` enum and `ModelBackend` trait
2. Creating `LlamaCppModel` struct with full `Model` trait implementation
3. Adding generation, streaming, chat, and embeddings support
4. Extending error types and configuration
5. Adding comprehensive tests

## Implementation Plan

See `feature.md` for the complete specification with:
- 14 detailed requirements
- Architecture diagrams
- Code snippets for all implementations
- 25 tasks across 10 task groups
- Test strategy
- Verification commands

## Key Files to Modify

1. `backends/foundation_ai/src/errors/mod.rs` — Extend error types
2. `backends/foundation_ai/src/backends/llamacpp.rs` — Core implementation
3. `backends/foundation_ai/src/models/model_descriptors.rs` — Add `LlamaConfig`
4. `backends/foundation_ai/src/types/mod.rs` — Add `ChatMessage`
5. `backends/foundation_ai/Cargo.toml` — Add feature flags
6. `backends/foundation_ai/tests/llamacpp_tests.rs` — Integration tests

## Prerequisites

- Review `infrastructure/llama-cpp/src/lib.rs` for available APIs
- Review `documentary/llamacpp-integration.md` for llama.cpp concepts
- Understand `foundation_ai` type system in `backends/foundation_ai/src/types/mod.rs`

## Next Steps

1. Read the full `feature.md` specification
2. Begin with Task Group 1: Core Backend Implementation
3. Follow the phased implementation approach in `feature.md`
