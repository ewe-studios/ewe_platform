---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/00-foundation"
this_file: "specifications/05-openai-compatible-api/features/00-foundation/feature.md"

status: pending
priority: high
created: 2026-03-08

depends_on: []

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---


# Foundation Feature

## Overview

Create the foundational layer for the OpenAI-compatible API: error types, HTTP client integration, and base types that all subsequent features will build upon.

## Dependencies

This feature has no dependencies on other features.

This feature is required by:
- `chat-completions-types` - Uses error types and base types
- `responses-api` - Uses error types and HTTP client patterns
- `streaming-support` - Uses error types for stream errors

## Requirements

### Error Handling Pattern (MANDATORY)

All error types **MUST** follow this pattern:

```rust
use derive_more::From;
use foundation_core::simple_http::HttpClientError;

#[derive(From, Debug)]
pub enum ApiError {
    #[from(ignore)]
    InvalidApiKey(String),

    #[from(ignore)]
    InvalidBaseUrl(String),

    #[from(ignore)]
    SerializationFailed(String),

    #[from(ignore)]
    DeserializationFailed(String),

    #[from(ignore)]
    HttpRequestFailed(HttpClientError),

    #[from(ignore)]
    ApiError {
        code: Option<String>,
        message: String,
        error_type: Option<String>,
    },

    #[from(ignore)]
    RateLimitExceeded {
        retry_after: Option<u64>,
    },

    #[from(ignore)]
    ContextSizeExceeded {
        prompt_tokens: usize,
        context_limit: usize,
    },
}

impl std::error::Error for ApiError {}

impl core::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidApiKey(msg) => write!(f, "Invalid API key: {}", msg),
            Self::HttpRequestFailed(err) => write!(f, "HTTP request failed: {}", err),
            Self::ApiError { message, .. } => write!(f, "API error: {}", message),
            Self::RateLimitExceeded { retry_after } => {
                write!(f, "Rate limit exceeded{}", retry_after.map(|s| format!(", retry after {}s", s)).unwrap_or_default())
            }
            Self::ContextSizeExceeded { prompt_tokens, context_limit } => {
                write!(f, "Context size exceeded: {} tokens (limit: {})", prompt_tokens, context_limit)
            }
            // ... clear, descriptive messages for each variant
        }
    }
}
```

### HTTP Client Integration

1. **OpenAIClient struct**
   ```rust
   pub struct OpenAIClient {
       http_client: SimpleHttpClient,
       base_url: String,
       api_key: String,  // NOTE: Use String; consider zeroize crate for production
   }
   ```

   **Note**: `foundation_core` does not have a `Secret` type. API key should be handled carefully:
   - Clear documentation about secure handling
   - Consider adding `zeroize` crate dependency for secure string clearing
   - Never log API keys in error messages

2. **Builder pattern**
   ```rust
   impl OpenAIClient {
       pub fn builder() -> OpenAIClientBuilder {
           OpenAIClientBuilder::default()
       }
   }

   pub struct OpenAIClientBuilder {
       api_key: Option<String>,
       base_url: Option<String>,
       connect_timeout: Option<Duration>,
       read_timeout: Option<Duration>,
   }
   ```

3. **Default endpoints**
   - Chat Completions: `/v1/chat/completions`
   - Responses: `/v1/responses`
   - Base URL default: `https://api.openai.com`

### Base Types

1. **Role enum**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum Role {
       System,
       User,
       Assistant,
       Developer,  // For Responses API
       Tool,
   }
   ```

2. **Content types**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum Content {
       Text(String),
       Parts(Vec<ContentPart>),
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(tag = "type", rename_all = "snake_case")]
   pub enum ContentPart {
       Text { text: String },
       Image { image_url: ImageUrl },
       InputImage { image_url: String },  // Responses API
       InputText { text: String },        // Responses API
   }
   ```

