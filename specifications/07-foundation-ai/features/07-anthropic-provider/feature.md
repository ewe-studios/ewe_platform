---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/07-anthropic-provider"
this_file: "specifications/07-foundation-ai/features/07-anthropic-provider/feature.md"

feature: "Anthropic Messages API Provider"
description: "Implement AnthropicMessagesAPIProvider connecting to the Anthropic Messages API (/v1/messages) — the primary interface for all Claude models — with full support for text generation, streaming, tool use, extended thinking, and multimodal input"
status: unapproved
priority: high
depends_on:
  - "00c-openai-provider"
  - "00g-openai-provider-enhancements"
estimated_effort: "large"
created: 2026-04-27
last_updated: 2026-04-27
author: "Main Agent"

tasks:
  completed: 0
  uncompleted: 32
  total: 32
  completion_percentage: 0%
---

# Anthropic Messages API Provider

## Overview

Implement `AnthropicMessagesProvider` — a provider that communicates with
Anthropic's Claude models via the **Messages API** (`POST /v1/messages`).

### What is the Messages API?

The Messages API is **the** primary way to interact with Anthropic's Claude
models. Unlike OpenAI, which historically had separate `/completions` and
`/chat/completions` endpoints, Anthropic only ever had one endpoint:
`/v1/messages`. There is no separate "completions" endpoint — every interaction
with a Claude model goes through Messages.

The Messages API is conceptually similar to OpenAI's Chat Completions API but
with important architectural differences:

| Aspect | OpenAI Chat Completions | Anthropic Messages |
|--------|------------------------|---------------------|
| **Endpoint** | `/v1/chat/completions` | `/v1/messages` |
| **Auth headers** | `Authorization: Bearer <key>` | `x-api-key: <key>` + `anthropic-version: <version>` |
| **System prompt** | `role: "system"` message | Separate `system` field (string or content array) |
| **Content model** | `content: string` or content array | Always `content: [...]` array of blocks |
| **Content types** | `text`, `image_url` | `text`, `image`, `tool_use`, `tool_result`, `thinking` |
| **Images** | `{"type":"image_url","image_url":{"url":"..."}}` | `{"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"..."}}` |
| **Tool calls** | `tool_calls` array on message | `{"type":"tool_use"}` content block |
| **Tool results** | `role: "tool"`, `tool_call_id` | `role: "user"`, `{"type":"tool_result"}` content block |
| **Stop reasons** | `stop`, `length`, `tool_calls`, `content_filter` | `end_turn`, `stop_sequence`, `max_tokens`, `tool_use` |
| **Thinking** | N/A | Extended thinking blocks (`thinking` + `redacted_thinking`) |
| **Streaming** | SSE with `data: {...}` chunks | SSE with `event: {...}` named events |
| **Streaming events** | Anonymous `data:` lines only | Named events: `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop` |
| **Usage in stream** | Only in final chunk | In `message_start` and `message_delta` events |
| **Model format** | `gpt-4o`, `o3`, `o1-pro` | `claude-sonnet-4-20250514`, `claude-opus-4-6-20250507` |

### Why a separate provider instead of reusing OpenAI?

While llama.cpp server and some gateways expose Anthropic models through an
OpenAI-compatible shim, the **native Anthropic Messages API** has distinct
features that the OpenAI format cannot express:

1. **Extended Thinking** — Claude 3.7+ supports a `thinking` parameter that
   makes the model produce internal reasoning before answering. The thinking
   content arrives as separate `thinking` type content blocks in the response.
2. **Native tool use format** — Anthropic's tool_use/tool_result content blocks
   are structurally different from OpenAI's tool_calls array.
3. **Native multimodal** — Image input uses a different content block format
   with inline base64 data and explicit media_type.
4. **Different streaming protocol** — Anthropic uses named SSE events
   (`message_start`, `content_block_delta`, etc.) rather than anonymous
   `data:` lines.
5. **Different authentication** — Requires both `x-api-key` and
   `anthropic-version` headers.

