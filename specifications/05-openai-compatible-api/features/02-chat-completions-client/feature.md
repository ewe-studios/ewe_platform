---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/02-chat-completions-client"
this_file: "specifications/05-openai-compatible-api/features/02-chat-completions-client/feature.md"

status: pending
priority: high
created: 2026-03-08

depends_on:
  - 00-foundation
  - 01-chat-completions-types

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---


# Chat Completions Client Feature

## Overview

Implement the non-streaming Chat Completions API client that sends requests and receives complete responses.

## Dependencies

This feature depends on:
- `00-foundation` - Uses `ApiError`, `OpenAIClient`, base types
- `01-chat-completions-types` - Uses request/response types

This feature is required by:
- `05-integration-tests` - Uses client for end-to-end tests

## Requirements

### Client Implementation

```rust
impl OpenAIClient {
    /// Send a chat completion request and wait for the complete response.
    ///
    /// # Arguments
    ///
    /// * `request` - Chat completion request with messages and parameters
    ///
    /// # Returns
    ///
    /// * `Ok(ChatCompletionResponse)` - Complete response from the model
    /// * `Err(ApiError)` - Error if request fails or API returns error
    ///
    /// # Example
    ///
    /// ```ignore
    /// let request = ChatCompletionRequest::builder()
    ///     .model("gpt-4")
    ///     .message(Message::user("Hello!"))
    ///     .max_tokens(100)
    ///     .build();
    ///
    /// let response = client.chat_completions(request).await?;
    /// println!("{}", response.choices[0].message.content);
    /// ```
    pub async fn chat_completions(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ApiError> {
        // Implementation uses foundation_core HTTP client
    }
}
```

### HTTP Request Building

1. **URL construction**: Append `/v1/chat/completions` to base URL
2. **Headers**:
   - `Authorization: Bearer {api_key}`
   - `Content-Type: application/json`
3. **Body**: Serialize request to JSON

### Response Handling

1. **Success (200)**: Deserialize response body
2. **Error (4xx/5xx)**: Parse error response and return `ApiError`
3. **Error response format**:
   ```json
   {
     "error": {
       "message": "Invalid API key",
       "type": "invalid_request_error",
       "code": "invalid_api_key"
     }
   }
   ```

### Builder Convenience Methods

```rust
impl ChatCompletionRequest {
    pub fn builder() -> ChatCompletionRequestBuilder {
        ChatCompletionRequestBuilder::new()
    }
}

impl ChatCompletionRequestBuilder {
    pub fn model(mut self, model: impl Into<String>) -> Self { ... }
    pub fn message(mut self, message: Message) -> Self { ... }
    pub fn messages(mut self, messages: Vec<Message>) -> Self { ... }
    pub fn max_tokens(mut self, max: usize) -> Self { ... }
    pub fn temperature(mut self, temp: f32) -> Self { ... }
    pub fn build(self) -> ChatCompletionRequest { ... }
}
```

## Tasks

1. **Implement chat completions method**
   - [ ] Add `chat_completions()` to `OpenAIClient`
   - [ ] Build HTTP POST request
   - [ ] Send using `foundation_core::SimpleHttpClient`
   - [ ] Parse response and handle errors

2. **Implement error response parsing**
   - [ ] Create `ErrorResponse` type
   - [ ] Parse error responses into `ApiError`
   - [ ] Handle different error types appropriately

3. **Add builder convenience methods**
   - [ ] Create `ChatCompletionRequestBuilder`
   - [ ] Add ergonomic methods for construction
   - [ ] Support single message and message list

4. **Write integration tests**
   - [ ] Test successful completion
   - [ ] Test error handling (invalid key, bad request)
   - [ ] Test with various parameters

5. **Add documentation**
   - [ ] Document `chat_completions()` method
   - [ ] Add example usage
   - [ ] Document error conditions

## Success Criteria

- [ ] All 5 tasks completed
- [ ] Client successfully sends requests
- [ ] Responses correctly deserialized
- [ ] Errors properly handled
- [ ] Tests pass

## Verification

```bash
cd backends/foundation_ai
cargo test openai::chat::client
```

---

_Created: 2026-03-08_
