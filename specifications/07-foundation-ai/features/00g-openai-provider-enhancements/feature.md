---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/00g-openai-provider-enhancements"
this_file: "specifications/07-foundation-ai/features/00g-openai-provider-enhancements/feature.md"

feature: "OpenAI Provider Enhancements"
description: "Close all gaps between the existing OpenAI provider and the full OpenAI API surface — Responses API, structured output, advanced sampling params, multimodal content, logprobs, and additional error handling"
status: complete
priority: high
depends_on:
  - "00c-openai-provider"
estimated_effort: "large"
created: 2026-04-26
last_updated: 2026-04-27
author: "Main Agent"

tasks:
  completed: 64
  uncompleted: 0
  total: 64
  completion_percentage: 100%
---

# OpenAI Provider Enhancements

## Overview

The `00c-openai-provider` feature implemented the Chat Completions API with core
parameters, streaming, retry, and embeddings. However, the full OpenAI API surface
is larger than what was originally scoped. This feature closes **all identified gaps**
between the current implementation and complete OpenAI compatibility.

Gaps were identified by comparing spec `05-openai-compatible-api` against the
actual `openai_provider.rs` implementation.

**Iron laws from `requirements.md` apply** — no tokio/async-trait, Valtron-only
async, zero warnings, `derive_more::From` + manual `Display` errors.

## Task Group 1: Output Format Control

Add `OutputFormat` to `ModelParams` and wire it through `build_chat_request`
as the `response_format` field.

### Types to Add (in `types/mod.rs`)

```rust
/// Constrain the model's output format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum OutputFormat {
    /// Plain text output (default).
    #[default]
    Text,
    /// Force JSON output. Model responds with valid JSON.
    JsonObject,
    /// Schema-constrained JSON output.
    JsonSchema(JsonSchema),
}

/// JSON schema for response_format constraint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonSchema {
    /// Name of the schema (for identification).
    pub name: String,
    /// Description of what the schema represents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The JSON schema definition.
    pub schema: serde_json::Value,
    /// Whether to enforce strict schema validation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}
```

### ModelParams Changes

```rust
pub output_format: Option<OutputFormat>,
```

### Request Serialization

```rust
// In build_chat_request:
response_format: params.output_format.map(|f| match f {
    OutputFormat::Text => OpenAIResponseFormat { format_type: "text" },
    OutputFormat::JsonObject => OpenAIResponseFormat { format_type: "json_object" },
    OutputFormat::JsonSchema(schema) => OpenAIResponseFormat {
        format_type: "json_schema",
        json_schema: Some(schema),
    },
}),
```

### Tasks

- [ ] Add `OutputFormat` and `JsonSchema` types to `types/mod.rs`
- [ ] Add `output_format: Option<OutputFormat>` to `ModelParams`
- [ ] Add `OpenAIResponseFormat` struct in `openai_provider.rs`
- [ ] Wire `response_format` into `build_chat_request`
- [ ] Test: JSON object mode serialization
- [ ] Test: JSON schema mode serialization with strict flag

## Task Group 2: Advanced Sampling Parameters

Add missing OpenAI sampling parameters that are already meaningful generation
controls.

### ModelParams Changes

```rust
pub frequency_penalty: Option<f32>,  // -2.0 to 2.0
pub presence_penalty: Option<f32>,   // -2.0 to 2.0
```

Note: `top_k` already exists in `ModelParams` but OpenAI does not support it
directly. No mapping needed — it remains a provider-agnostic parameter that
local backends (llama.cpp, Candle) use.

### Request Types

```rust
// In ChatCompletionRequest:
#[serde(skip_serializing_if = "Option::is_none")]
pub frequency_penalty: Option<f32>,

#[serde(skip_serializing_if = "Option::is_none")]
pub presence_penalty: Option<f32>,

#[serde(skip_serializing_if = "Option::is_none")]
pub logit_bias: Option<HashMap<String, f32>>,

#[serde(skip_serializing_if = "Option::is_none")]
pub user: Option<String>,
```

### `ModelParams` Changes

```rust
pub logit_bias: Option<HashMap<String, f32>>,
```

### Tasks