The `AnthropicMessagesProvider` implements the native protocol directly,
giving access to the full Claude feature set.

### Supported capabilities

- **Text generation** — single-turn and multi-turn conversations
- **Streaming** — token-by-token via named SSE events
- **Tool use** — define tools, parse tool_use blocks, format tool_result blocks
- **Extended thinking** — configure thinking budget for Claude 3.7+ models
- **Multimodal input** — image content blocks with base64 data
- **System prompts** — native system field (string or multimodal content array)
- **Retry** — exponential backoff on 429/5xx with `Retry-After` parsing
- **Model discovery** — via Anthropic's `/v1/models` endpoint

**Iron laws from `requirements.md` apply** — no tokio/async-trait, Valtron-only
async, zero warnings, `derive_more::From` + manual `Display` errors.

## Local File Paths

- Spec: `specifications/07-foundation-ai/features/07-anthropic-provider/feature.md`
- Provider impl: `backends/foundation_ai/src/backends/anthropic_messages_provider.rs`
- Backend registration: `backends/foundation_ai/src/backends/mod.rs`
- Types: `backends/foundation_ai/src/types/mod.rs`
- Errors: `backends/foundation_ai/src/errors/mod.rs`
- Model descriptors: `backends/foundation_ai/src/models/model_descriptors.rs`
- Tests: `backends/foundation_ai/tests/anthropic_messages_provider.rs`
- Reference: `backends/foundation_ai/src/backends/openai_provider.rs` (pattern to follow)

## Task Group 1: Configuration and request/response types

### AnthropicConfig

```rust
/// Configuration for the Anthropic Messages API provider.
pub struct AnthropicConfig {
    pub base_url: String,          // default: "https://api.anthropic.com"
    pub api_version: String,       // default: "2023-06-01" (latest stable)
    pub timeout_secs: u64,         // default: 120 (reasoning models can be slow)
    pub max_retries: u32,          // default: 3
    pub proxy_url: Option<String>,
    pub streaming: bool,           // default: true
    pub auth: Option<AuthCredential>,
}
```

Key differences from `OpenAIConfig`:
- `api_version` maps to the `anthropic-version` header (required by Anthropic)
- Default timeout is 120s (vs 30s for OpenAI) because reasoning/thinking models
  can take much longer to produce their first token
- Auth is always API key (`x-api-key` header) — no Bearer token support

### Request types

```rust
/// POST /v1/messages request body.
#[derive(Serialize)]
pub struct MessagesRequest {
    pub model: String,

    /// System prompt — can be a string or array of content blocks
    /// (for multimodal system prompts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<AnthropicSystemContent>,

    /// Conversation messages.
    pub messages: Vec<AnthropicMessage>,

    /// Maximum tokens to generate.
    pub max_tokens: u32,

    /// Sampling temperature (0.0–1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Nucleus sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,

    /// Top-k sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,

    /// Enable streaming.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Stop sequences that will truncate generation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,

    /// Available tools for the model to call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,

    /// Tool choice configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<AnthropicToolChoice>,

    /// Extended thinking configuration (Claude 3.7+).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingConfig>,
}
```

### Message and content types

Anthropic messages have a **content array** model — even plain text is wrapped
in a single-element array. This is fundamentally different from OpenAI where
`content` can be a simple string.

```rust
/// Role for Anthropic messages: "user", "assistant", or "tool".
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnthropicRole {
    User,
    Assistant,
    // Anthropic doesn't have a "tool" role — tool results are
    // sent as user messages with tool_result content blocks.
}

/// A single message in the conversation.
#[derive(Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: AnthropicRole,
    pub content: Vec<AnthropicContentBlock>,
}

/// Content blocks within a message. Anthropic uses a block-based content model
/// where each block has a specific type and structure.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicContentBlock {
    Text {
        text: String,
    },
    Image {
        source: AnthropicImageSource,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking {
        data: String,
    },
}

/// Image source for Anthropic content blocks.
/// Only supports base64-encoded inline images (no URL references).
#[derive(Serialize, Deserialize)]
pub struct AnthropicImageSource {
    #[serde(rename = "type")]
    pub source_type: String,  // always "base64"
    pub media_type: String,    // "image/jpeg", "image/png", "image/gif", "image/webp"
    pub data: String,          // base64-encoded image data
}
```

