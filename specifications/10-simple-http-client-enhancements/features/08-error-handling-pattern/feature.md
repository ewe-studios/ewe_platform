---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/08-error-handling-pattern"
this_file: "specifications/10-simple-http-client-enhancements/features/08-error-handling-pattern/feature.md"

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

# Error Handling Pattern

## Overview

This feature documents structured error types with `derive_more` and graceful degradation patterns for HTTP client operations.

## WHY: Problem Statement

Users need structured, actionable error reporting that:
1. Provides context about what operation failed
2. Supports automatic Display/From implementations
3. Enables graceful degradation vs. fail-fast behavior

Without structured errors:
- Error messages lack context (which URL failed?)
- Boilerplate for Display/From implementations
- Hard to distinguish error types programmatically
- Debugging requires tracing through multiple layers

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
use derive_more::{Display, From};

/// WHY: Provides structured, actionable error reporting for every failure mode
/// in the model-generation pipeline.
///
/// WHAT: Covers HTTP transport, JSON parsing, body handling, and file I/O.
///
/// HOW: Uses derive_more for Display/From to avoid boilerplate.
///
/// # Panics
///
/// Never panics.
#[derive(Debug, Display, From)]
pub enum GenModelError {
    /// HTTP client could not be constructed or the request failed.
    #[display("http error for {url}: {source}")]
    Http {
        url: String,
        source: foundation_core::wire::simple_http::HttpClientError,
    },

    /// Server returned a non-200 status code.
    #[display("http {status} from {url}")]
    BadStatus { url: String, status: usize },

    /// Response body was not UTF-8.
    #[display("response body from {url} is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        source: std::string::FromUtf8Error,
    },

    /// Response body was an unexpected variant (e.g. stream instead of text).
    #[display("unexpected response body type from {url}")]
    UnexpectedBody { url: String },

    /// JSON deserialization failed.
    #[display("json parse error for {url}: {source}")]
    Json {
        url: String,
        source: serde_json::Error,
    },

    /// Could not write the generated file.
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },
}

impl std::error::Error for GenModelError {}
```

### Graceful Degradation Pattern

```rust
// In .map_ready() closure
.map_ready(move |intro| {
    match intro {
        RequestIntro::Success { stream, .. } => {
            match read_and_parse(stream) {
                Ok(result) => result,
                Err(e) => {
                    tracing::warn!("Parse error: {e}");
                    Vec::new() // Return empty, don't fail entire operation
                }
            }
        }
        RequestIntro::Failed(e) => {
            tracing::warn!("Request failed: {e}");
            Vec::new() // Graceful degradation
        }
    }
})
```

## WHAT: Solution Overview

### Structured Error Types with derive_more

```rust
use derive_more::{Display, From};

/// Structured error types for HTTP client operations.
///
/// WHY: Provides structured, actionable error reporting.
///
/// WHAT: Covers HTTP transport, JSON parsing, body handling, and I/O.
///
/// HOW: Uses derive_more for Display/From to avoid boilerplate.
#[derive(Debug, Display, From)]
pub enum HttpClientError {
    /// HTTP client could not be constructed or the request failed.
    #[display("http error for {url}: {source}")]
    Http {
        url: String,
        source: crate::wire::simple_http::HttpClientError,
    },

    /// Server returned a non-200 status code.
    #[display("http {status} from {url}")]
    BadStatus { url: String, status: u16 },

    /// Response body was not UTF-8.
    #[display("response body from {url} is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        source: std::string::FromUtf8Error,
    },

    /// Response body was an unexpected variant.
    #[display("unexpected response body type from {url}")]
    UnexpectedBody { url: String },

    /// JSON deserialization failed.
    #[display("json parse error for {url}: {source}")]
    Json {
        url: String,
        source: serde_json::Error,
    },

    /// Request state is invalid for operation.
    #[display("invalid request state: {0}")]
    InvalidRequestState(String),

    /// No connection pool available.
    #[display("no connection pool available")]
    NoPool,

    /// Request execution failed.
    #[display("request failed: {0}")]
    FailedWith(Box<dyn std::error::Error + Send + Sync>),
}

impl std::error::Error for HttpClientError {}
```

### Error Design Principles

1. **Include Context**: Always include URL, source, or identifiers
2. **Use derive_more**: Reduce boilerplate with `#[derive(Display, From)]`
3. **Source Chaining**: Wrap underlying errors for debugging
4. **Graceful Degradation**: Log and continue vs. fail fast for parallel ops

### Graceful Degradation vs. Fail Fast

```rust
/// Pattern 1: Graceful Degradation (for parallel operations)
///
/// Use when: One failure shouldn't affect other operations
fn parse_with_fallback(body: &str, source: &str) -> Vec<ModelEntry> {
    match serde_json::from_str(body) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Failed to parse {source}: {e}");
            Vec::new() // Return empty, continue with other sources
        }
    }
}

/// Pattern 2: Fail Fast (for critical operations)
///
/// Use when: Failure means the entire operation should abort
fn parse_with_error(body: &str, source: &str) -> Result<Vec<ModelEntry>, HttpClientError> {
    serde_json::from_str(body).map_err(|e| HttpClientError::Json {
        url: source.to_string(),
        source: e,
    })
}
```

## HOW: Implementation

### Error Module Structure

