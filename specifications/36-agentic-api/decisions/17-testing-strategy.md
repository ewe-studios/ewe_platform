# Decision 17: Testing Strategy

**Status:** Proposed  
**Date:** 2026-06-12  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem

The agentic API spans multiple components (LLM, tools, memory, queues, vector search) that need testing at multiple levels:
- Unit tests for individual components
- Integration tests for the full agentic loop
- Real model testing without external API dependencies
- Deterministic test scenarios for error handling and loop detection

## Decision

Testing uses the existing foundation_ai test infrastructure: **Candle** for pure-Rust model testing, **llama.cpp** for GGUF model testing, **TestHarness** for model downloads, and **MockModelProvider** for deterministic scenarios.

### Test Tiers

| Tier | What | How | When |
|------|------|-----|------|
| **Unit** | Config, builders, types, traits | Standard `#[test]`, no model needed | Always runs |
| **Integration** | Model loading, generation, tool execution | Candle or llama.cpp with downloaded model | `#[ignore]` — opt-in |
| **End-to-End** | Full agentic loop | llama.cpp with SmolLM2 + mock tools | `#[ignore]` — opt-in |
| **Deterministic** | Error handling, loop detection | MockModelProvider with scripted responses | Always runs |

### Model Testing

**Candle backend** (pure Rust, no external dependencies):
```rust
#[cfg(feature = "candle")]
#[test]
fn test_candle_generation() {
    let backend = CandleBackend::cpu();
    let config = CandleBackendConfig::builder().context_length(512).build();
    let provider = backend.create(Some(config)).unwrap();
    // Load a small model for testing...
}
```

**llama.cpp backend** (requires GGUF model):
```rust
#[test]
#[ignore = "requires a local GGUF model file"]
fn test_llama_generation() {
    let _guard = valtron::initialize_pool(42, Some(4));
    let harness = TestHarness::new(project_root);
    let model_path = harness.get_smollm_model().expect("Failed to download model");
    // Load model, test generation...
}
```

**TestHarness** downloads models from HuggingFace:
```rust
let harness = TestHarness::new(project_root);
let model_path = harness.get_smollm_model()?;  // SmolLM2-360M-Instruct-Q2_K.gguf (~150MB)
```

### Mock Model Provider

For deterministic testing (error handling, loop detection, tool call scenarios), a **MockModelProvider** returns scripted responses:

```rust
pub struct MockModelProvider {
    /// Scripted responses: input pattern → response
    responses: Vec<(Regex, Messages)>,
    /// Error injection: input pattern → error
    errors: Vec<(Regex, AgenticError)>,
}

impl MockModelProvider {
    /// Return a specific response when input matches the pattern
    pub fn on(&mut self, pattern: &str, response: Messages) -> &mut Self {
        self.responses.push((Regex::new(pattern).unwrap(), response));
        self
    }
    
    /// Inject an error when input matches the pattern
    pub fn fail_with(&mut self, pattern: &str, error: AgenticError) -> &mut Self {
        self.errors.push((Regex::new(pattern).unwrap(), error));
        self
    }
}

impl ModelProvider for MockModelProvider {
    fn get_model(&self, _model_id: ModelId) -> ModelProviderResult<Self::Model> {
        Ok(MockModel { 
            responses: self.responses.clone(), 
            errors: self.errors.clone() 
        })
    }
}
```

Usage in tests:
```rust
#[test]
fn test_circuit_breaker_model_failure() {
    let mut mock = MockModelProvider::new();
    // First 3 calls fail, then succeed
    mock.fail_with(".*", AgenticError::Generation(GenerationError::ProviderError("rate limit".into())));
    mock.fail_with(".*", AgenticError::Generation(GenerationError::ProviderError("rate limit".into())));
    mock.fail_with(".*", AgenticError::Generation(GenerationError::ProviderError("rate limit".into())));
    mock.on(".*", Messages::Assistant { content: ModelOutput::text("OK now"), .. });
    
    let agent = AgentSession::new(session_id, Arc::new(mock), config);
    // Run agent loop, verify circuit breaker trips and switches model
}
```

### Tool Testing

