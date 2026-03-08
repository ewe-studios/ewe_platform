---
purpose: "Architecture review and logic verification for the OpenAI-compatible API specification"
version: "1.0"
created: 2026-03-08
---

# Architecture Review

## Module Structure Diagram

```mermaid
graph TB
    subgraph foundation_ai["backends/foundation_ai/src/openai/"]
        mod["mod.rs - Public API"]
        errors["errors.rs - ApiError, StreamingError"]
        types["types.rs - Common types"]
        client["client.rs - OpenAIClient"]

        subgraph chat["chat/"]
            chat_mod["mod.rs"]
            chat_types["types.rs"]
            chat_stream["stream.rs"]
        end

        subgraph responses["responses/"]
            resp_mod["mod.rs"]
            resp_types["types.rs"]
            resp_stream["stream.rs"]
        end
    end

    subgraph foundation_core["foundation_core"]
        http_client["simple_http::client::SimpleHttpClient"]
        sse_parser["event_source::SseParser"]
        sse_event["event_source::SseEvent"]
    end

    mod --> errors
    mod --> types
    mod --> client
    mod --> chat
    mod --> responses

    client --> http_client
    chat_stream --> sse_parser
    resp_stream --> sse_parser
    chat_types --> types
    resp_types --> types
```

## Request Flow Diagram

```mermaid
sequenceDiagram
    participant User
    participant Client as OpenAIClient
    participant HTTP as SimpleHttpClient
    participant Server as API Server

    User->>Client: chat_completions(request)
    Client->>Client: Serialize request to JSON
    Client->>HTTP: POST /v1/chat/completions
    Note over Client,HTTP: Headers: Authorization, Content-Type
    HTTP->>Server: HTTP POST request
    Server->>HTTP: HTTP Response (JSON)
    HTTP->>Client: Response bytes
    Client->>Client: Deserialize response
    Client->>User: ChatCompletionResponse
```

## Streaming Flow Diagram

```mermaid
sequenceDiagram
    participant User
    participant Stream as ChatCompletionStream
    participant Parser as SseParser
    participant HTTP as SimpleHttpClient
    participant Server as API Server

    User->>Stream: chat_completions_stream(request)
    Stream->>HTTP: POST with stream=true
    HTTP->>Server: HTTP POST request
    Server->>HTTP: SSE stream

    loop For each SSE event
        HTTP->>Parser: Read bytes
        Parser->>Parser: parse_next()
        Parser->>Stream: SseEvent (data: {...})
        Stream->>Stream: Parse JSON chunk
        Stream->>User: ChatCompletionChunk
    end

    Server->>HTTP: [DONE]
    Stream->>User: None (end of stream)
```

## Type Hierarchy Diagram

```mermaid
classDiagram
    class ApiError {
        +InvalidApiKey
        +InvalidBaseUrl
        +SerializationFailed
        +DeserializationFailed
        +HttpRequestFailed
        +ApiError
        +RateLimitExceeded
        +ContextSizeExceeded
    }

    class Role {
        <<enum>>
        +System
        +User
        +Assistant
        +Developer
        +Tool
    }

    class Content {
        <<enum>>
        +Text(String)
        +Parts(Vec~ContentPart~)
    }

    class ContentPart {
        <<enum>>
        +Text{text}
        +Image{image_url}
        +InputImage{url}
        +InputText{text}
    }

    class Message {
        +role: Role
        +content: Content
        +name: Option~String~
    }

    class Usage {
        +prompt_tokens: usize
        +completion_tokens: usize
        +total_tokens: usize
    }

    class CommonParams {
        +temperature: Option~f32~
        +top_p: Option~f32~
        +max_tokens: Option~usize~
        +stop: Option~Vec~String~~
        +seed: Option~u32~
    }

    ApiError --|> HttpClientError
    Message --> Role
    Message --> Content
    Content --> ContentPart
```

## Chat Completions Type Diagram

