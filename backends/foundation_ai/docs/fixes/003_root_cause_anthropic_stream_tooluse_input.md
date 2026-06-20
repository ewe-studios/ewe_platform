# 003 — Anthropic streaming silently dropped tool calls

## Symptom

`test_llama_server_anthropic_streaming` returned an empty text message with 0 output tokens. The stream yielded only `Ignore` items followed by a single final `Next` with `content: Text("")` — no text content, no tool calls.

## Root cause

`AnthropicContentBlock::ToolUse` required the `input` field:

```rust
ToolUse {
    id: String,
    name: String,
    input: serde_json::Value,  // required
}
```

In the Anthropic streaming protocol, `content_block_start` for a tool call does **not** include `input` — the input arrives incrementally via subsequent `content_block_delta` events with `input_json_delta` payloads. The server sends:

```json
{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"...","name":"shed"}}
```

No `input` field. Serde deserialization failed with `"missing field 'input'"`, so the `content_block_start` handler silently returned `Ignore`. Without the tool being registered in `self.tool_calls`, all subsequent `input_json_delta` events had nowhere to accumulate. At `message_stop`, both `accumulated_text` and `tool_calls` were empty, triggering the fallback path that emitted a single empty-text message.

## Fix

**File:** `backends/foundation_ai/src/backends/anthropic_messages_provider.rs` — `AnthropicContentBlock::ToolUse`

Added `#[serde(default)]` to the `input` field so it deserializes to `serde_json::Value::Null` when absent:

```rust
ToolUse {
    id: String,
    name: String,
    #[serde(default)]
    input: serde_json::Value,
}
```

The `input` field on `content_block_start` is only used in non-streaming responses (where the full input is present). In streaming, the actual input is assembled from `input_json_delta` chunks accumulated in `AccumulatedToolCall::arguments`.
