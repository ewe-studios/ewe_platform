---
workspace_name: "ewe_platform"
spec_directory: "specifications/07-foundation-ai"
feature_directory: "specifications/07-foundation-ai/features/08-tool-arguments-refactor"
this_file: "specifications/07-foundation-ai/features/08-tool-arguments-refactor/feature.md"

feature: "Tool Arguments Refactor"
description: "Replace Tool.arguments/Tool.returns HashMap with ordered Vec<Args> enum supporting Named/Unnamed parameters, and add ArgType::JSONMap for nested structured objects"
status: complete
priority: medium
depends_on:
  - "01-llamacpp-integration"
  - "00c-openai-provider"
  - "07-anthropic-provider"
estimated_effort: "small"
created: 2026-04-28
last_updated: 2026-04-28
author: "Main Agent"

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# Tool Arguments Refactor

## Overview

Replace `Tool.arguments` and `Tool.returns` from `Option<HashMap<String, ArgType>>`
to `Option<Vec<Args>>`. The `Args` enum supports both named and unnamed parameters,
preserving insertion order and supporting providers that use positional arguments.

Add `ArgType::JSONMap` for nested structured objects where a `HashMap<String, ArgType>`
is needed as a value type (distinct from `ArgType::JSON(String)` which is opaque raw JSON).

### Current design (problem)

```rust
pub struct Tool {
    pub arguments: Option<HashMap<String, ArgType>>,
    pub returns: Option<HashMap<String, ArgType>>,
}
```

Problems:
- `HashMap` loses insertion order — non-deterministic serialization
- No support for unnamed/positional parameters
- No structured nested objects — `ArgType::JSON(String)` is an opaque blob

### New design

```rust
/// A single tool argument parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Args {
    Named(String, ArgType),   // "location" → Text("Paris")
    Unnamed(ArgType),         // positional arg
}
```

```rust
pub struct Tool {
    pub arguments: Option<Vec<Args>>,
    pub returns: Option<Vec<Args>>,
}
```

### ArgType::JSONMap

Add a new `ArgType` variant for nested structured objects:

```rust
pub enum ArgType {
    // ... existing variants ...
    JSON(String),                           // opaque raw JSON string
    JSONMap(HashMap<String, ArgType>),      // nested named sub-fields
}
```

## Provider Conversion

Providers convert `Vec<Args>` → JSON Schema `properties` object:

```rust
// Anthropic / OpenAI tool conversion:
for arg in &tool.arguments {
    match arg {
        Args::Named(key, value) => {
            // maps to {"properties": {"key": {"type": ...}}}
        }
        Args::Unnamed(_) => {
            // positional — providers that don't support unnamed params
            // can skip or use generic param names ("arg0", "arg1", ...)
        }
    }
}
```

## Affected files

- `backends/foundation_ai/src/types/mod.rs` — `Args` enum, `ArgType::JSONMap`, `Tool` struct
- `backends/foundation_ai/src/backends/anthropic_messages_provider.rs` — tool schema conversion
- `backends/foundation_ai/src/backends/openai_provider.rs` — tool schema conversion
- `backends/foundation_ai/tests/anthropic_messages_provider.rs` — test updates

## Task List

1. **[types]** Add `Args` enum (`Named`, `Unnamed`) in `types/mod.rs`
2. **[types]** Add `ArgType::JSONMap(HashMap<String, ArgType>)` variant
3. **[types]** Change `Tool.arguments` / `Tool.returns` to `Option<Vec<Args>>`
4. **[providers]** Update OpenAI and Anthropic tool conversion code to match on `Args`
