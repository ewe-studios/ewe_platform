---
description: "Create an OpenAI-compatible API module in foundation_ai that supports both Chat Completions and Responses API (reasoning models), using foundation_core's simple_http client for HTTP communication."
status: "pending"
priority: "high"
created: 2026-03-08
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
features:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---

# Overview

This specification defines the implementation of an OpenAI-compatible API client module in `foundation_ai` that supports both the **Chat Completions API** (standard models) and the **Responses API** (reasoning models). The implementation leverages `foundation_core`'s `simple_http` client for all HTTP communication.

## Goals

- Implement OpenAI Chat Completions API compatibility (messages-based inference)
- Implement OpenAI Responses API compatibility (reasoning models with structured output)
- Support both streaming and non-streaming request patterns
- Use `foundation_core::simple_http::client::SimpleHttpClient` for HTTP transport
- Provide ergonomic Rust API with proper type safety
- Support all common parameters (temperature, max_tokens, top_p, stop tokens, etc.)
- Implement proper error handling with descriptive error types

## Implementation Location

- Primary implementation: `backends/foundation_ai/src/openai/`
- Types module: `backends/foundation_ai/src/openai/types.rs`
- Chat Completions: `backends/foundation_ai/src/openai/chat.rs`
- Responses API: `backends/foundation_ai/src/openai/responses.rs`
- Streaming support: `backends/foundation_ai/src/openai/stream.rs`
- Error types: `backends/foundation_ai/src/openai/errors.rs`

## Known Issues

None currently identified.

## Feature Index

Features are listed in dependency order. Each feature contains detailed requirements, tasks, and verification steps in its respective `feature.md` file.

### Features (6 total)

1. **[foundation](./features/00-foundation/feature.md)** ⬜
   - Description: Error types, HTTP client integration, base types for OpenAI compatibility
   - Dependencies: None
   - Status: Pending

2. **[chat-completions-types](./features/01-chat-completions-types/feature.md)** ⬜
   - Description: Chat Completions API request/response types, message structures
   - Dependencies: #0
   - Status: Pending

3. **[chat-completions-client](./features/02-chat-completions-client/feature.md)** ⬜
   - Description: Chat Completions API client implementation, non-streaming
   - Dependencies: #1
   - Status: Pending

4. **[streaming-support](./features/03-streaming-support/feature.md)** ⬜
   - Description: SSE streaming support for both APIs, stream parsers
   - Dependencies: #1
   - Status: Pending

5. **[responses-api](./features/04-responses-api/feature.md)** ⬜
   - Description: Responses API (reasoning models) types and client
   - Dependencies: #0, #3
   - Status: Pending

6. **[integration-tests](./features/05-integration-tests/feature.md)** ⬜
   - Description: End-to-end integration tests with mock servers
   - Dependencies: #2, #4
   - Status: Pending

---

# Requirements Conversation Summary

This specification was created after analyzing:

1. **llama.cpp OpenAI compatibility tests** - Provides real-world implementation examples for:
   - Chat Completions endpoint (`/chat/completions`)
   - Responses endpoint (`/responses`)
   - Streaming formats (SSE-based)
   - Error handling patterns

2. **OpenAI API patterns discovered:**

   **Chat Completions Request:**
   ```json
   {
     "model": "gpt-4.1",
     "messages": [
       {"role": "system", "content": "You are helpful"},
       {"role": "user", "content": "Hello"}
     ],
     "max_tokens": 1024,
     "temperature": 0.7,
     "stream": false
   }
   ```

   **Chat Completions Response:**
   ```json
   {
     "id": "cmpl-xxx",
     "object": "chat.completion",
     "created": 1234567890,
     "model": "gpt-4.1",
     "choices": [{
       "index": 0,
       "message": {"role": "assistant", "content": "Hello! How can I help?"},
       "finish_reason": "stop"
     }],
     "usage": {
       "prompt_tokens": 10,
       "completion_tokens": 8,
       "total_tokens": 18
     }
   }
   ```

   **Responses API Request:**
   ```json
   {
     "model": "o1-pro",
     "input": [
       {"role": "system", "content": "Think carefully"},
       {"role": "user", "content": "Solve this problem"}
     ],
     "max_output_tokens": 2048,
     "temperature": 1.0
   }
   ```

   **Responses API Events (streaming):**
   - `response.created` - Initial response object
   - `response.in_progress` - Reasoning in progress
   - `response.output_item.added` - New output item
   - `response.output_text.delta` - Text chunk
   - `response.completed` - Final response

# High-Level Architecture

```
backends/foundation_ai/src/openai/
├── mod.rs              # Public API exports
├── errors.rs           # Error types (ApiError, StreamingError)
├── types.rs            # Common types (Role, Content, FunctionCall, etc.)
├── client.rs           # OpenAIClient struct with HTTP integration
├── chat/
│   ├── mod.rs          # Chat Completions module
│   ├── types.rs        # ChatCompletionRequest, ChatCompletionResponse
│   └── stream.rs       # ChatCompletionStream for SSE parsing
└── responses/
    ├── mod.rs          # Responses API module
    ├── types.rs        # ResponseRequest, Response, OutputItem
    └── stream.rs       # ResponseStream for reasoning events
```

## HTTP Client Integration

The implementation uses `foundation_core::simple_http::client::SimpleHttpClient`:

```rust
use foundation_core::simple_http::client::SimpleHttpClient;

pub struct OpenAIClient {
    http_client: SimpleHttpClient,
    base_url: String,
    api_key: String,
}

impl OpenAIClient {
    pub fn new(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            http_client: SimpleHttpClient::from_system(),
            base_url: base_url.into(),
            api_key: api_key.into(),
        }
    }

    pub async fn chat_completions(&self, request: ChatCompletionRequest) -> Result<ChatCompletionResponse, ApiError> {
        // Uses foundation_core HTTP client
    }

    pub fn chat_completions_stream(&self, request: ChatCompletionRequest) -> Result<ChatCompletionStream, ApiError> {
        // Returns SSE stream iterator
    }
}
```

# Success Criteria (Spec-Wide)

This specification is considered complete when:

## Functionality
- All 6 features implemented and verified
- Chat Completions API supports all common parameters
- Responses API supports reasoning model patterns
- Streaming works correctly with SSE parsing
- Integration tests pass against llama.cpp server

## Code Quality
- Zero warnings from `cargo clippy -- -D warnings`
- `cargo fmt -- --check` passes
- All unit tests pass
- Integration tests demonstrate OpenAI library compatibility

## Documentation
- `LEARNINGS.md` captures design decisions
- `VERIFICATION.md` with all checks passing
- `REPORT.md` with implementation summary
- Module documentation updated

## Module References

Agents implementing features should read:
- `backends/foundation_core/src/wire/simple_http/client/client.rs` - HTTP client API
- `documentation/simple_http/doc.md` - HTTP patterns
- `.agents/stacks/rust.md` - Rust conventions

---

_Created: 2026-03-08_
_Structure: Feature-based (has_features: true)_