```mermaid
classDiagram
    class ChatCompletionRequest {
        +model: String
        +messages: Vec~Message~
        +temperature: Option~f32~
        +max_tokens: Option~usize~
        +stream: Option~bool~
        +tools: Option~Vec~Tool~~
        +response_format: Option~ResponseFormat~
    }

    class ChatCompletionResponse {
        +id: String
        +object: String
        +created: u64
        +model: String
        +choices: Vec~ChatCompletionChoice~
        +usage: Usage
    }

    class ChatCompletionChoice {
        +index: usize
        +message: AssistantMessage
        +finish_reason: Option~FinishReason~
        +logprobs: Option~LogProbs~
    }

    class AssistantMessage {
        +role: Role
        +content: Option~String~
        +tool_calls: Option~Vec~ToolCall~~
        +refusal: Option~String~
    }

    class Tool {
        +type: ToolType
        +function: Function
    }

    class ToolCall {
        +id: String
        +type: ToolType
        +function: FunctionCall
    }

    class ChatCompletionChunk {
        +id: String
        +choices: Vec~ChunkChoice~
        +usage: Option~Usage~
    }

    class ChunkChoice {
        +delta: ChoiceDelta
        +finish_reason: Option~FinishReason~
    }

    class ChoiceDelta {
        +role: Option~Role~
        +content: Option~String~
    }

    ChatCompletionRequest --> Message
    ChatCompletionResponse --> ChatCompletionChoice
    ChatCompletionChoice --> AssistantMessage
    ChatCompletionChoice --> FinishReason
    ChatCompletionChunk --> ChunkChoice
    ChunkChoice --> ChoiceDelta
```

## Responses API Type Diagram

```mermaid
classDiagram
    class ResponseRequest {
        +model: String
        +input: ResponseInput
        +instructions: Option~String~
        +max_output_tokens: Option~usize~
        +stream: Option~bool~
        +previous_response_id: Option~String~
    }

    class ResponseInput {
        <<enum>>
        +Text(String)
        +Items(Vec~InputItem~)
    }

    class InputItem {
        <<enum>>
        +Message{role, content}
        +FunctionCallOutput{call_id, output}
        +Image{image_url}
        +File{filename, file_data}
    }

    class Response {
        +id: String
        +object: String
        +created_at: u64
        +model: String
        +output: Vec~OutputItem~
        +status: String
        +usage: Option~ResponseUsage~
    }

    class OutputItem {
        <<enum>>
        +Message{id, status, role, content}
        +FunctionCall{id, call_id, name, arguments}
        +Reasoning{id, content}
    }

    class ResponseEvent {
        <<enum>>
        +ResponseCreated{response}
        +ResponseInProgress{response}
        +ResponseOutputItemAdded{item}
        +ResponseOutputTextDelta{delta}
        +ResponseCompleted{response}
        +ResponseFailed{response}
    }

    ResponseRequest --> ResponseInput
    ResponseInput --> InputItem
    Response --> OutputItem
    ResponseEvent --> Response
    ResponseEvent --> OutputItem
```

## Feature Dependency Graph

```mermaid
graph TD
    F0["00-foundation<br/>Error types, base types,<br/>OpenAIClient"]

    F1["01-chat-completions-types<br/>Request/Response types,<br/>Tool calling types"]

    F2["02-chat-completions-client<br/>Non-streaming client<br/>Error handling"]

    F3["03-streaming-support<br/>SSE parsing,<br/>Stream iterators"]

    F4["04-responses-api<br/>ResponseRequest,<br/>Response, events"]

    F5["05-integration-tests<br/>Mock server tests,<br/>llama.cpp tests"]

    F0 --> F1
    F0 --> F3
    F0 --> F4

    F1 --> F2
    F1 --> F3

    F3 --> F2
    F3 --> F4

    F2 --> F5
    F4 --> F5
```

## Logic Verification Checklist

### Foundation Layer (00-foundation)

- [x] Error types cover all API error scenarios
  - [x] Invalid API key
  - [x] Invalid base URL
  - [x] Serialization/deserialization errors
  - [x] HTTP request errors (via HttpClientError)
  - [x] API errors (parsed from response)
  - [x] Rate limiting
  - [x] Context size exceeded
- [x] Base types reusable across both APIs
  - [x] Role enum (includes Developer for Responses API)
  - [x] Content/ContentPart (supports both text and multipart)
  - [x] Message struct
  - [x] Usage tracking
- [x] HTTP client integration
  - [x] Uses foundation_core::SimpleHttpClient
  - [x] Builder pattern for configuration
  - [x] API key handling
- [ ] **MISSING**: `foundation_core::Secret` type verification
  - Note: Grep showed no Secret type in foundation_core
  - **ACTION**: Use String with secure handling or create Secret wrapper

### Chat Completions (01, 02)

