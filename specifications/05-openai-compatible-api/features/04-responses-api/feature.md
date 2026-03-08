---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/04-responses-api"
this_file: "specifications/05-openai-compatible-api/features/04-responses-api/feature.md"

status: pending
priority: high
created: 2026-03-08

depends_on:
  - 00-foundation
  - 03-streaming-support

tasks:
  completed: 0
  uncompleted: 6
  total: 6
  completion_percentage: 0%
---


# Responses API Feature

## Overview

Implement the OpenAI Responses API for reasoning models (o1, o3, etc.). This API uses different request/response structures optimized for models that perform extended reasoning before producing output.

## Dependencies

This feature depends on:
- `00-foundation` - Uses `ApiError`, `OpenAIClient`, base types
- `03-streaming-support` - Reuses streaming infrastructure

This feature is required by:
- `05-integration-tests` - For end-to-end reasoning model tests

## Requirements

### Responses API Request

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResponseRequest {
    /// Model identifier (e.g., "o1", "o1-pro", "o3-mini")
    pub model: String,

    /// Input to process (text string or structured input)
    pub input: ResponseInput,

    /// System instructions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Max output tokens (default varies by model)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,

    /// Temperature (typically 1.0 for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top P
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Whether to stream responses
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Truncate input if exceeds context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate: Option<bool>,

    /// Previous response ID for conversation continuation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}
```

### Response Input Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    /// Simple text input
    Text(String),

    /// Structured input item list
    Items(Vec<InputItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    /// Input message from user/system
    Message {
        role: InputRole,
        content: InputContent,
    },

    /// Function call output
    FunctionCallOutput {
        call_id: String,
        output: String,
    },

    /// Image input (multimodal)
    Image {
        image_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },

    /// File input
    File {
        filename: String,
        file_data: String,  // base64 encoded
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRole {
    System,
    User,
    Assistant,
    Developer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputContent {
    Text(String),
    Parts(Vec<InputContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputContentPart {
    InputText { text: String },
    InputImage { image_url: String },
    InputFile { filename: String, file_data: String },
}
```

### Response Output

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Unique identifier (starts with "resp_")
    pub id: String,

    /// Object type ("response")
    pub object: String,

    /// Unix timestamp
    pub created_at: u64,

    /// Model identifier
    pub model: String,

    /// Output items
    pub output: Vec<OutputItem>,

    /// Status
    pub status: String,

    /// Error (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,

    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    /// Message output
    Message {
        id: String,
        status: OutputStatus,
        role: String,
        content: Vec<OutputContent>,
    },

    /// Function call
    FunctionCall {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: OutputStatus,
    },

    /// Reasoning content (thinking tokens)
    Reasoning {
        id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStatus {
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContent {
    OutputText { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    /// Reasoning-specific: tokens used for thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}
```

### Streaming Events

Responses API streaming uses these SSE events:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseEvent {
    /// Initial response created
    ResponseCreated { response: Response },

    /// Response in progress
    ResponseInProgress { response: Response },

    /// New output item added
    ResponseOutputItemAdded {
        response_id: String,
        output_index: usize,
        item: OutputItem,
    },

    /// Output item completed
    ResponseOutputItemDone {
        response_id: String,
        output_index: usize,
        item: OutputItem,
    },

    /// Text delta (chunk)
    ResponseOutputTextDelta {
        item_id: String,
        output_index: usize,
        delta: String,
    },

    /// Text completed
    ResponseOutputTextDone {
        item_id: String,
        output_index: usize,
        text: String,
    },

    /// Response completed
    ResponseCompleted { response: Response },

    /// Response failed
    ResponseFailed { response: Response },
}
```

### Client Implementation

```rust
impl OpenAIClient {
    /// Send a Responses API request (non-streaming).
    pub async fn responses(
        &self,
        request: ResponseRequest,
    ) -> Result<Response, ApiError> {
        // POST to /v1/responses
    }

    /// Send a Responses API request (streaming).
    pub fn responses_stream(
        &self,
        request: ResponseRequest,
    ) -> Result<ResponseStream, ApiError> {
        // POST to /v1/responses with stream: true
    }
}
```

## Tasks

1. **Create Responses API types module**
   - [ ] Create `backends/foundation_ai/src/openai/responses/types.rs`
   - [ ] Define `ResponseRequest` and all input types
   - [ ] Define `Response` and all output types
   - [ ] Define `ResponseEvent` for streaming

2. **Implement non-streaming client**
   - [ ] Add `responses()` method to `OpenAIClient`
   - [ ] Handle POST to `/v1/responses`
   - [ ] Parse response

3. **Implement streaming client**
   - [ ] Add `responses_stream()` method
   - [ ] Create `ResponseStream` iterator
   - [ ] Parse SSE events specific to Responses API

4. **Handle reasoning-specific features**
   - [ ] Support `reasoning_tokens` in usage
   - [ ] Parse `Reasoning` output items
   - [ ] Handle extended thinking patterns

5. **Write tests**
   - [ ] Test request serialization
   - [ ] Test response deserialization
   - [ ] Test streaming event parsing

6. **Add documentation**
   - [ ] Document Responses API types
   - [ ] Add usage examples
   - [ ] Document differences from Chat Completions

## Implementation Notes

- Responses API uses different endpoint: `/v1/responses`
- Response IDs start with `resp_` (not `cmpl_`)
- Output items have IDs starting with `msg_`
- Reasoning models may not support all parameters
- Streaming events are more granular than Chat Completions

## Success Criteria

- [ ] All 6 tasks completed
- [ ] Compatible with llama.cpp Responses API
- [ ] Streaming events parsed correctly
- [ ] Tests pass

## Verification

```bash
cd backends/foundation_ai
cargo test openai::responses::
```

---

_Created: 2026-03-08_
