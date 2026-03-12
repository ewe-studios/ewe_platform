---
purpose: "Design decisions and learnings for the OpenAI-compatible API specification"
version: "1.1"
created: 2026-03-08
last_updated: 2026-03-08
---

# Learnings

## Design Decisions

### 1. Reuse foundation_core HTTP Client

**Decision**: Use `foundation_core::simple_http::client::SimpleHttpClient` for all HTTP communication.

**Rationale**:
- Avoids duplicating HTTP client logic
- Leverages existing connection pooling, redirects, compression
- Consistent error handling across the platform
- Already tested and verified

### 2. Reuse foundation_core SSE Parser

**Decision**: Use `foundation_core::event_source::SseParser` for SSE streaming.

**Rationale**:
- Already implements W3C SSE specification
- Iterator-based API (`Iterator<Item = Result<Event, EventSourceError>>`)
- Handles all SSE field types (`data:`, `event:`, `id:`, `retry:`)
- No need to implement custom parser

**Integration**:
```rust
use foundation_core::event_source::{SseParser, Event as SseEvent};

// Wrap parser with type-safe chunk iterator
pub struct ChatCompletionStream<R: Read> {
    parser: SseParser<R>,
}

impl<R: Read> Iterator for ChatCompletionStream<R> {
    type Item = Result<ChatCompletionChunk, StreamingError>;
    // Parse SSE data into chunks
}
```

### 3. Separate Chat Completions and Responses API

**Decision**: Implement as separate modules with shared foundation types.

**Rationale**:
- Different request/response structures
- Responses API has unique streaming events
- Reasoning models have different parameter support
- Clearer code organization

### 4. Streaming with Iterator Pattern

**Decision**: Use Rust Iterator pattern for SSE stream parsing.

**Rationale**:
- Ergonomic Rust API
- Compatible with foundation_core patterns
- Easy to consume with standard iterator methods
- Memory efficient (lazy evaluation)

### 5. Comprehensive Error Types

**Decision**: Detailed `ApiError` enum plus `StreamingError` for stream-specific errors.

**Rationale**:
- Clear error messages for users
- Pattern matchable error handling
- Proper error categorization (client vs server vs API errors)
- Integration with `derive_more::From` for automatic conversions

### 6. API Key Handling

**Decision**: Use `String` with secure handling documentation (no `Secret` type in foundation_core).

**Rationale**:
- `foundation_core` does not have a `Secret` type
- Adding `zeroize` crate is optional for production use
- Document secure handling practices (never log API keys)

### 7. OpenAI Compatibility First

**Decision**: Match OpenAI's exact field names and JSON structure.

**Rationale**:
- Drop-in compatibility with OpenAI SDK
- Easy testing against real OpenAI API
- llama.cpp compatibility
- Future-proof for new OpenAI features

## Patterns Discovered

### SSE Parsing

OpenAI uses SSE (Server-Sent Events) format:
- Lines prefixed with `data: `
- Empty line separates events
- `[DONE]` marks stream end

**foundation_core SSE parser handles**:
- `data:` field → `SseEvent::Message { data, ... }`
- `event:` field → Sets event type
- `id:` field → Tracked for reconnection
- `:` comments → `SseEvent::Comment`

### Chat Completions vs Responses API

| Aspect | Chat Completions | Responses API |
|--------|-----------------|---------------|
| Endpoint | `/v1/chat/completions` | `/v1/responses` |
| ID Prefix | `cmpl_` | `resp_` |
| Input | `messages` array | `input` (text or structured) |
| Output | `choices` array | `output` array |
| Streaming | Chunks with deltas | Granular events by type |
| Stream events | Single format | Multiple event types |

### Error Response Structure

OpenAI error responses follow this pattern:
```json
{
  "error": {
    "message": "Invalid API key",
    "type": "invalid_request_error",
    "code": "invalid_api_key"
  }
}
```

Implementation:
```rust
#[derive(Deserialize)]
pub struct OpenAiErrorResponse {
    pub error: OpenAiError,
}

#[derive(Deserialize)]
pub struct OpenAiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: Option<String>,
}
```

## Implementation Notes

### Type Design

- Use `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields
- Use `#[serde(untagged)]` for polymorphic types (like `Content`)
- Use `#[serde(tag = "type")]` for tagged enums (like `ContentPart`)
- Use `String` for API keys with secure handling documentation

### HTTP Client Usage

```rust
let response = client
    .post(&url)?
    .header("Authorization", &format!("Bearer {}", api_key))?
    .header("Content-Type", "application/json")?
    .body(json_body)?
    .send()?;
```

### SSE to Type Mapping

```rust
// Chat Completions
match sse_event {
    SseEvent::Message { data, .. } if data == "[DONE]" => None,
    SseEvent::Message { data, .. } => {
        let chunk: ChatCompletionChunk = serde_json::from_str(&data)?;
        Some(Ok(chunk))
    }
    SseEvent::Comment(_) => continue,  // Ignore
}

// Responses API
match sse_event {
    SseEvent::Message { data, .. } => {
        let event: ResponseEvent = serde_json::from_str(&data)?;
        Some(Ok(event))
    }
    // ...
}
```

## Architecture Diagrams

See `ARCHITECTURE_REVIEW.md` for complete Mermaid diagrams showing:
- Module structure
- Request/response flows
- Type hierarchies
- Feature dependencies

## Verified Integrations

1. **foundation_core::SimpleHttpClient** - HTTP transport
2. **foundation_core::SseParser** - SSE parsing (verified in `wire/event_source/parser.rs`)
3. **foundation_core::event_source::Event** - SSE event type
4. **derive_more::From** - Error conversions (already in dependencies)
5. **serde/serde_json** - JSON handling (already in dependencies)

## Gaps Addressed

1. **No Secret type**: Use `String` with documentation
2. **Missing StreamingError**: Added to foundation feature tasks
3. **Missing OpenAiErrorResponse**: Added to foundation feature tasks
4. **SSE parser details**: Verified and documented integration approach

---

_Last Updated: 2026-03-08_
_Architecture Review: Complete_