### Tool types

Anthropic tools are defined at the request level and use JSON Schema:

```rust
#[derive(Serialize)]
pub struct AnthropicTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema object
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicToolChoice {
    Auto,                   // model decides
    Any,                    // must use at least one tool
    Tool { name: String },  // force specific tool
}
```

### Extended thinking

Claude 3.7+ supports "extended thinking" — the model produces internal reasoning
before answering. This is different from OpenAI's `o1` reasoning because:
- The thinking content is exposed in the response as separate content blocks
- You control the max thinking tokens separately from output tokens
- Thinking tokens are billed separately and don't count toward max_tokens

```rust
#[derive(Serialize)]
pub struct AnthropicThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,  // "enabled"
    pub budget_tokens: u32,     // max tokens for thinking
}
```

### Response types

```rust
/// POST /v1/messages response body.
#[derive(Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,  // "message"
    pub role: String,            // "assistant"
    pub content: Vec<AnthropicContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,  // "end_turn", "stop_sequence", "max_tokens", "tool_use"
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
}

#[derive(Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}
```

### Streaming event types

Anthropic's SSE streaming uses **named events** (unlike OpenAI's anonymous
`data:` lines). Each event has an `event:` line followed by a `data:` line:

```
event: message_start
data: {"type":"message_start","message":{"id":"msg_123",...}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":15}}

event: message_stop
data: {"type":"message_stop"}
```

```rust
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: MessagesResponse,
    },
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: AnthropicDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaDelta,
        usage: AnthropicUsage,
    },
    MessageStop,
    Ping,  // periodic keepalive, can be ignored
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AnthropicDelta {
    TextDelta { text: String },
    ThinkingDelta { thinking: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Deserialize)]
pub struct MessageDeltaDelta {
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
}
```

## Task Group 2: Provider implementation

### AnthropicMessagesProvider

Follow the same pattern as `OpenAIProvider`:

```rust
pub struct AnthropicMessagesProvider<R: DnsResolver = SystemDnsResolver> {
    config: AnthropicConfig,
    api_key: Option<ConfidentialText>,
    http_client: Option<SimpleHttpClient<R>>,
    resolver: Option<R>,
    models_cache: Arc<Mutex<Vec<AnthropicModelInfo>>>,
}
```

Implement `ModelProvider` trait:
- `create()` — validate API key, build HTTP client
- `get_model()` — return `AnthropicModel` by name
- `describe()` — return provider metadata
- `list_models()` — fetch from `/v1/models` endpoint

### AnthropicModel

Implement `Model` trait:
- `generate()` — POST `/v1/messages`, parse `MessagesResponse`
- `stream()` — POST `/v1/messages` with `stream: true`, parse SSE events
- `get_state()` / `update_state()` — model lifecycle

### HTTP request building

Anthropic uses different auth headers than OpenAI:

```rust
fn build_request(&self, url: &str, body: &impl Serialize) -> Result<..., ...> {
    let json = serde_json::to_string(body)?;
    let mut headers = vec![
        ("Content-Type".into(), "application/json".into()),
        ("x-api-key".into(), self.api_key.expose_secret().into()),
        ("anthropic-version".into(), self.config.api_version.clone()),
    ];
    // ... build and return request
}
```

### Response parsing

Convert `AnthropicContentBlock` → `ModelOutput`:
- `Text` → `ModelOutput::Text(TextContent)`
- `ToolUse` → `ModelOutput::ToolCall { id, name, arguments, ... }`
- `Thinking` → `ModelOutput::ThinkingContent { thinking, signature }`
- `RedactedThinking` → `ModelOutput::ThinkingContent` (with redacted marker)

