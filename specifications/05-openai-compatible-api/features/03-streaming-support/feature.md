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
        // Return stream parser wrapped in ChatCompletionStream
    }

    /// Send a streaming Responses API request.
    ///
    /// Returns a stream iterator that yields response events.
    pub fn responses_stream(
        &self,
        request: ResponseRequest,
    ) -> Result<ResponseStream, ApiError> {
        // Set stream: true in request
        // Send HTTP request to /v1/responses
        // Return stream parser wrapped in ResponseStream
    }
}
```

### SSE to Event Mapping

**Chat Completions SSE events**:
```
data: {"id":"cmpl-xxx","choices":[{"delta":{"role":"assistant"}}]}
data: {"id":"cmpl-xxx","choices":[{"delta":{"content":"Hello"}}]}
data: [DONE]
```

Maps to:
- `SseEvent::Message { data }` → Parse JSON → `ChatCompletionChunk`
- `data == "[DONE]"` → End of stream (return `None`)

**Responses API SSE events**:
```
data: {"type":"response.created","response":{"id":"resp_xxx",...}}
data: {"type":"response.output_text.delta","delta":"Hello"}
data: {"type":"response.completed","response":{...}}
```

Maps to:
- `SseEvent::Message { data }` → Parse JSON → `ResponseEvent`
- Event type determined by `type` field in JSON

### SSE Parser Integration

**VERIFIED**: `foundation_core` has existing SSE support:

- `foundation_core::event_source::SseParser<R: Read>` - Streaming SSE parser
- `foundation_core::event_source::SseEvent` - Parsed SSE event type
- `foundation_core::event_source::Event` - Alternative event type

The existing parser:
- Implements `Iterator<Item = Result<Event, EventSourceError>>`
- Handles W3C SSE specification parsing
- Supports `data:`, `event:`, `id:`, `retry:` fields
- Returns `Event::Message { id, event_type, data, retry }` or `Event::Comment`

**Integration approach**:
```rust
use foundation_core::event_source::{SseParser, Event as SseEvent};

pub struct ChatCompletionStream<R: Read> {
    parser: SseParser<R>,
}

impl<R: Read> Iterator for ChatCompletionStream<R> {
    type Item = Result<ChatCompletionChunk, StreamingError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.parser.next() {
            Some(Ok(SseEvent::Message { data, .. })) => {
                if data == "[DONE]" {
                    return None;  // End of stream
                }
                match serde_json::from_str::<ChatCompletionChunk>(&data) {
                    Ok(chunk) => Some(Ok(chunk)),
                    Err(e) => Some(Err(StreamingError::ParseError(e))),
                }
            }
            Some(Ok(SseEvent::Comment(_))) => {
                // Ignore comments, get next event
                self.next()
            }
            Some(Err(e)) => Some(Err(StreamingError::SseError(e))),
            None => None,  // EOF
        }
    }
}
```

## Tasks

1. **Create streaming types**
   - [ ] Define `ChatCompletionChunk` in `chat/types.rs`
   - [ ] Define `ChoiceDelta` in `chat/types.rs`
   - [ ] Define `StreamingError` enum in `errors.rs`
   - [ ] Define `ResponseEvent` in `responses/types.rs`

2. **Create ChatCompletionStream**
   - [ ] Create `backends/foundation_ai/src/openai/chat/stream.rs`
   - [ ] Wrap `foundation_core::SseParser` with type-safe iterator
   - [ ] Implement `Iterator` trait for `ChatCompletionStream`
   - [ ] Handle `[DONE]` marker detection
   - [ ] Parse JSON chunks from SSE data events

3. **Create ResponseStream**
   - [ ] Create `backends/foundation_ai/src/openai/responses/stream.rs`
   - [ ] Wrap `foundation_core::SseParser` for Responses API
   - [ ] Implement `Iterator` trait for `ResponseStream`
   - [ ] Parse `ResponseEvent` types from SSE data

4. **Add streaming client methods**
   - [ ] Add `chat_completions_stream()` to `OpenAIClient`
   - [ ] Add `responses_stream()` to `OpenAIClient`
   - [ ] Configure request with `stream: true`
   - [ ] Return appropriate stream type

5. **Handle stream errors**
   - [ ] Handle SSE parsing errors
   - [ ] Handle JSON deserialization errors
   - [ ] Handle incomplete stream errors
   - [ ] Handle HTTP connection errors

6. **Write streaming tests**
   - [ ] Test chunk reception
   - [ ] Test stream completion
   - [ ] Test error handling mid-stream
   - [ ] Test with mock SSE server

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