- [ ] Add `frequency_penalty`, `presence_penalty` to `ModelParams`
- [ ] Add `logit_bias` to `ModelParams`
- [ ] Add fields to `ChatCompletionRequest` with serialization
- [ ] Wire them through `build_chat_request`
- [ ] Test: all new params serialized when set, omitted when not

## Task Group 3: Tool Choice Strategy

Add `tool_choice` parameter to control whether/how the model uses tools.

### Types to Add

```rust
/// Strategy for tool selection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto,
    /// Force the model to not use any tools.
    None,
    /// Force the model to use at least one tool.
    Required,
    /// Force the model to use a specific function.
    Function(ToolChoiceFunction),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolChoiceFunction {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolFunction {
    pub name: String,
}
```

**Note on `ToolFunction` single-field struct:** Serde supports `#[serde(serialize_with, deserialize_with)]` to render
a plain `String` as `{"name": "..."}` and back, which would eliminate the struct. However, that requires custom
serialization functions (~10 lines) that are more code and less readable than the struct itself. Keeping the
struct is the simpler approach.

### `ModelInteraction` Changes

```rust
pub tool_choice: Option<ToolChoice>,
```

### Request Serialization

```rust
// In ChatCompletionRequest:
#[serde(skip_serializing_if = "Option::is_none")]
pub tool_choice: Option<ToolChoice>,
```

### Tasks

- [ ] Add `ToolChoice` enum and helper structs to `types/mod.rs`
- [ ] Add `tool_choice: Option<ToolChoice>` to `ModelInteraction`
- [ ] Add `tool_choice` to `ChatCompletionRequest`
- [ ] Wire through `build_chat_request`
- [ ] Test: auto/none/required/function serialization

## Task Group 4: Multimodal Input

Replace the `[Image]` placeholder in `build_chat_request` with proper
multimodal content using OpenAI's content array format.

### OpenAI Message Content Format

