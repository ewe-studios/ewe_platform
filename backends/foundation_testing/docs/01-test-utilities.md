# Fundamentals 01 — Test utilities and mock providers

Zero-to-expert on testing infrastructure.

---

## 1. MockModelProvider

A scripted model provider for testing:

```rust
let mut mock = MockModelProvider::new();
mock.on(|interaction| true, vec![mock_text("hello")]);

// Or use nth-call matching:
mock.on_nth_call(0, vec![mock_text("first response")])
    .on_nth_call(1, vec![mock_text("second response")])
    .on_any(vec![mock_text("fallback")]);

// Or inject failures:
mock.fail_with(|_| true, "injected error");
```

## 2. MockTool

A scripted tool implementation:

```rust
// Always succeeds with a result:
let tool = MockTool::returning("echo", "echoed");

// Always fails:
let tool = MockTool::failing("broken", "always fails");

// Fails N times then succeeds:
let tool = MockTool::new(
    "flaky",
    ToolBehavior::FailsThenSucceeds {
        failures: 2,
        error: ToolError::Execution { tool: "flaky".into(), reason: "transient".into() },
        result: ToolCallResult { content: UserModelContent::Text(...), error_detail: None },
    },
);
```

## 3. Message builders

```rust
mock_user("fix the bug")          // User message
mock_assistant("I'll help")       // Assistant text response
mock_tool_call("read_file", args) // Assistant tool call
mock_tool_result(id, "content")   // Tool result
```

## 4. Integration test patterns

### Full agent session test
```rust
#[test]
fn agent_responds_to_user_message() {
    let mut mock = MockModelProvider::new();
    mock.on_any(vec![mock_text("Hello!")]);

    let session = AgentSession::builder()
        .provider(Arc::new(mock))
        .build();

    let result = futures_lite::future::block_on(session.send("Hi"));
    assert!(result.is_ok());
}
```

### Tool execution test
```rust
#[test]
fn tool_call_executes_correctly() {
    let tool = MockTool::returning("read_file", "file contents");
    let result = futures_lite::future::block_on(
        tool.execute(HashMap::from([("path".into(), ArgType::Text("/test.txt".into()))]))
    );
    assert!(result.is_ok());
}
```

## 5. Test feature flag

`foundation_ai`'s `testing` feature enables:
- `MockModelProvider`
- `MockTool`
- Message builder functions
- `into_router()` on MockModelProvider for routing tests

Tests that need these must specify `required-features = ["testing"]`.

## 6. Call counting

```rust
assert_eq!(mock.call_count(), 0);
let _ = mock.resolve(&interaction);
assert_eq!(mock.call_count(), 1);
```

Useful for verifying the agent called the model the expected number of times.