```rust
// File: backends/foundation_core/src/wire/simple_http/errors.rs

//! Error types for HTTP client operations.
//!
//! WHY: Provides structured, actionable error reporting.
//!
//! WHAT: Covers HTTP transport, parsing, and I/O errors.
//!
//! HOW: Uses derive_more for Display/From implementations.

use derive_more::{Display, From};
use crate::wire::simple_http::HttpReaderError;

/// HTTP client errors.
#[derive(Debug, Display, From)]
pub enum HttpClientError {
    /// HTTP transport error.
    #[display("http error for {url}: {source}")]
    Http {
        url: String,
        source: crate::wire::simple_http::HttpClientError,
    },

    /// Server returned error status.
    #[display("http {status} from {url}")]
    BadStatus { url: String, status: u16 },

    /// Response body encoding error.
    #[display("response body from {url} is not valid UTF-8: {source}")]
    InvalidUtf8 {
        url: String,
        source: std::string::FromUtf8Error,
    },

    /// Unexpected body type.
    #[display("unexpected response body type from {url}")]
    UnexpectedBody { url: String },

    /// JSON parsing error.
    #[display("json parse error for {url}: {source}")]
    Json {
        url: String,
        source: serde_json::Error,
    },

    /// Stream reading error.
    #[display("stream read error for {url}: {source}")]
    ReaderError {
        url: String,
        source: HttpReaderError,
    },

    /// Invalid request state.
    #[display("invalid request state: {0}")]
    InvalidRequestState(String),

    /// No connection pool.
    #[display("no connection pool available")]
    NoPool,

    /// Request failed.
    #[display("request failed: {0}")]
    FailedWith(String),

    /// Too many redirects.
    #[display("too many redirects for {url}")]
    TooManyRedirects { url: String },

    /// Retry needed.
    #[display("retry needed for {url} (attempt {attempt})")]
    RetryNeeded { url: String, attempt: u32 },
}

impl std::error::Error for HttpClientError {}

/// Parse errors for response parsing.
#[derive(Debug, Display)]
pub enum ParseError {
    /// Invalid JSON format.
    #[display("invalid JSON: {0}")]
    InvalidJson(serde_json::Error),

    /// Missing required field.
    #[display("missing field: {0}")]
    MissingField(String),

    /// Invalid field value.
    #[display("invalid value for field {field}: {reason}")]
    InvalidValue { field: String, reason: String },
}

impl std::error::Error for ParseError {}

impl From<serde_json::Error> for ParseError {
    fn from(err: serde_json::Error) -> Self {
        Self::InvalidJson(err)
    }
}
```

### Usage Examples

```rust
use foundation_core::wire::simple_http::errors::{HttpClientError, ParseError};

// Example 1: Constructing errors with context
fn fetch_url(url: &str) -> Result<String, HttpClientError> {
    let response = client.get(url).send().map_err(|e| HttpClientError::Http {
        url: url.to_string(),
        source: e,
    })?;
    Ok(response.body().to_string())
}

// Example 2: Graceful degradation in parser
fn parse_response(body: &str, source: &str) -> Vec<ModelEntry> {
    match serde_json::from_str(body) {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("Failed to parse {source}: {e}");
            Vec::new()
        }
    }
}

// Example 3: Fail fast with structured error
fn parse_response_strict(body: &str, source: &str) -> Result<Vec<ModelEntry>, HttpClientError> {
    serde_json::from_str(body).map_err(|e| HttpClientError::Json {
        url: source.to_string(),
        source: e,
    })
}

// Example 4: Error conversion with ?
fn process_response(body: &str) -> Result<ModelEntry, ParseError> {
    let data: ModelEntry = serde_json::from_str(body)?; // Automatically converts
    Ok(data)
}
```

### Error Reporting Pattern

```rust
use foundation_core::wire::simple_http::errors::HttpClientError;

/// Report error with full context.
fn report_error(err: &HttpClientError) {
    match err {
        HttpClientError::Http { url, source } => {
            tracing::error!("HTTP error for {}: {}", url, source);
        }
        HttpClientError::BadStatus { url, status } => {
            tracing::error!("Bad status {} from {}", status, url);
        }
        HttpClientError::Json { url, source } => {
            tracing::error!("JSON parse error for {}: {}", url, source);
        }
        _ => {
            tracing::error!("Error: {}", err);
        }
    }
}
```

## Implementation Location

```
backends/foundation_core/src/wire/simple_http/
└── errors.rs (MODIFY - Add structured error types)
```

## Success Criteria

- [ ] Structured error types documented
- [ ] derive_more usage shown
- [ ] Source identification pattern covered
- [ ] Graceful degradation vs. fail-fast explained
- [ ] Display/From implementations demonstrated
- [ ] Error conversion with ? operator shown
- [ ] Error reporting pattern documented

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::errors

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **Context is King**: Always include enough context (URL, source, operation) to identify what failed without additional debugging.

2. **Error Chaining**: Use `source` fields to preserve the original error for debugging.

3. **Display Formatting**: Make Display output human-readable and suitable for logging.

4. **From Implementations**: Use `#[from]` attribute for automatic conversion of common error types.

### Common Pitfalls

1. Not including context (which URL failed?)
2. Losing the original error (can't debug root cause)
3. Using panic instead of proper error handling
4. Not implementing Display for custom errors
5. Too many error variants (consider grouping related errors)

---

_Created: 2026-03-25_
_Source: gen_model_descriptors error handling analysis_