Map stop reasons:
- `end_turn` → `StopReason::Stop`
- `stop_sequence` → `StopReason::StopSequence`
- `max_tokens` → `StopReason::Length`
- `tool_use` → `StopReason::ToolUse`

### Streaming implementation

Parse Anthropic's named SSE events:
1. `message_start` → emit initial `Messages::Assistant` with metadata
2. `content_block_delta` (text_delta) → accumulate text
3. `content_block_delta` (thinking_delta) → accumulate thinking
4. `content_block_delta` (input_json_delta) → accumulate tool args
5. `message_delta` → update stop reason and usage
6. `message_stop` → finalize and emit complete message

Use `StreamIterator` / `ReconnectingEventSourceTask` same as `OpenAIStream`.

### Retry logic

Same exponential backoff pattern as `OpenAIProvider`:
- Retry on 429 (rate limit) — parse `Retry-After` header if present
- Retry on 5xx (server error)
- Max retries from config

### Error mapping

```
400 Bad Request        → GenerationError::InvalidRequest
401 Unauthorized       → GenerationError::Authentication
403 Forbidden          → GenerationError::Authorization
404 Not Found          → GenerationError::ModelNotFound
429 Rate Limit         → GenerationError::RateLimited
5xx Server Error       → GenerationError::Backend
```

## Task Group 3: build_anthropic_request helper

Convert `ModelInteraction` → `MessagesRequest`:

- `system_prompt` → `system` field (string, or multimodal array if needed)
- `messages` → map each message to `AnthropicMessage` with content blocks:
  - `Messages::User { content: Text }` → `AnthropicMessage { role: User, content: [Text] }`
  - `Messages::User { content: Image }` → `AnthropicMessage { role: User, content: [Image] }`
  - `Messages::User { content: Multimodal }` → `AnthropicMessage { role: User, content: [Text, Image, ...] }`
  - `Messages::Assistant { content: Text }` → `AnthropicMessage { role: Assistant, content: [Text] }`
  - `Messages::Assistant { content: ToolCall }` → `AnthropicMessage { role: Assistant, content: [ToolUse] }`
- `tools` → `AnthropicTool` definitions
- `tool_choice` → `AnthropicToolChoice`
- `params.max_tokens` → required `max_tokens` field (Anthropic requires this)
- `params.thinking_budget` → `thinking` config
- `params.temperature` → `temperature`
- `params.top_p` → `top_p`
- `params.top_k` → `top_k`

## Task Group 4: Integration tests

### Mock server tests (using TestHttpServer)

```rust
/// Test: generate — mock response with text content
#[test]
fn test_provider_generate()

/// Test: streaming text — mock SSE named events
#[test]
fn test_provider_streaming_text()

/// Test: tool use — mock response with tool_use content block
#[test]
fn test_provider_streaming_tool_calls()

/// Test: extended thinking — mock response with thinking block
#[test]
fn test_provider_generate_with_thinking()

/// Test: multimodal — mock request with image content
#[test]
fn test_provider_generate_multimodal()
```

### Integration tests against real Anthropic API (#[ignore]-gated)

```rust
/// Test: generate against real Anthropic API
#[test]
#[ignore]
fn test_anthropic_generate()

/// Test: streaming against real Anthropic API
#[test]
#[ignore]
fn test_anthropic_streaming()

/// Test: tool use against real Anthropic API
#[test]
#[ignore]
fn test_anthropic_tool_use()

/// Test: extended thinking (Claude 3.7+)
#[test]
#[ignore]
fn test_anthropic_thinking()

/// Test: multimodal input
#[test]
#[ignore]
fn test_anthropic_multimodal()
```

Requires environment variable `ANTHROPIC_TEST=1` and `ANTHROPIC_API_KEY`.

## Complete Task List

### Task Group 1: Types (8 tasks)

