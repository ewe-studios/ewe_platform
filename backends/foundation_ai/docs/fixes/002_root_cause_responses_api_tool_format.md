# 002 — Responses API tool serialization using wrong format

## Symptom

Both `test_llama_server_responses_generate` and `test_llama_server_responses_stream` returned HTTP 500 from llama-server:

```
Failed to parse tools: key 'name' not found
```

## Root cause

`ResponseTool` serialized tools using the **Chat Completions** nested format instead of the **Responses API** flat format.

The struct had a nested `ResponseFunction` wrapper:

```rust
// Wrong — Chat Completions format
struct ResponseTool {
    type: "function",
    function: ResponseFunction {
        name: "shed",
        description: "...",
        parameters: {...},
    }
}
// Serialized as: {"type":"function","function":{"name":"shed",...}}
```

The Responses API expects tools at the top level:

```json
{"type":"function","name":"shed","description":"...","parameters":{...}}
```

The server couldn't find `name` because it was nested one level too deep under `function`.

## Fix

**File:** `backends/foundation_ai/src/backends/openai_responses_provider.rs` — `ResponseTool` struct

Flattened `ResponseFunction` fields directly into `ResponseTool` and removed the `ResponseFunction` struct:

```rust
pub struct ResponseTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}
```

Updated the construction site in `format_tools` to populate flat fields instead of wrapping in `ResponseFunction`.
