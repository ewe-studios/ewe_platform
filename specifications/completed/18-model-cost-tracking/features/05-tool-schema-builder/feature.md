---
name: "Tool Schema Builder"
status: "completed"
priority: "high"
---

# Feature 05: Tool Schema Builder

## Summary

`Tool.arguments` and `Tool.returns` now hold a full JSON Schema document and a pre-built `ValidationOptions` (from `foundation_jsonschema::scheme`) instead of the previous `Vec<Args>` enum mapping. This enables expressive tool definitions with constraints (min, max, format, required, etc.) and on-demand validation.

## Design

### `Args` struct

```rust
pub struct Args {
    pub schema: serde_json::Value,          // The JSON Schema document
    pub validator: ValidationOptions,       // Pre-built, ready to .compile() → Validator
}

impl Args {
    pub fn new(opts: ValidationOptions) -> Self;        // From scheme builder
    pub fn from_value(value: serde_json::Value) -> Self; // From raw JSON
}
```

`Args` manually implements `Clone` (reconstructs `ValidationOptions` from the schema) and `PartialEq` (compares schemas).

### Usage

```rust
use foundation_jsonschema::scheme;

// Build with scheme builder
let opts = scheme::object()
    .required("query", scheme::string().min_len(1))
    .required("limit", scheme::integer().min(1).max(100))
    .optional("format", scheme::string().r#enum(vec!["json", "xml"]))
    .build();

let tool = Tool {
    id: "search".into(),
    name: "search".into(),
    description: "Search the web".into(),
    arguments: Some(Args::new(opts)),
    returns: None,
};

// Get the JSON Schema for API requests
let schema = &tool.arguments.as_ref().unwrap().schema;

// Validate tool call arguments at runtime
let validator = tool.arguments.as_ref().unwrap().validator.clone().compile().unwrap();
validator.validate(&call_args)?;
```

## Provider Integration

Each provider now uses `args.schema` directly for the `input_schema` / `parameters` field in API requests, instead of manually mapping `ArgType → "type"` strings. This reduces ~30 lines of duplicated code per provider.

- **Anthropic**: `input_schema` = `args.schema`
- **OpenAI**: `parameters` = `args.schema`
- **OpenAI Responses**: `parameters` = `args.schema`
- **llama.cpp / Candle**: Extract property names from `args.schema["properties"]` for text-based tool listing

## What Changed

| Before | After |
|--------|-------|
| `Tool.arguments: Option<Vec<Args>>` | `Tool.arguments: Option<Args>` |
| `Args` = enum of `Named(key, ArgType)` | `Args` = struct with `schema: Value, validator: ValidationOptions` |
| Per-provider ArgType→type mapping (~30 lines each) | Direct schema passthrough |
| No validation possible | `validator.compile()` → real JSON Schema validator |
