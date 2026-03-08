---
workspace_name: "ewe_platform"
spec_directory: "specifications/05-openai-compatible-api"
feature_directory: "specifications/05-openai-compatible-api/features/05-integration-tests"
this_file: "specifications/05-openai-compatible-api/features/05-integration-tests/feature.md"

status: pending
priority: medium
created: 2026-03-08

depends_on:
  - 02-chat-completions-client
  - 04-responses-api

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---


# Integration Tests Feature

## Overview

Create comprehensive integration tests for the OpenAI-compatible API client, including tests against mock servers and compatibility verification with llama.cpp server.

## Dependencies

This feature depends on:
- `02-chat-completions-client` - Tests chat completions functionality
- `04-responses-api` - Tests responses API functionality

## Requirements

### Test Server Mock

Create a minimal mock server for testing:

```rust
// tests/openai/mock_server.rs
struct MockServer {
    base_url: String,
    // Server handle
}

impl MockServer {
    fn start() -> Self { ... }
    fn base_url(&self) -> &str { ... }
}
```

### Chat Completions Tests

1. **Basic completion**
   ```rust
   #[test]
   fn test_chat_completion_basic() {
       let server = MockServer::start();
       let client = OpenAIClient::new("test-key", server.base_url());

       let request = ChatCompletionRequest::builder()
           .model("test-model")
           .message(Message::user("Hello"))
           .build();

       let response = client.chat_completions(request).await.unwrap();
       assert!(response.choices.len() > 0);
   }
   ```

2. **With system message**
   ```rust
   #[test]
   fn test_chat_completion_with_system() {
       // Test system prompt behavior
   }
   ```

3. **Streaming**
   ```rust
   #[test]
   fn test_chat_completion_stream() {
       // Test SSE streaming
       let mut stream = client.chat_completions_stream(request).unwrap();
       let mut content = String::new();
       while let Some(chunk) = stream.next() {
           if let Some(delta) = chunk.choices[0].delta.content.as_ref() {
               content.push_str(delta);
           }
       }
       assert!(!content.is_empty());
   }
   ```

4. **Error handling**
   ```rust
   #[test]
   fn test_invalid_api_key() {
       // Test 401 error
   }

   #[test]
   fn test_context_size_exceeded() {
       // Test 400 error with context exceeded
   }
   ```

5. **With tools/functions**
   ```rust
   #[test]
   fn test_tool_calling() {
       // Test function calling behavior
   }
   ```

### Responses API Tests

1. **Basic response**
   ```rust
   #[test]
   fn test_responses_basic() {
       let response = client.responses(request).await.unwrap();
       assert!(response.id.starts_with("resp_"));
   }
   ```

2. **Streaming events**
   ```rust
   #[test]
   fn test_responses_stream() {
       let mut stream = client.responses_stream(request).unwrap();
       let mut events = Vec::new();
       while let Some(event) = stream.next() {
           events.push(event.unwrap());
       }
       assert!(events.iter().any(|e| matches!(e, ResponseEvent::ResponseCompleted { .. })));
   }
   ```

3. **Input variations**
   ```rust
   #[test]
   fn test_responses_text_input() {
       // Test simple text input
   }

   #[test]
   fn test_responses_structured_input() {
       // Test structured input items
   }
   ```

### llama.cpp Integration Tests

Optional: Run tests against actual llama.cpp server:

```rust
// tests/openai/llama_cpp_integration.rs
// Only run with LLAMA_CPP_TEST=1 environment variable

#[test]
#[ignore]  // Requires running llama.cpp server
fn test_llama_cpp_chat_completion() {
    let client = OpenAIClient::new("dummy", "http://localhost:8080/v1");
    // Test against real server
}
```

## Tasks

1. **Create test infrastructure**
   - [ ] Create `tests/backends/foundation_ai/openai/` directory
   - [ ] Create mock server helper
   - [ ] Set up test utilities

2. **Write Chat Completions tests**
   - [ ] Basic completion tests
   - [ ] Streaming tests
   - [ ] Error handling tests
   - [ ] Tool calling tests

3. **Write Responses API tests**
   - [ ] Basic response tests
   - [ ] Streaming event tests
   - [ ] Input variation tests

4. **Add llama.cpp integration tests**
   - [ ] Create integration test file
   - [ ] Add environment variable gating
   - [ ] Document how to run

5. **Add CI configuration**
   - [ ] Add test job to CI pipeline
   - [ ] Configure llama.cpp server for integration tests
   - [ ] Add test reporting

## Success Criteria

- [ ] All 5 tasks completed
- [ ] All unit tests pass
- [ ] Mock server tests pass
- [ ] Integration tests documented

## Verification

```bash
cd backends/foundation_ai
cargo test --test openai_
```

---

_Created: 2026-03-08_