OpenAI accepts `content` as either a string (text-only) or an array of
content parts for multimodal:

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "What is in this image?" },
    { "type": "image_url", "image_url": { "url": "data:image/png;base64,..." } }
  ]
}
```

### Types to Add

```rust
/// Content for an OpenAI message — either simple text or multimodal parts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAIMessageContent {
    /// Simple text content (shorthand form).
    Text(String),
    /// Array of content parts for multimodal messages.
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenAIContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenAIImageUrlObject },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageUrlObject {
    /// Data URL (base64) or HTTPS URL.
    pub url: String,
    /// Detail level: "low", "high", or "auto".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
```

### Current `build_chat_request` Change

```rust
// Current (broken):
crate::types::UserModelContent::Image(_) => String::from("[Image]"),

// New:
crate::types::UserModelContent::Image(img) => {
    OpenAIMessageContent::Parts(vec![
        OpenAIContentPart::ImageUrl {
            image_url: OpenAIImageUrlObject {
                url: format!("data:{};base64,{}", img.mime_type, img.b64),
                detail: Some("auto".into()),
            },
        },
    ])
}
```

This changes `OpenAIMessage.content` from `Option<String>` to
`Option<OpenAIMessageContent>`.

### Tasks

- [ ] Add `OpenAIMessageContent` and `OpenAIContentPart` types
- [ ] Add `OpenAIImageUrlObject` struct
- [ ] Change `OpenAIMessage.content` from `Option<String>` to `Option<OpenAIMessageContent>`
- [ ] Update `build_chat_request` to build content parts for images
- [ ] Update `build_chat_request` to build content parts for assistant images
- [ ] Test: text-only message serializes as simple string
- [ ] Test: image message serializes as content array with data URL

## Task Group 5: Responses API Provider

Implement `OpenAIResponsesProvider` implementing `ModelProvider` for the
`/v1/responses` endpoint (reasoning models: o1, o3, o1-pro, etc.).

### Architecture

```
OpenAIResponsesProvider (ModelProvider)
├── ResponseRequest (serializable request)
├── Response (deserializable response)
├── ResponseStream (StreamIterator for SSE events)
└── Responses API types (input/output/event)
```

### Request Types

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ResponseRequest {
    pub model: String,
    pub input: ResponseInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncate: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_response_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseInput {
    /// Simple text input.
    Text(String),
    /// Structured input item list.
    Items(Vec<InputItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputItem {
    Message { role: String, content: InputContent },
    FunctionCallOutput { call_id: String, output: String },
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
}
```

### Response Types

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Response {
    pub id: String,
    pub object: String,
    pub created_at: u64,
    pub model: String,
    pub output: Vec<OutputItem>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ResponseError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<ResponseUsage>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Message {
        id: String,
        status: String,
        role: String,
        content: Vec<OutputContent>,
    },
    FunctionCall {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: String,
    },
    Reasoning {
        id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputContent {
    OutputText { text: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponseError {
    pub code: String,
    pub message: String,
}
```

### Streaming Event Types

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseEvent {
    ResponseCreated { response: Response },
    ResponseInProgress { response: Response },
    ResponseOutputItemAdded {
        response_id: String,
        item: OutputItem,
    },
    ResponseOutputItemDone {
        response_id: String,
        item: OutputItem,
    },
    ResponseOutputTextDelta {
        item_id: String,
        delta: String,
    },
    ResponseOutputTextDone {
        item_id: String,
        text: String,
    },
    ResponseCompleted { response: Response },
    ResponseFailed { response: Response },
}
```

### Provider Implementation

`OpenAIResponsesProvider` struct mirrors `OpenAIProvider`:
- Same config pattern (base_url, timeout, auth, retry)
- `ModelProvider` impl with `describe()` returning `ModelAPI::OpenAIResponses`
- `get_model()` / `get_all()` — same model discovery via `/v1/models`
- `generate()` — builds `ResponseRequest`, POSTs to `/v1/responses`, maps `Response` → `Messages`
- `stream()` — builds streaming request, uses `ReconnectingEventSourceTask`, parses `ResponseEvent` SSE events

### Mapping `ModelInteraction` → `ResponseRequest`

| foundation_ai | → Responses API |
|---|---|
| `system_prompt` | `instructions` |
| `messages: Vec<Messages>` | `input: ResponseInput::Items([...])` |
| `Messages::User` | `InputItem::Message { role: "user" }` |
| `Messages::Assistant { Text }` | `InputItem::Message { role: "assistant" }` |
| `Messages::ToolResult` | `InputItem::FunctionCallOutput` |
| `ModelParams::max_tokens` | `max_output_tokens` |
| `ModelParams::temperature` | `temperature` |
| `ModelParams::top_p` | `top_p` |

### Mapping `Response` → `Messages`

| Responses API | → `ModelOutput` |
|---|---|
| `OutputItem::Message { content }` | `ModelOutput::Text` |
| `OutputItem::FunctionCall` | `ModelOutput::ToolCall` |
| `OutputItem::Reasoning` | `ModelOutput::ThinkingContent` |
| `ResponseUsage` | `UsageReport` |
| `response_tokens` | `cache_read` field (reuse — closest semantic) |

### ProviderDescriptor

```rust
fn describe(&self) -> ModelProviderResult<ModelProviderDescriptor> {
    Ok(ModelProviderDescriptor {
        id: String::from("openai-responses"),
        name: String::from("OpenAI Responses"),
        reasoning: true,
        api: crate::types::ModelAPI::OpenAIResponses,
        // ... rest mirrors OpenAIProvider
    })
}
```

### Tasks

- [ ] Create `backends/foundation_ai/src/backends/openai_responses_provider.rs`
- [ ] Add all request types (`ResponseRequest`, `ResponseInput`, `InputItem`, etc.)
- [ ] Add all response types (`Response`, `OutputItem`, `ResponseUsage`, etc.)
- [ ] Add streaming event types (`ResponseEvent`)
- [ ] Implement `OpenAIResponsesProvider` struct with config/auth
- [ ] Implement `ModelProvider` trait for `OpenAIResponsesProvider`
- [ ] Implement `generate()` — non-streaming request + response mapping
- [ ] Implement `stream()` — SSE streaming with `ResponseEvent` parsing
- [ ] Implement `describe()` with `ModelAPI::OpenAIResponses`
- [ ] Add `generate()` → `Messages` mapping for all `OutputItem` variants
- [ ] Add `stream()` → `Messages` mapping for streaming events
- [ ] Add `UsageReport` mapping with `reasoning_tokens` handling
- [ ] Register module in `backends/mod.rs`
- [ ] Unit tests: request serialization, response deserialization
- [ ] Unit tests: streaming event parsing
- [ ] Unit tests: OutputItem → ModelOutput mapping
- [ ] Integration tests: provider creation, model discovery

## Task Group 6: Logprobs + Generation Metadata

Parse and expose log probabilities from responses, and consolidate provider-specific
metadata into a typed enum on `Messages::Assistant`.

### `GenerationMetadata` Enum (in `types/mod.rs`)

```rust
/// Provider-specific metadata attached to a generation result.
///
/// Variants are added as needed from provider wire formats.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerationMetadata {
    /// Log probabilities for each generated token (OpenAI).
    LogProbs {
        /// Per-token log probability and alternative tokens.
        content: Vec<ContentLogProb>,
        /// Log probs for refusal tokens, if any.
        refusal: Vec<RefusalLogProb>,
    },
    /// System fingerprint for reproducibility (OpenAI).
    /// Identifies the backend configuration that generated the response.
    SystemFingerprint(String),
    /// Timing information for generation (local backends).
    Timing {
        /// Total generation time in milliseconds.
        total_ms: u64,
        /// Time to first token in milliseconds.
        time_to_first_ms: Option<u64>,
        /// Tokens per second.
        tokens_per_sec: Option<f64>,
    },
    /// Model refusal reason for safety/policy violations.
    RefusalReason(String),
}
```

### LogProb Sub-types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContentLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogProbEntry>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TopLogProbEntry {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefusalLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}
```

### `Messages::Assistant` Changes

Add the metadata field to `Messages::Assistant` in `types/mod.rs`:

```rust
pub metadata: Option<Vec<GenerationMetadata>>,
```

This consolidates what was previously scattered across:
- `signature` — used for model signatures (kept, unrelated purpose)
- `error_detail` — used for error messages (kept, human-readable)
- New: `metadata` — typed, provider-specific structured data

### OpenAI Wire Format Types (in `openai_provider.rs`)

These deserialize the raw OpenAI response before mapping to `GenerationMetadata`:

```rust
// Wire format — OpenAI's logprobs in the response.
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAILogProbs {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<OpenAIContentLogProb>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Vec<OpenAIRefusalLogProb>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIContentLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<OpenAITopLogProb>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAITopLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIRefusalLogProb {
    pub token: String,
    pub logprob: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Vec<u8>>,
}
```

### Request Params

```rust
// In ChatCompletionRequest:
#[serde(skip_serializing_if = "Option::is_none")]
pub logprobs: Option<bool>,

#[serde(skip_serializing_if = "Option::is_none")]
pub top_logprobs: Option<usize>,

// In OpenAIChoice:
#[serde(skip_serializing_if = "Option::is_none")]
pub logprobs: Option<OpenAILogProbs>,

// In ChatCompletionResponse:
#[serde(skip_serializing_if = "Option::is_none")]
pub system_fingerprint: Option<String>,
```

### Mapping Wire → Internal

```
OpenAI wire field          → GenerationMetadata variant
───────────────────────────┼────────────────────────────────────────
choice.logprobs            → GenerationMetadata::LogProbs { content, refusal }
response.system_fingerprint → GenerationMetadata::SystemFingerprint
assistant.refusal (string) → GenerationMetadata::RefusalReason
```

### Tasks

- [ ] Add `GenerationMetadata` enum to `types/mod.rs`
- [ ] Add `LogProb`, `TopLogProbEntry`, `RefusalLogProb` types to `types/mod.rs`
- [ ] Add `metadata: Option<Vec<GenerationMetadata>>` to `Messages::Assistant`
- [ ] Add `logprobs` and `top_logprobs` to `ChatCompletionRequest`
- [ ] Add `logprobs` field to `OpenAIChoice`
- [ ] Add `system_fingerprint` to `ChatCompletionResponse`
- [ ] Wire `logprobs` → `GenerationMetadata::LogProbs` in `parse_chat_response`
- [ ] Wire `system_fingerprint` → `GenerationMetadata::SystemFingerprint`
- [ ] Test: logprobs deserialization and mapping
- [ ] Test: system_fingerprint deserialization and mapping
- [ ] Test: streaming chunk logprobs

## Task Group 7: Additional Response Fields

### `refusal` on Assistant Message

OpenAI returns safety/policy refusals as:

```json
{
  "message": {
    "role": "assistant",
    "content": null,
    "refusal": "I can't help with that."
  }
}
```

Parse the `refusal` field from `OpenAIMessage` and map to:
- `Messages::Assistant` with `stop_reason: StopReason::Error`
- `metadata: Some(vec![GenerationMetadata::RefusalReason(refusal_text)])`

### Tasks

- [ ] Parse `refusal` field from `OpenAIMessage`
- [ ] Map refusal → `StopReason::Error` + `GenerationMetadata::RefusalReason`
- [ ] Test: refusal deserialization and mapping

## Task Group 8: Enhanced StopReason + Error Handling

### `StopReason::Message` Variant

Add to the `StopReason` enum in `types/mod.rs`:

```rust
pub enum StopReason {
    Stop,
    Length,
    ToolUse,
    Error,
    Aborted,
    /// Custom or unknown stop reason — preserves the provider's
    /// actual reason string instead of silently mapping to Stop or Error.
    Message(String),
}
```

### Current Mappings to Fix

These are the locations across backends where unknown/custom finish reasons
are silently lost. All should use `StopReason::Message` instead:

**`openai_provider.rs` — `build_final_message()` (streaming, line ~862):**
```rust
// Before: silently maps everything unknown to Error
Some("stop") | None => StopReason::Stop,
Some("length") => StopReason::Length,
Some("tool_calls") => StopReason::ToolUse,
Some(_) => StopReason::Error,

// After: preserve the actual reason
Some("stop") | None => StopReason::Stop,
Some("length") => StopReason::Length,
Some("tool_calls") => StopReason::ToolUse,
Some("content_filter") => StopReason::Message("content_filter".into()),
Some(other) => StopReason::Message(other.into()),
```

**`openai_provider.rs` — `parse_chat_response()` (non-streaming, line ~1467):**
```rust
// Before
Some("stop") | None => StopReason::Stop,
Some("length") => StopReason::Length,
Some("tool_calls") => StopReason::ToolUse,
Some("content_filter") => StopReason::Error,
Some(other) => { tracing::warn!(…); StopReason::Stop }

// After
Some("stop") | None => StopReason::Stop,
Some("length") => StopReason::Length,
Some("tool_calls") => StopReason::ToolUse,
Some("content_filter") => StopReason::Message("content_filter".into()),
Some(other) => StopReason::Message(other.into()),
```

**`openai_provider.rs` — `generate_embeddings()` (line ~637):**
```rust
// Embedding requests always complete normally — keep as StopReason::Stop
// (no change needed, but add a comment so future readers know it's intentional)
```

**`openai_provider.rs` — streaming chunk loop (line ~818):**
```rust
// Before: hardcodes StopReason::Stop for every streamed chunk
stop_reason: StopReason::Stop,

// After: track finish_reason as it arrives (will be set by the
// time build_final_message is called, so the chunk can keep Stop)
// This is fine as-is — build_final_message overrides with the
// correct reason at stream end.
```

### ContextSizeExceeded Error Variant

Add a dedicated error variant for context overflow (currently detected via
regex patterns in `Messages::is_context_overflow()` but not as a dedicated
`GenerationError` variant).

```rust
// In errors/mod.rs:
ContextOverflow {
    prompt_tokens: usize,
    context_limit: usize,
},
```

### Tasks

- [ ] Add `Message(String)` variant to `StopReason` in `types/mod.rs`
- [ ] Update `build_final_message()` in `openai_provider.rs`: unknown finish reasons → `StopReason::Message`
- [ ] Update `parse_chat_response()` in `openai_provider.rs`: unknown finish reasons → `StopReason::Message`, remove `tracing::warn` fallback
- [ ] Add `ContextOverflow` variant to `GenerationError`
- [ ] Detect context overflow from HTTP 400 responses with matching body
- [ ] Return `ContextOverflow` instead of generic `Backend` error
- [ ] Test: `"content_filter"` finish reason maps to `StopReason::Message("content_filter")`
- [ ] Test: unknown finish reason (e.g. `"malicious_user_input"`) maps to `StopReason::Message("malicious_user_input")`
- [ ] Test: context overflow error detection

## Task Group 9: ModelProvider Registration

Register the new `OpenAIResponsesProvider` in the provider registry so it
can be selected by `ModelAPI::OpenAIResponses`.

### Tasks

- [ ] Add `openai-responses` provider ID to the registry
- [ ] Wire `ModelAPI::OpenAIResponses` to select the responses provider
- [ ] Test: provider selection by API type

## Success Criteria

### Output Format
- [ ] `OutputFormat::JsonObject` serializes as `"response_format": {"type": "json_object"}`
- [ ] `OutputFormat::JsonSchema` serializes with full schema + strict flag
- [ ] Default behavior (no format set) omits the field

### Sampling Params
- [ ] `frequency_penalty`, `presence_penalty`, `logit_bias`, `user` all serialize
- [ ] Fields omitted when None/default

### Tool Choice
- [ ] `ToolChoice::Auto` → `"tool_choice": "auto"`
- [ ] `ToolChoice::None` → `"tool_choice": "none"`
- [ ] `ToolChoice::Required` → `"tool_choice": "required"`
- [ ] `ToolChoice::Function` → `{"type": "function", "function": {"name": "..."}}`

### Multimodal
- [ ] Text-only messages serialize as simple string content
- [ ] Image messages serialize as content array with data URL
- [ ] Mixed text+image messages serialize correctly

### Responses API
- [ ] `OpenAIResponsesProvider` implements `ModelProvider`
- [ ] Non-streaming `generate()` returns correct `Messages`
- [ ] Streaming yields `Messages` via `ResponseEvent` parsing
- [ ] `OutputItem::Reasoning` → `ModelOutput::ThinkingContent`
- [ ] `OutputItem::FunctionCall` → `ModelOutput::ToolCall`
- [ ] `ResponseUsage` maps to `UsageReport` with `reasoning_tokens`
- [ ] `describe()` returns `ModelAPI::OpenAIResponses`, `reasoning: true`

### Metadata & Logprobs
- [ ] `GenerationMetadata` enum added with `LogProbs`, `SystemFingerprint`, `Timing`, `RefusalReason` variants
- [ ] `metadata: Option<Vec<GenerationMetadata>>` field on `Messages::Assistant`
- [ ] `logprobs` field serialized in request when set
- [ ] OpenAI logprobs mapped to `GenerationMetadata::LogProbs`
- [ ] `system_fingerprint` mapped to `GenerationMetadata::SystemFingerprint`
- [ ] Refusal text mapped to `GenerationMetadata::RefusalReason`

### StopReason
- [ ] `StopReason::Message(String)` variant for unknown/custom finish reasons
- [ ] Unknown finish reasons mapped to `StopReason::Message` instead of silently falling back to `Stop`

### Error Handling
- [ ] `refusal` field parsed from responses
- [ ] `ContextOverflow` error variant returned on context overflow

### Code Quality
- [ ] `cargo clippy --package foundation_ai --profile uat -- -D warnings` passes
- [ ] `cargo test --package foundation_ai --profile uat` passes
- [ ] `cargo fmt --package foundation_ai -- --check` passes

## Verification Commands

```bash
# Full verification
cargo check --package foundation_ai --profile uat
cargo clippy --package foundation_ai --profile uat -- -D warnings
cargo test --package foundation_ai --profile uat
cargo fmt --package foundation_ai -- --check

# Focused tests
cargo test --package foundation_ai --profile uat -- openai_provider
cargo test --package foundation_ai --profile uat -- openai_responses_provider
```

## Dependencies

This feature builds on:
- **00c-openai-provider** — existing `OpenAIProvider`, HTTP client integration, SSE streaming infrastructure
- **00e-http-client-connection-pool** — HTTP connection reuse for Responses API

The `ModelAPI::OpenAIResponses` enum variant already exists in `types/mod.rs`.