1. **[types]** Create `AnthropicConfig` struct with builder pattern (base_url, api_version, timeout_secs=120, max_retries, proxy_url, streaming, auth)
2. **[types]** Create `AnthropicRole`, `AnthropicMessage`, `AnthropicContentBlock` enums/structs with serde `tag = "type"` for content blocks
3. **[types]** Create `AnthropicImageSource` struct for base64 image content
4. **[types]** Create `AnthropicTool`, `AnthropicToolChoice` types for tool definitions
5. **[types]** Create `AnthropicThinkingConfig` for extended thinking
6. **[types]** Create `MessagesRequest` with all optional fields properly `skip_serializing_if`
7. **[types]** Create `MessagesResponse`, `AnthropicUsage` for response parsing
8. **[types]** Create `StreamEvent`, `AnthropicDelta`, `MessageDeltaDelta` for SSE streaming events

### Task Group 2: Provider (8 tasks)

9. **[provider]** Create `AnthropicMessagesProvider` struct with `Default`, builder methods
10. **[provider]** Implement `AuthProvider` trait on `AnthropicConfig`
11. **[provider]** Implement `ModelProvider` trait: `create()`, `get_model()`, `describe()`, `list_models()`
12. **[provider]** Implement model listing via `/v1/models` endpoint with caching
13. **[provider]** Create `AnthropicModel` struct implementing `Model` trait
14. **[provider]** Implement `AnthropicModel::generate()` — POST `/v1/messages`, parse response
15. **[provider]** Implement `AnthropicModel::stream()` — SSE with named event parsing
16. **[provider]** Create `AnthropicStream` iterator accumulating content blocks from events

### Task Group 3: Request building and parsing (7 tasks)

17. **[helper]** Implement `build_anthropic_request()` — convert `ModelInteraction` → `MessagesRequest`
18. **[helper]** Map `Messages::User/Assistant` to `AnthropicMessage` with proper content blocks
19. **[helper]** Map `[Image]` placeholder to `AnthropicImageSource` with base64 data URLs
20. **[helper]** Map tools from `ModelInteraction` to `AnthropicTool` definitions
21. **[helper]** Map `tool_choice` enum to `AnthropicToolChoice`
22. **[helper]** Implement `parse_response()` — convert `MessagesResponse` → `Vec<Messages>`
23. **[helper]** Map Anthropic stop reasons to `StopReason` enum variants

### Task Group 4: Error handling (3 tasks)

24. **[errors]** Implement HTTP status code → `GenerationError` mapping for Anthropic responses
25. **[errors]** Parse Anthropic error response format `{"type":"error","error":{"type":"...","message":"..."}}`
26. **[errors]** Implement retry with exponential backoff (429/5xx) including `Retry-After` header

### Task Group 5: Tests (5 tasks)

27. **[test]** Mock tests: generate, streaming text, streaming tool calls (using `TestHttpServer`)
28. **[test]** Mock test: extended thinking response parsing
29. **[test]** Mock test: multimodal request serialization
30. **[test]** Integration tests (#[ignore]-gated): generate, streaming, tool use against real API
31. **[test]** Integration test (#[ignore]-gated): extended thinking with Claude 3.7+ model

### Task Group 6: Wiring (1 task)

32. **[wire]** Register `anthropic_messages_provider` module in `backends/mod.rs`

## Execution Order

```
1–8   (types — all can be done together)
9–12  (provider skeleton — depends on types)
13–16 (provider logic — depends on provider skeleton + types)
17–23 (helpers — depends on types)
24–26 (error handling — depends on provider logic)
27–31 (tests — depends on everything above)
32    (registration — depends on provider implementation)
```

## Verification

```bash
# Compile check
cargo check --package foundation_ai

# Run unit tests
cargo test --package foundation_ai --lib -- anthropic_messages

# Run mock integration tests
cargo test --package foundation_ai --test anthropic_messages_provider

# Run real API tests (requires ANTHROPIC_API_KEY)
ANTHROPIC_TEST=1 ANTHROPIC_API_KEY=sk-ant-... cargo test --package foundation_ai --test anthropic_messages_provider -- --ignored
```
