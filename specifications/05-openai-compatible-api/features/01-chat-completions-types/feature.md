---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/01-chat-completions-types"
this_file: "specifications/05-openai-compatible-api/features/01-chat-completions-types/feature.md"

status: pending
priority: high
created: 2026-03-08

depends_on:
  - 00-foundation

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Chat Completions Types Feature

## Overview

Define all types required for the OpenAI Chat Completions API, including request structures, response structures, and streaming event types.

## Dependencies

This feature depends on:
- `00-foundation` - Uses `ApiError`, `Role`, `Message`, `Content`, `CommonParams`

This feature is required by:
- `02-chat-completions-client` - Uses request/response types
- `03-streaming-support` - Uses streaming event types

## Requirements

### Chat Completion Request

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    /// Model identifier (e.g., "gpt-4", "gpt-3.5-turbo")
    pub model: String,

    /// Messages to send
    pub messages: Vec<Message>,

    /// Controls randomness (0.0 - 2.0, default: 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling (0.0 - 1.0, default: 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Number of completions to generate (default: 1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<usize>,

    /// Whether to stream responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Max tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,

    /// Presence penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Frequency penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Modify likelihood based on prior tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, f32>>,

    /// User identifier for monitoring
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Random seed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,

    /// Response format constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// Tools available for function calling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,

    /// Tool choice strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}
```

### Response Format

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// "text", "json_object", or "json_schema"
    #[serde(rename = "type")]
    pub format_type: ResponseFormatType,

    /// JSON schema (for "json_schema" type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<JsonSchema>,

    /// Schema (for "json_object" type - legacy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormatType {
    Text,
    JsonObject,
    JsonSchema,
}
```

### Tool/Function Calling

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "required")]
    Required,
    Function(FunctionChoice),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionChoice {
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: FunctionName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionName {
    pub name: String,
}
```

### Chat Completion Response

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    /// Unique identifier for this completion
    pub id: String,

    /// Object type ("chat.completion")
    pub object: String,

    /// Unix timestamp of creation
    pub created: u64,

    /// Model that generated this response
    pub model: String,

    /// System fingerprint for reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,

    /// Generated completions
    pub choices: Vec<ChatCompletionChoice>,

    /// Token usage statistics
    pub usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    /// Index of this choice in the list
    pub index: usize,

    /// The generated message
    pub message: AssistantMessage,

    /// Reason generation stopped
    pub finish_reason: Option<FinishReason>,

    /// Log probabilities (if requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: Role,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,

    /// Tool calls from the model
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,

    /// Function call (legacy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<FunctionCall>,

    /// Refusal message (if model refused)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}
```

### Tool Call Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: ToolType,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Function,
}
```

### Log Probabilities

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogProbs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentLogProb>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<RefusalLogProb>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentLogProb {
    pub token: String,
    pub logprob: f32,
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogProb>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopLogProb {
    pub token: String,
    pub logprob: f32,
    pub bytes: Option<Vec<u8>>,
}
```

## Tasks

1. **Create Chat Completions types module**
   - [ ] Create `backends/foundation_ai/src/openai/chat/types.rs`
   - [ ] Define all request types
   - [ ] Define all response types
   - [ ] Define tool/function calling types

2. **Create streaming event types**
   - [ ] Define `ChatCompletionChunk` for SSE events
   - [ ] Define `ChoiceDelta` for incremental updates
   - [ ] Define streaming-specific types

3. **Implement serialization helpers**
   - [ ] Add builder pattern for `ChatCompletionRequest`
   - [ ] Add convenience methods for common patterns

4. **Create module structure**
   - [ ] Create `backends/foundation_ai/src/openai/chat/mod.rs`
   - [ ] Export all public types
   - [ ] Re-export commonly used types to parent module

5. **Write unit tests**
   - [ ] Test request serialization
   - [ ] Test response deserialization
   - [ ] Test streaming chunk deserialization
   - [ ] Test tool calling types

6. **Add documentation**
   - [ ] Document all public types
   - [ ] Add usage examples
   - [ ] Link to OpenAI documentation references

## Implementation Notes

- Use `#[serde(skip_serializing_if = "Option::is_none")]` for optional fields
- Follow OpenAI's exact field names for compatibility
- Support both new `tools` API and legacy `functions` API
- Handle both string content and array content in messages

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Types serialize to OpenAI-compatible JSON
- [ ] Types deserialize from OpenAI-compatible JSON
- [ ] Clippy passes with no warnings
- [ ] Tests pass

## Verification

```bash
cd backends/foundation_ai
cargo test openai::chat::
```

---

_Created: 2026-03-08_
