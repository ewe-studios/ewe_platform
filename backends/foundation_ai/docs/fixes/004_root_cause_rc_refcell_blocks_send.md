# 001 — Rc<RefCell<T>> makes all model backends !Send

## Symptom

`AgentLoop` cannot be scheduled on valtron's multi-thread executor (`execute()`
requires `T: Send` when the `multi` feature is on). Compilation fails with:

```
the trait `Send` is not implemented for `Rc<RefCell<LlamaModelsInner>>`
```

The same error surfaces for every backend: llamacpp, candle, openai, anthropic.

## Root Cause

All four model backends used `Rc<RefCell<T>>` for interior mutability on their
model state and cost accumulators. `Rc` is `!Send` by design — it uses
non-atomic reference counting. Any type containing an `Rc` is also `!Send`,
which propagates up through:

1. `LlamaModels` / `CandleModels` / `OpenAIModel` / `AnthropicModel` — hold `Rc<RefCell<Inner>>`
2. `LlamaCppStream` / `CandleStream` / `OpenAIStream` / `AnthropicStream` — hold the model
3. `Model::stream()` returns `Box<dyn StreamIterator<...>>` — inherits `!Send`
4. `AgentLoopState::InnerGenerate::stream` — holds the stream box
5. `AgentLoop` — holds the state → `!Send`

## Fix

- Replaced `Rc<RefCell<T>>` with `Arc<Mutex<T>>` in all four backends
- Replaced `Rc<T>` with `Arc<T>` where used (e.g. `Rc<LlamaModel>`)
- Changed `.borrow()` / `.borrow_mut()` to `.lock().unwrap()`
- Added `+ Send` to `Model::stream()` return type in base_types.rs
- Added `+ Send` to `InnerGenerate::stream` field in agent_loop.rs

## Files Changed

- `backends/foundation_ai/src/backends/llamacpp.rs`
- `backends/foundation_ai/src/backends/candle.rs`
- `backends/foundation_ai/src/backends/openai_provider.rs`
- `backends/foundation_ai/src/backends/anthropic_messages_provider.rs`
- `backends/foundation_ai/src/backends/openai_responses_provider.rs` (return type only)
- `backends/foundation_ai/src/types/base_types.rs`
- `backends/foundation_ai/src/agentic/agent_loop.rs`

## Trade-offs

`Mutex::lock()` has slightly higher overhead than `RefCell::borrow()` (atomic
CAS vs no-op). In practice, these locks are uncontended — each stream is polled
sequentially by one thread — so the cost is negligible.