**Mock tools** for testing tool call execution:
```rust
pub struct MockTool {
    name: String,
    behavior: ToolBehavior,
}

pub enum ToolBehavior {
    /// Always returns this result
    Returns(ToolCallResult),
    /// Fails with this error
    Fails(ToolError),
    /// Fails N times, then succeeds
    FailsThenSucceeds { failures: u32, error: ToolError, result: ToolCallResult },
}
```

### Loop Detection Testing

Synthetic loop scenarios using MockModelProvider:
```rust
#[test]
fn test_loop_detection_exact_repetition() {
    let mut mock = MockModelProvider::new();
    // Return same response 3 times (loop)
    let same_response = Messages::Assistant {
        content: ModelOutput::text("I'll fix the bug by editing auth.rs"),
        ..
    };
    mock.on(".*", same_response.clone());
    
    let agent = AgentSession::new(session_id, Arc::new(mock), config);
    // Run agent loop, verify loop detector fires after 2 identical responses
}
```

### Vector Search Testing

In-memory vector store for testing:
```rust
#[test]
fn test_vector_search_recall() {
    let store = InMemoryVectorStore::new(dimensions=768);
    // Insert test vectors
    store.insert("msg1", vec![0.1, 0.2, ...], metadata);
    store.insert("msg2", vec![0.3, 0.4, ...], metadata);
    
    // Query and verify
    let results = store.query(vec![0.1, 0.2, ...], top_k=1);
    assert_eq!(results[0].id, "msg1");
}
```

### Session Resume Testing

```rust
#[test]
fn test_session_resume() {
    // Create session, run some turns, persist
    let session = AgentSession::new(session_id, provider.clone(), config);
    session.run_turn("Fix the auth bug").unwrap();
    session.end().unwrap();  // flushes everything
    
    // Resume session
    let resumed = AgentSession::resume(session_id, provider.clone(), config);
    // Verify working memory, reflections, last 10 messages loaded
    assert_eq!(resumed.working_memory.version, 1);
    assert_eq!(resumed.message_store.recent(10).len(), 10);
}
```

### Test Execution

```bash
# Fast unit tests (always run)
cargo test -p foundation_ai

# Integration tests (requires model download)
cargo test -p foundation_ai -- --ignored

# Candle-only tests (pure Rust, no llama.cpp needed)
cargo test -p foundation_ai --features candle

# All tests (includes model downloads)
cargo test -p foundation_ai --all-features -- --include-ignored
```

### Test Annotations

All valtron pool tests use the standard annotations:
```rust
#[test]
#[ntest::timeout(60_000)]           // 60s timeout
#[serial_test::serial]              // Global serialization (pool is global)
#[tracing_test::traced_test]        // Log visibility
fn test_name() {
    let _guard = init_pool();       // MUST be first line
    // ...
}
```

### WASM Testing

Tests that need to run in WASM use the same patterns but with WASM-compatible backends:
- In-memory vector store (no fjall)
- Mock model provider (no native model loading)
- Cloudflare D1 storage (native Turso not available)

## Rationale

**Why Candle + llama.cpp for real model testing?**
- Candle is pure Rust — no external dependencies, works everywhere
- llama.cpp supports GGUF models — widely available, small models for testing
- TestHarness downloads models automatically — no manual setup

**Why MockModelProvider for deterministic tests?**
- LLM responses are non-deterministic — can't assert on specific output
- Mock provider allows scripted scenarios: "return error 3 times, then succeed"
- Circuit breaker, loop detection, error handling all testable deterministically

**Why #[ignore] for integration tests?**
- Model downloads are slow (~150MB)
- Not all CI environments have model files
- Developers can opt-in when needed

## Alternatives Considered

### Cloud API testing (Anthropic, OpenAI)
- **Pros:** Tests real production models
- **Cons:** API costs, rate limits, non-deterministic responses, network dependency
- **Rejected because:** Local models (Candle, llama.cpp) are sufficient and free

### Fixture-based LLM response replay
- **Pros:** Deterministic, fast
- **Cons:** Fixtures go stale, don't test real model behavior
- **Used for:** MockModelProvider provides this pattern without maintaining fixture files
