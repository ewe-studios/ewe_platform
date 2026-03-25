---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/06-parser-function-pattern"
this_file: "specifications/10-simple-http-client-enhancements/features/06-parser-function-pattern/feature.md"

status: pending
priority: medium
created: "2026-03-25"
completed: null

depends_on: []

tasks:
  completed: 0
  uncompleted: 4
  total: 4
  completion_percentage: 0
---

# Parser Function Pattern

## Overview

This feature documents the separate parser function pattern for API-specific response parsing with graceful error handling.

## WHY: Problem Statement

Different APIs return different JSON formats. Users need a clean separation between:
1. HTTP fetch logic (transport layer)
2. Response parsing logic (domain layer)

Without this separation:
- Fetch logic becomes coupled to specific API formats
- Testing requires actual HTTP responses
- Parser logic can't be reused or tested independently

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
/// Parser function for models.dev API
fn parse_models_dev_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    // Parse the JSON and extract models
    let data: serde_json::Value = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse models.dev response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    // ... parsing logic ...
    models
}

/// Parser function for OpenRouter API
fn parse_openrouter_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    let data: OpenRouterResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse OpenRouter response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    // ... parsing logic ...
    models
}

/// Parser function for AI Gateway API
fn parse_ai_gateway_response(body: &str, _source: &'static str) -> Vec<ModelEntry> {
    let data: AiGatewayResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse AI Gateway response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    // ... parsing logic ...
    models
}
```

### Parser Signature Pattern

```rust
fn parser(body: &str, source: &'static str) -> Vec<ModelEntry>
//    │            │                │
//    │            │                └─ Source identifier for logging
//    │            └─ Response body to parse
//    └─ Returns domain type (or empty on error)
```

## WHAT: Solution Overview

Separate parser functions with consistent signature:

### Core Pattern

```rust
/// Parser function signature pattern
///
/// # Arguments
/// * `body` - Response body to parse
/// * `source` - Source identifier for logging/debugging
///
/// # Returns
/// Parsed domain entities. Returns empty Vec on error (graceful degradation).
fn parse_api_response(body: &str, source: &'static str) -> Vec<MyType> {
    // Parse JSON
    let data: serde_json::Value = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse {source} response: {e}");
            return Vec::new(); // Graceful degradation
        }
    };

    // Extract domain entities
    let mut results = Vec::new();
    // ... parsing logic ...
    results
}
```

### Parser with Strongly-Typed Response

```rust
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct OpenRouterResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModel {
    id: String,
    name: Option<String>,
    context_length: Option<u64>,
    pricing: Option<OpenRouterPricing>,
}

/// Parse OpenRouter API response
fn parse_openrouter_response(body: &str, source: &'static str) -> Vec<ModelEntry> {
    // Parse to strongly-typed structure
    let data: OpenRouterResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse {source} response: {e}");
            return Vec::new();
        }
    };

    // Transform to domain type
    data.data.iter()
        .filter(|m| is_tool_capable(m))
        .map(|m| ModelEntry {
            id: m.id.clone(),
            name: m.name.clone().unwrap_or(m.id.clone()),
            context_window: m.context_length.unwrap_or(4096) as u32,
            // ...
        })
        .collect()
}

fn is_tool_capable(model: &OpenRouterModel) -> bool {
    model.supported_parameters
        .as_ref()
        .is_some_and(|p| p.iter().any(|s| s == "tools"))
}
```

### Parser with Optional Fields Handling

```rust
/// Parse AI Gateway API response with optional fields
fn parse_ai_gateway_response(body: &str, source: &'static str) -> Vec<ModelEntry> {
    #[derive(Deserialize)]
    struct AiGatewayResponse {
        data: Option<Vec<AiGatewayModel>>,
    }

    #[derive(Deserialize)]
    struct AiGatewayModel {
        id: String,
        name: Option<String>,
        context_window: Option<u64>,
        tags: Option<Vec<String>>,
        pricing: Option<AiGatewayPricing>,
    }

    let data: AiGatewayResponse = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse {source} response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();
    for m in data.data.iter().flat_map(|d| d.iter()) {
        // Filter by tags
        let tags = m.tags.as_deref().unwrap_or(&[]);
        if !tags.iter().any(|t| t == "tool-use") {
            continue;
        }

        // Parse pricing with fallback
        let cost = parse_pricing(m.pricing.as_ref());

        models.push(ModelEntry {
            id: m.id.clone(),
            name: m.name.clone().unwrap_or(m.id.clone()),
            context_window: m.context_window.unwrap_or(4096) as u32,
            cost_input: cost.0,
            cost_output: cost.1,
            // ...
        });
    }

    models
}

fn parse_pricing(pricing: Option<&AiGatewayPricing>) -> (f64, f64, f64, f64) {
    let p = match pricing {
        Some(p) => p,
        None => return (0.0, 0.0, 0.0, 0.0),
    };

    (
        parse_price_value(p.input.as_ref()),
        parse_price_value(p.output.as_ref()),
        parse_price_value(p.input_cache_read.as_ref()),
        parse_price_value(p.input_cache_write.as_ref()),
    )
}