3. **Message base type**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Message {
       pub role: Role,
       pub content: Content,
       pub name: Option<String>,
   }
   ```

4. **Usage tracking**
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Usage {
       pub prompt_tokens: usize,
       pub completion_tokens: usize,
       pub total_tokens: usize,
   }
   ```

### Common Parameters

All parameters that are shared between Chat Completions and Responses API:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommonParams {
    /// Controls randomness (0.0 - 2.0)
    pub temperature: Option<f32>,

    /// Nucleus sampling (0.0 - 1.0)
    pub top_p: Option<f32>,

    /// Frequency penalty (-2.0 - 2.0)
    pub frequency_penalty: Option<f32>,

    /// Presence penalty (-2.0 - 2.0)
    pub presence_penalty: Option<f32>,

    /// Stop sequences
    pub stop: Option<Vec<String>>,

    /// Random seed for reproducibility
    pub seed: Option<u32>,

    /// Max tokens to generate
    pub max_tokens: Option<usize>,
}
```

## Tasks

1. **Create error types module**
   - [ ] Create `backends/foundation_ai/src/openai/errors.rs`
   - [ ] Define `ApiError` enum with all variants
   - [ ] Implement `Display` and `Error` traits
   - [ ] Add conversion from `HttpClientError`

2. **Create base types module**
   - [ ] Create `backends/foundation_ai/src/openai/types.rs`
   - [ ] Define `Role` enum
   - [ ] Define `Content` and `ContentPart` types
   - [ ] Define `Message` struct
   - [ ] Define `Usage` struct
   - [ ] Define `CommonParams` struct

3. **Create HTTP client wrapper**
   - [ ] Create `backends/foundation_ai/src/openai/client.rs`
   - [ ] Define `OpenAIClient` struct
   - [ ] Implement builder pattern
   - [ ] Add API key handling (use String, document secure handling)
   - [ ] Configure default timeouts

4. **Create error response types**
   - [ ] Add `OpenAiErrorResponse` struct for API error parsing
   - [ ] Add `OpenAiError` struct with message, type, code fields
   - [ ] Add helper to convert to `ApiError`

5. **Create streaming error type**
   - [ ] Define `StreamingError` enum for stream-specific errors
   - [ ] Include `Done` variant for end-of-stream
   - [ ] Include `ParseError` for SSE parsing failures
   - [ ] Include `IncompleteStream` for unexpected termination

6. **Add serde dependencies**
   - [ ] Update `backends/foundation_ai/Cargo.toml` if needed
   - [ ] Ensure `serde` and `serde_json` are available
   - [ ] Consider adding `zeroize` crate for secure API key handling

7. **Create module structure**
   - [ ] Create `backends/foundation_ai/src/openai/mod.rs`
   - [ ] Export public types
   - [ ] Update `backends/foundation_ai/src/lib.rs`

8. **Write unit tests**
   - [ ] Test error type conversions
   - [ ] Test serialization/deserialization of base types
   - [ ] Test client builder
   - [ ] Test error response parsing

9. **Update documentation**
   - [ ] Add module-level documentation to `mod.rs`
   - [ ] Document all public types with examples

## Implementation Notes

- Use `derive_more` crate for `From` implementations (already in dependencies)
- No `Secret` type in foundation_core - use String with secure handling documentation
- Consider `zeroize` crate for production API key handling
- Reuse `foundation_core::simple_http::client::SimpleHttpClient`
- Reuse `foundation_core::event_source::SseParser` for SSE streaming
- Follow existing error handling patterns from `foundation_core`

## Success Criteria

- [ ] All 9 tasks completed
- [ ] `cargo check` passes with no warnings
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Unit tests pass
- [ ] Types serialize/deserialize correctly with OpenAI-compatible JSON

## Verification

```bash
# Build and check
cd backends/foundation_ai
cargo check
cargo clippy -- -D warnings
cargo fmt -- --check

# Run tests
cargo test openai::
```

---

_Created: 2026-03-08_
