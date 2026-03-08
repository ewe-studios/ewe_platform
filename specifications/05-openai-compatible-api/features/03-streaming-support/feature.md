---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/03-streaming-support"
this_file: "specifications/05-openai-compatible-api/features/03-streaming-support/feature.md"

status: pending
priority: high
created: 2026-03-08

depends_on:
  - 00-foundation
  - 01-chat-completions-types

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---


# Streaming Support Feature

## Overview

Implement Server-Sent Events (SSE) streaming support for both Chat Completions and Responses APIs, including stream parsers and async iterators.

## Dependencies

This feature depends on:
- `00-foundation` - Uses `ApiError`, base types
- `01-chat-completions-types` - Uses chunk types

This feature is required by:
- `02-chat-completions-client` - For streaming completions
- `04-responses-api` - For streaming responses
- `05-integration-tests` - For streaming tests

## Requirements

### SSE Stream Format

OpenAI uses SSE (Server-Sent Events) format:

```
data: {"id":"cmpl-xxx","choices":[{"delta":{"content":"Hello"}}]}

data: {"id":"cmpl-xxx","choices":[{"delta":{"content":" world"}}]}

data: [DONE]
```

### Stream Iterator Pattern

```rust
pub struct ChatCompletionStream {
    // Internal SSE parser state
    // Uses foundation_core event_source if available
}

impl ChatCompletionStream {
    /// Get the next chunk from the stream
    pub fn next(&mut self) -> Option<Result<ChatCompletionChunk, StreamingError>> {
        // Parse next SSE event
    }
}

// Alternative: Iterator-based approach
impl Iterator for ChatCompletionStream {
    type Item = Result<ChatCompletionChunk, StreamingError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Parse next SSE event
    }
}
```

### Chat Completion Chunk

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: usize,
    pub delta: ChoiceDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}
```

### Streaming Client Methods

```rust
impl OpenAIClient {
    /// Send a streaming chat completion request.
    ///
    /// Returns a stream iterator that yields completion chunks.
    pub fn chat_completions_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionStream, ApiError> {
        // Set stream: true in request
        // Send HTTP request
        // Return stream parser
    }
}
```

### SSE Parser Integration

Check if `foundation_core` has existing SSE support:
- `backends/foundation_core/src/wire/event_source/`

If available, reuse existing SSE parser.
If not, implement minimal SSE parser:

```rust
struct SseParser {
    buffer: String,
}

impl SseParser {
    fn feed(&mut self, data: &[u8]) -> Vec<SseEvent> { ... }
}

enum SseEvent {
    Data(String),
    Done,
}
```

## Tasks

1. **Check for existing SSE support**
   - [ ] Review `foundation_core/src/wire/event_source/`
   - [ ] Reuse existing SSE parser if available
   - [ ] Otherwise, implement minimal SSE parser

2. **Create streaming types**
   - [ ] Define `ChatCompletionChunk`
   - [ ] Define `ChoiceDelta`
   - [ ] Define `StreamingError` type

3. **Implement stream iterator**
   - [ ] Create `ChatCompletionStream` struct
   - [ ] Implement `Iterator` or custom `next()` method
   - [ ] Handle SSE parsing

4. **Add streaming client method**
   - [ ] Add `chat_completions_stream()` to `OpenAIClient`
   - [ ] Configure request with `stream: true`
   - [ ] Return stream parser

5. **Handle stream completion**
   - [ ] Detect `[DONE]` marker
   - [ ] Handle incomplete stream errors
   - [ ] Clean up resources

6. **Write streaming tests**
   - [ ] Test chunk reception
   - [ ] Test stream completion
   - [ ] Test error handling mid-stream

7. **Add documentation**
   - [ ] Document streaming API
   - [ ] Add usage examples
   - [ ] Document stream lifecycle

## Success Criteria

- [ ] All 7 tasks completed
- [ ] SSE parsing works correctly
- [ ] Stream iterator yields chunks
- [ ] Tests pass with mock SSE server

## Verification

```bash
cd backends/foundation_ai
cargo test openai::stream
```

---

_Created: 2026-03-08_