fn parse_price_value(value: Option<&serde_json::Value>) -> f64 {
    match value {
        Some(serde_json::Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}
```

### Parser with Validation

```rust
/// Parse with validation logic
fn parse_models_dev_response(body: &str, source: &'static str) -> Vec<ModelEntry> {
    #[derive(Deserialize)]
    struct ModelsDevProvider {
        models: Option<BTreeMap<String, ModelsDevModel>>,
    }

    #[derive(Deserialize)]
    struct ModelsDevModel {
        name: Option<String>,
        tool_call: Option<bool>,
        reasoning: Option<bool>,
        status: Option<String>,
        limit: Option<ModelsDevLimit>,
    }

    let data: ModelsDevProvider = match serde_json::from_str(body) {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to parse {source} response: {e}");
            return Vec::new();
        }
    };

    let mut models = Vec::new();

    // Iterate and validate each model
    for (id, model) in data.models.iter().flat_map(|m| m.iter()) {
        // Skip deprecated models
        if model.status.as_deref() == Some("deprecated") {
            continue;
        }

        // Skip models without tool support
        if model.tool_call != Some(true) {
            continue;
        }

        // Parse limits with defaults
        let context_window = model.limit
            .as_ref()
            .and_then(|l| l.context)
            .unwrap_or(4096) as u32;

        models.push(ModelEntry {
            id: id.clone(),
            name: model.name.clone().unwrap_or(id.clone()),
            reasoning: model.reasoning == Some(true),
            context_window,
            // ...
        });
    }

    models
}
```

## HOW: Implementation

### Parser Module Structure

```rust
// File: backends/foundation_core/src/wire/simple_http/client/parsers.rs

//! Response parser utilities for HTTP clients.
//!
//! WHY: Provides reusable parser patterns for API responses.
//!
//! WHAT: Exports parser trait and helper functions.
//!
//! HOW: Uses serde for deserialization with graceful error handling.

use serde::de::DeserializeOwned;
use tracing;

/// Parser trait for HTTP response bodies.
///
/// Implement this trait for types that can be parsed from HTTP response bodies.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::parsers::ResponseParser;
///
/// #[derive(serde::Deserialize)]
/// struct MyResponse {
///     data: Vec<Item>,
/// }
///
/// impl ResponseParser for MyResponse {
///     type Output = Vec<Item>;
///
///     fn parse(body: &str, source: &str) -> Self::Output {
///         match serde_json::from_str::<Self>(body) {
///             Ok(response) => response.data,
///             Err(e) => {
///                 tracing::error!("Failed to parse {source}: {e}");
///                 Vec::new()
///             }
///         }
///     }
/// }
/// ```
pub trait ResponseParser: Sized {
    /// The output type after parsing.
    type Output;

    /// Parse a response body.
    ///
    /// # Arguments
    /// * `body` - Response body to parse
    /// * `source` - Source identifier for logging
    ///
    /// # Returns
    /// Parsed output. Returns default/empty on error (graceful degradation).
    fn parse(body: &str, source: &str) -> Self::Output;
}

/// Generic JSON parser helper.
///
/// # Type Parameters
/// * `T` - Target type for deserialization
///
/// # Arguments
/// * `body` - Response body to parse
/// * `source` - Source identifier for logging
///
/// # Returns
/// `Some(T)` on success, `None` on error.
pub fn parse_json<T: DeserializeOwned>(body: &str, source: &str) -> Option<T> {
    match serde_json::from_str::<T>(body) {
        Ok(data) => Some(data),
        Err(e) => {
            tracing::error!("Failed to parse JSON from {source}: {e}");
            None
        }
    }
}

/// Parse with fallback on error.
///
/// # Arguments
/// * `body` - Response body to parse
/// * `source` - Source identifier for logging
/// * `fallback` - Function to produce fallback value on error
///
/// # Returns
/// Parsed value or fallback result.
pub fn parse_with_fallback<T, F>(body: &str, source: &str, fallback: F) -> T
where
    T: DeserializeOwned,
    F: FnOnce() -> T,
{
    match serde_json::from_str::<T>(body) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Failed to parse {source}: {e}");
            fallback()
        }
    }
}
```

### Usage Examples

```rust
// Using the parser trait
use foundation_core::wire::simple_http::client::parsers::{ResponseParser, parse_json};

#[derive(serde::Deserialize)]
struct ApiResponse {
    data: Vec<ModelEntry>,
}

impl ResponseParser for ApiResponse {
    type Output = Vec<ModelEntry>;

    fn parse(body: &str, source: &str) -> Self::Output {
        match parse_json::<Self>(body, source) {
            Some(response) => response.data,
            None => Vec::new(),
        }
    }
}

// Use with create_fetch_task
let task = create_fetch_task(
    &mut client,
    "api.example.com",
    "https://api.example.com/models",
    |body, source| ApiResponse::parse(body, source),
);
```

## Success Criteria

- [ ] Parser signature documented: `fn(&str, &'static str) -> T`
- [ ] Graceful error handling with logging demonstrated
- [ ] Empty/default return on error (not panic)
- [ ] Source parameter for debugging shown
- [ ] Multiple parser examples provided (different APIs)
- [ ] Strongly-typed parsing demonstrated
- [ ] Optional fields handling shown
- [ ] Validation logic pattern covered
- [ ] `ResponseParser` trait documented
- [ ] `parse_json` helper documented

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::client::parsers

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **Graceful Degradation**: Parsers should return empty/default values on error, not propagate errors. This allows parallel fetch to continue even if individual sources fail.

2. **Source Parameter**: Always include the source parameter for logging. It helps identify which API failed when debugging.

3. **Strong Typing**: Use strongly-typed structures (`#[derive(Deserialize)]`) rather than raw `serde_json::Value` when possible. This catches format errors early.

4. **Validation**: Filter and validate parsed data before returning. Don't trust external APIs to always return valid data.

5. **Optional Fields**: Handle optional fields gracefully with `.unwrap_or()` or `.unwrap_or_default()`.

### Common Pitfalls

1. Panicking on parse errors instead of graceful fallback
2. Not logging parse errors (makes debugging impossible)
3. Returning `Result` instead of using graceful degradation
4. Not handling optional/missing fields
5. Coupling parser logic to fetch logic (should be separate)

---

_Created: 2026-03-25_
_Source: gen_model_descriptors parser function analysis_
