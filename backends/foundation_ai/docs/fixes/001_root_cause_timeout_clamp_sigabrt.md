# 001 — Timeout clamp panic causing SIGABRT in provider tests

## Symptom

All provider tests using a read timeout greater than 60 seconds (Anthropic: 120s, OpenAI Responses: 120s) crashed with:

```
fatal runtime error: failed to initiate panic, error 5, aborting
```

The process received SIGABRT. The crash was non-deterministic — it occurred during valtron pool shutdown when a worker thread tried to clean up an in-flight HTTP request.

## Root cause

`SimpleHttpClient::read_timeout()` in `foundation_netio` only set `min_read_timeout` but left `max_read_timeout` at its default (60s). When `TimeoutCalculator::clamp()` was called internally, it asserted `min <= max`. With `min_read_timeout = 120s` and `max_read_timeout = 60s`, the clamp panicked.

The panic happened on a valtron worker thread. During pool shutdown, the worker was already being killed, so the cleanup path hit the clamp assertion. Since the thread was already unwinding from the kill signal, this triggered a double-panic, which Rust aborts with SIGABRT.

## Fix

**File:** `backends/foundation_netio/src/simple_http/client/native/client.rs` — `read_timeout()` method

When setting `min_read_timeout`, also bump `max_read_timeout` if it would be lower:

```rust
pub fn read_timeout(mut self, timeout: Duration) -> Self {
    let mut config = *self.config.timeout_calculator.config();
    config.min_read_timeout = timeout;
    if config.max_read_timeout < timeout {
        config.max_read_timeout = timeout;
    }
    self.config.timeout_calculator = TimeoutCalculator::with_config(config);
    self
}
```

## Affected providers

Any provider that sets a read timeout above the default `max_read_timeout` (60s):

- `AnthropicMessagesProvider` — `timeout_secs: 120`
- `OpenAIResponsesProvider` — `timeout_secs: 120`