- [x] Request types match OpenAI spec
  - [x] All common parameters
  - [x] Tool/function calling
  - [x] Response format constraints
- [x] Response types match OpenAI spec
  - [x] ChatCompletionResponse structure
  - [x] Choice and message types
  - [x] Tool call structures
  - [x] Log probabilities
- [x] Streaming types
  - [x] ChatCompletionChunk
  - [x] ChoiceDelta
- [ ] **MISSING**: FinishReason variant for tool_calls content_filter

### Streaming Support (03)

- [x] Reuses foundation_core SSE parser
  - [x] SseParser available
  - [x] SseEvent type available
  - [x] Iterator pattern implemented
- [x] Stream wrapper for Chat Completions
- [x] Stream wrapper for Responses API
- [ ] **MISSING**: Detailed SSE event parsing for Responses API events

### Responses API (04)

- [x] Request types
  - [x] ResponseRequest
  - [x] ResponseInput (text or structured)
  - [x] InputItem variants
- [x] Response types
  - [x] Response structure
  - [x] OutputItem variants (Message, FunctionCall, Reasoning)
  - [x] ResponseUsage with reasoning_tokens
- [x] Streaming events
  - [x] ResponseEvent enum with all event types
- [ ] **MISSING**: Detailed handling for thinking/reasoning content

### Integration Tests (05)

- [x] Mock server infrastructure
- [x] Chat Completions tests
- [x] Responses API tests
- [x] llama.cpp integration tests

## Identified Gaps and Actions

### Gap 1: Secret Type for API Key

**Issue**: Specification references `foundation_core::Secret` which doesn't exist.

**Options**:
1. Use String with documentation about secure handling
2. Create a simple Secret wrapper in foundation_ai
3. Check if Secret exists elsewhere in the codebase

**Resolution**: Add to foundation feature:
```rust
// Simple secret wrapper or use String with clear documentation
// Option: Use zeroize crate for secure string handling
```

### Gap 2: Additional FinishReason Variants

**Issue**: Missing `ToolCalls` and `ContentFilter` in some type definitions.

**Resolution**: Ensure all FinishReason variants are present:
```rust
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}
```

### Gap 3: Streaming Event Type Mapping

**Issue**: SSE parser returns generic `SseEvent`, need to map to API-specific types.

**Resolution**: Add parsing layer in stream modules:
```rust
impl ChatCompletionStream {
    fn parse_sse_event(&mut self, sse: SseEvent) -> Result<ChatCompletionChunk, StreamingError> {
        match sse {
            SseEvent::Message { data, .. } => {
                if data == "[DONE]" { return Err(StreamingError::Done); }
                serde_json::from_str(&data).map_err(...)
            }
            SseEvent::Comment(_) => None, // Ignore comments
        }
    }
}
```

### Gap 4: Async Support

**Issue**: HTTP client appears to be synchronous. OpenAI APIs typically support async.

**Resolution**: Document that current implementation is sync-only, or add async wrapper using threads.

### Gap 5: Error Response Structure

**Issue**: Need explicit error response type for parsing API errors.

**Resolution**: Add to foundation feature:
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

## Updated Dependencies

```mermaid
graph LR
    subgraph foundation_core
        HC[SimpleHttpClient]
        SSE[SseParser/SseEvent]
    end

    subgraph foundation_ai
        ERR[errors.rs]
        TYPS[types.rs - common]
        CLIENT[client.rs]
        CHAT_T[chat/types.rs]
        CHAT_S[chat/stream.rs]
        RESP_T[responses/types.rs]
        RESP_S[responses/stream.rs]
    end

    HC --> CLIENT
    SSE --> CHAT_S
    SSE --> RESP_S
    TYPS --> CHAT_T
    TYPS --> RESP_T
    ERR --> CLIENT
    ERR --> CHAT_S
    ERR --> RESP_S
```

## Summary

The specification is **logically sound** with the following verified aspects:

1. **Module structure** - Clear separation of concerns
2. **Type hierarchy** - Proper inheritance and composition
3. **Dependencies** - Correct feature dependency order
4. **SSE handling** - Reuses existing foundation_core parser
5. **Error handling** - Comprehensive error types

**Required additions**:
1. Add `OpenAiErrorResponse` type for error parsing
2. Add `StreamingError` type for stream-specific errors
3. Clarify API key handling (no Secret type exists)
4. Add explicit SSE-to-chunk parsing logic

---

_Review completed: 2026-03-08_
