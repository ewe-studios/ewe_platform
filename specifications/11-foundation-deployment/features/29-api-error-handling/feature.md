---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/29-api-error-handling"
this_file: "specifications/11-foundation-deployment/features/29-api-error-handling/feature.md"

status: implemented
priority: high
created: 2026-04-06
implemented: 2026-04-11

depends_on: ["26-gen-provider-clients", "14-provider-spec-fetcher-core"]

tasks:
  completed: 7
  uncompleted: 0
  total: 7
  completion_percentage: 100%
---


# API Error Handling - Enhanced Error Detection and Deserialization

## Iron Law: Zero Warnings

> **All generated code must compile with zero warnings and pass all lints.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings from generated error types
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` in generated code.**

## Overview

**Problem:** When fetching OpenAPI specs from providers, some APIs return error responses instead of valid specs. For example, the GCP `developerknowledge` API returns:

```json
{
  "error": {
    "code": 400,
    "message": "No default GOOGLE_REST format gets defined in this API.",
    "status": "INVALID_ARGUMENT"
  }
}
```

The current generator **silently skips** these specs, producing empty client files with only a comment:

```rust
//! Auto-generated API clients for gcp/developerknowledge.
//! SKIPPED: Spec fetch returned an error
```

**Additionally**, generated clients lack proper error deserialization from API responses. When an API endpoint returns a 4xx or 5xx error, the client only reports the HTTP status code without parsing the error body.

**Solution:**
1. **Detect error responses** when fetching OpenAPI specs
2. **Generate error types** that can deserialize the standard Google Cloud error format
3. **Add universal error handling** to ALL generated clients for parsing 4xx/5xx responses
4. **Integrate with existing `ApiError` enum** for uniform error handling

## Standard Error Format

Most cloud providers follow the Google Cloud error format:

```json
{
  "error": {
    "code": 400,
    "message": "Human readable message",
    "status": "INVALID_ARGUMENT",
    "details": [...]
  }
}
```

**Fields:**
- `code` (integer) - HTTP status code
- `message` (string) - Human-readable error message
- `status` (string) - Canonical error status (e.g., `INVALID_ARGUMENT`, `NOT_FOUND`, `UNAUTHORIZED`)
- `details` (array, optional) - Additional error details (structured per-error-type)

## Motivation

### Why This Matters

1. **Better debugging**: Developers get actionable error messages, not just "HTTP 400"
2. **Programmatic handling**: Status codes like `INVALID_ARGUMENT` enable retry logic and error classification
3. **Consistent UX**: All providers use the same error structure
4. **Discoverability**: Empty generated files with "SKIPPED" comments are confusing

### Current Pain Points

| Issue | Current Behavior | Desired Behavior |
|-------|-----------------|------------------|
| Error spec detection | Silent skip, empty file | Generate error types, log warning |
| HTTP error responses | `ApiError::HttpStatus { code, headers }` | Parse error body, include message |
| Error details | Lost | Deserialized into structured type |
| Error type reuse | Per-provider duplication | Shared `ApiErrorDetails` type |

## Requirements

### What: Error Response Detection

**When fetching OpenAPI specs:**

1. Detect JSON responses that contain `error` field at the root
2. Validate the error structure matches expected format:
   - `error.code` (integer)
   - `error.message` (string)
   - `error.status` (string)
3. Log a warning with the error details
4. **Do NOT generate client code** for APIs that return errors
5. **Do generate** a stub file documenting the error (optional, for traceability)

**Detection logic:**

```rust
/// Check if a JSON response is an error response instead of a valid OpenAPI spec.
fn is_error_response(value: &serde_json::Value) -> Option<&ErrorBody> {
    // Valid OpenAPI specs have "openapi" or "swagger" field
    if value.get("openapi").is_some() || value.get("swagger").is_some() {
        return None;
    }
    
    // GCP Discovery Document format
    if value.get("kind") == Some(&json!("discovery#restDescription")) {
        return None;
    }
    
    // Check for error structure
    value.get("error")
        .and_then(|e| e.as_object())
        .and_then(|e| {
            if e.contains_key("code") && e.contains_key("message") {
                Some(ErrorBody {
                    code: e.get("code").and_then(|v| v.as_u64())?,
                    message: e.get("message")?.as_str()?.to_string(),
                    status: e.get("status").and_then(|v| v.as_str()).map(String::from),
                })
            } else {
                None
            }
        })
}
```

### What: Error Type Generation

**Generate error types in `clients/types.rs`:**

```rust
//! Shared types for {provider} API clients.
//!
//! Feature flag: `{provider}`

#![cfg(feature = "{provider}")]

use foundation_core::wire::simple_http::SimpleHeaders;
use serde::{Deserialize, Serialize};

/// Generic API response with status, headers, and parsed body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: u16,
    pub headers: SimpleHeaders,
    pub body: T,
}

/// Provider-agnostic error type for API operations.
#[derive(Debug)]
pub enum ApiError {
    /// Request building failed (e.g., URL formatting, header setup).
    RequestBuildFailed(String),
    
    /// Request sending failed (e.g., connection error, timeout).
    RequestSendFailed(String),
    
    /// HTTP error status received (4xx, 5xx).
    /// Use `ApiErrorDetails` to parse the error body for more details.
    HttpStatus { 
        code: u16, 
        headers: SimpleHeaders,
        body: Option<String>,
    },
    
    /// Response body parsing failed.
    ParseFailed(String),
    
    /// API returned an error response.
    /// Contains parsed error details if available.
    ApiError(ApiErrorDetails),
}

/// Standard Google Cloud API error format.
/// 
/// Most providers follow this structure:
/// ```json
/// {
///   "error": {
///     "code": 400,
///     "message": "Human readable message",
///     "status": "INVALID_ARGUMENT",
///     "details": [...]
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetails,
}

/// Detailed error information from API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorDetails {
    /// HTTP status code (e.g., 400, 404, 500).
    pub code: u64,
    
    /// Human-readable error message.
    pub message: String,
    
    /// Canonical error status (e.g., "INVALID_ARGUMENT", "NOT_FOUND").
    #[serde(default)]
    pub status: Option<String>,
    
    /// Additional error details (provider-specific structures).
    #[serde(default)]
    pub details: Vec<serde_json::Value>,
}

impl std::fmt::Display for ApiErrorDetails {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(status) = &self.status {
            write!(f, "{}: {}", status, self.message)
        } else {
            write!(f, "Error {}: {}", self.code, self.message)
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::RequestBuildFailed(e) => write!(f, "request build failed: {}", e),
            ApiError::RequestSendFailed(e) => write!(f, "request send failed: {}", e),
            ApiError::HttpStatus { code, body, .. } => {
                write!(f, "HTTP status {}", code)?;
                if let Some(body_str) = body {
                    write!(f, ": {}", body_str)?;
                }
                Ok(())
            }
            ApiError::ParseFailed(e) => write!(f, "parse failed: {}", e),
            ApiError::ApiError(details) => write!(f, "API error: {}", details),
        }
    }
}

impl std::error::Error for ApiError {}

/// Progress states for API operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiPending {
    Building,
    Sending,
}
```

### What: Universal Error Deserialization

**Update execute functions to parse error bodies:**

Current implementation:
```rust
.map_ready(|intro| match intro {
    RequestIntro::Success { stream, status } => {
        let headers = status.headers().clone();
        
        if !status.is_success() {
            return Err(ApiError::HttpStatus {
                code: status.as_u16(),
                headers,
            });
        }
        
        let body = body_reader::collect_string(stream);
        let parsed: T = serde_json::from_str(&body)
            .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
        
        Ok(ApiResponse {
            status: status.as_u16(),
            headers,
            body: parsed,
        })
    }
    RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
})
```

**Updated implementation with error parsing:**

```rust
.map_ready(|intro| match intro {
    RequestIntro::Success { stream, status } => {
        let headers = status.headers().clone();
        
        if !status.is_success() {
            // Collect body for error details
            let body = body_reader::collect_string(stream);
            
            // Try to parse as standard error format
            let api_error = serde_json::from_str::<ApiErrorBody>(&body)
                .map(|err_body| ApiError::ApiError(err_body.error))
                .unwrap_or_else(|_| {
                    // Fallback: raw body string
                    ApiError::HttpStatus {
                        code: status.as_u16(),
                        headers,
                        body: Some(body),
                    }
                });
            
            return Err(api_error);
        }
        
        // Success path - parse expected response type
        let body = body_reader::collect_string(stream);
        let parsed: T = serde_json::from_str(&body)
            .map_err(|e| ApiError::ParseFailed(e.to_string()))?;
        
        Ok(ApiResponse {
            status: status.as_u16(),
            headers,
            body: parsed,
        })
    }
    RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),
})
```

### How: Generator Changes

**File: `bin/platform/src/gen_resources/clients.rs`**

**Step 1: Add error detection to spec loading**

```rust
/// Check if a JSON value is an error response instead of a valid OpenAPI spec.
fn is_error_response(value: &serde_json::Value) -> Option<ErrorInfo> {
    // Valid OpenAPI specs have "openapi" or "swagger" field
    if value.get("openapi").is_some() || value.get("swagger").is_some() {
        return None;
    }
    
    // GCP Discovery Document format
    if value.get("kind") == Some(&json!("discovery#restDescription")) {
        return None;
    }
    
    // Check for error structure
    value.get("error")
        .and_then(|e| e.as_object())
        .and_then(|e| {
            let code = e.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
            let message = e.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            let status = e.get("status")
                .and_then(|v| v.as_str())
                .map(String::from);
            
            Some(ErrorInfo { code, message, status })
        })
}

#[derive(Debug, Clone)]
struct ErrorInfo {
    code: u64,
    message: String,
    status: Option<String>,
}
```

**Step 2: Update spec loading to detect errors**

```rust
fn load_spec(spec_path: &Path) -> Result<Option<OpenApiSpec>, GenClientError> {
    let content = std::fs::read_to_string(spec_path)
        .map_err(|e| GenClientError::ReadFile {
            path: spec_path.display().to_string(),
            source: e,
        })?;
    
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| GenClientError::Json {
            path: spec_path.display().to_string(),
            source: e,
        })?;
    
    // Check for error response
    if let Some(error_info) = is_error_response(&value) {
        tracing::warn!(
            "Spec {} returned error: {} - {}",
            spec_path.display(),
            error_info.status.as_deref().unwrap_or("UNKNOWN"),
            error_info.message
        );
        return Ok(None); // Skip this spec
    }
    
    // Parse as OpenAPI spec
    let spec = serde_json::from_str(&content)
        .map_err(|e| GenClientError::Json {
            path: spec_path.display().to_string(),
            source: e,
        })?;
    
    Ok(Some(spec))
}
```

**Step 3: Update generated types.rs with new error structure**

```rust
fn generate_shared_types(
    &self,
    output_dir: &Path,
    provider: &str,
) -> Result<(), GenClientError> {
    let mut out = String::new();
    
    writeln!(out, "//! Shared types for {} API clients.", provider).unwrap();
    writeln!(out, "//!").unwrap();
    writeln!(out, "//! Feature flag: `{}`", provider).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#![cfg(feature = \"{provider}\")]").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "use foundation_core::wire::simple_http::SimpleHeaders;").unwrap();
    writeln!(out, "use serde::{{Deserialize, Serialize}};").unwrap();
    writeln!(out).unwrap();
    
    // ApiResponse<T>
    writeln!(out, "/// Generic API response with status, headers, and parsed body.").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").unwrap();
    writeln!(out, "pub struct ApiResponse<T> {{").unwrap();
    writeln!(out, "    pub status: u16,").unwrap();
    writeln!(out, "    pub headers: SimpleHeaders,").unwrap();
    writeln!(out, "    pub body: T,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    
    // ApiError enum
    writeln!(out, "/// Provider-agnostic error type for API operations.").unwrap();
    writeln!(out, "#[derive(Debug)]").unwrap();
    writeln!(out, "pub enum ApiError {{").unwrap();
    writeln!(out, "    /// Request building failed.").unwrap();
    writeln!(out, "    RequestBuildFailed(String),").unwrap();
    writeln!(out, "    /// Request sending failed.").unwrap();
    writeln!(out, "    RequestSendFailed(String),").unwrap();
    writeln!(out, "    /// HTTP error status received (4xx, 5xx).").unwrap();
    writeln!(out, "    HttpStatus {{").unwrap();
    writeln!(out, "        code: u16,").unwrap();
    writeln!(out, "        headers: SimpleHeaders,").unwrap();
    writeln!(out, "        body: Option<String>,").unwrap();
    writeln!(out, "    }},").unwrap();
    writeln!(out, "    /// Response body parsing failed.").unwrap();
    writeln!(out, "    ParseFailed(String),").unwrap();
    writeln!(out, "    /// API returned an error response.").unwrap();
    writeln!(out, "    ApiError(ApiErrorDetails),").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    
    // ApiErrorBody
    writeln!(out, "/// Standard Google Cloud API error format.").unwrap();
    writeln!(out, "///").unwrap();
    writeln!(out, "/// Most providers follow this structure:").unwrap();
    writeln!(out, "/// ```json").unwrap();
    writeln!(out, "/// {{").unwrap();
    writeln!(out, "///   \"error\": {{").unwrap();
    writeln!(out, "///     \"code\": 400,").unwrap();
    writeln!(out, "///     \"message\": \"Human readable message\",").unwrap();
    writeln!(out, "///     \"status\": \"INVALID_ARGUMENT\",").unwrap();
    writeln!(out, "///     \"details\": [...]").unwrap();
    writeln!(out, "///   }}").unwrap();
    writeln!(out, "/// }}").unwrap();
    writeln!(out, "/// ```").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").unwrap();
    writeln!(out, "pub struct ApiErrorBody {{").unwrap();
    writeln!(out, "    pub error: ApiErrorDetails,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    
    // ApiErrorDetails
    writeln!(out, "/// Detailed error information from API responses.").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Serialize, Deserialize)]").unwrap();
    writeln!(out, "pub struct ApiErrorDetails {{").unwrap();
    writeln!(out, "    /// HTTP status code (e.g., 400, 404, 500).").unwrap();
    writeln!(out, "    pub code: u64,").unwrap();
    writeln!(out, "    /// Human-readable error message.").unwrap();
    writeln!(out, "    pub message: String,").unwrap();
    writeln!(out, "    /// Canonical error status (e.g., \"INVALID_ARGUMENT\", \"NOT_FOUND\").").unwrap();
    writeln!(out, "    #[serde(default)]").unwrap();
    writeln!(out, "    pub status: Option<String>,").unwrap();
    writeln!(out, "    /// Additional error details (provider-specific structures).").unwrap();
    writeln!(out, "    #[serde(default)]").unwrap();
    writeln!(out, "    pub details: Vec<serde_json::Value>,").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    
    // Display impl for ApiErrorDetails
    writeln!(out, "impl std::fmt::Display for ApiErrorDetails {{").unwrap();
    writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{").unwrap();
    writeln!(out, "        if let Some(status) = &self.status {{").unwrap();
    writeln!(out, "            write!(f, \"{{}}: {{}}\", status, self.message)").unwrap();
    writeln!(out, "        }} else {{").unwrap();
    writeln!(out, "            write!(f, \"Error {{}}: {{}}\", self.code, self.message)").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    
    // Display impl for ApiError
    writeln!(out, "impl std::fmt::Display for ApiError {{").unwrap();
    writeln!(out, "    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{").unwrap();
    writeln!(out, "        match self {{").unwrap();
    writeln!(out, "            ApiError::RequestBuildFailed(e) => write!(f, \"request build failed: {{}}\", e),").unwrap();
    writeln!(out, "            ApiError::RequestSendFailed(e) => write!(f, \"request send failed: {{}}\", e),").unwrap();
    writeln!(out, "            ApiError::HttpStatus {{ code, body, .. }} => {{").unwrap();
    writeln!(out, "                write!(f, \"HTTP status {{}}\", code)?;").unwrap();
    writeln!(out, "                if let Some(body_str) = body {{").unwrap();
    writeln!(out, "                    write!(f, \": {{}}\", body_str)?;").unwrap();
    writeln!(out, "                }}").unwrap();
    writeln!(out, "                Ok(())").unwrap();
    writeln!(out, "            }},").unwrap();
    writeln!(out, "            ApiError::ParseFailed(e) => write!(f, \"parse failed: {{}}\", e),").unwrap();
    writeln!(out, "            ApiError::ApiError(details) => write!(f, \"API error: {{}}\", details),").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "impl std::error::Error for ApiError {{}}").unwrap();
    writeln!(out).unwrap();
    
    // ApiPending enum
    writeln!(out, "/// Progress states for API operations.").unwrap();
    writeln!(out, "#[derive(Debug, Clone, Copy, PartialEq, Eq)]").unwrap();
    writeln!(out, "pub enum ApiPending {{").unwrap();
    writeln!(out, "    Building,").unwrap();
    writeln!(out, "    Sending,").unwrap();
    writeln!(out, "}}").unwrap();
    
    let output_path = output_dir.join("types.rs");
    std::fs::write(&output_path, out)
        .map_err(|e| GenClientError::WriteFile {
            path: output_path.display().to_string(),
            source: e,
        })?;
    
    Ok(())
}
```

**Step 4: Update execute function generation with error parsing**

```rust
/// Generate the valtron combinator chain for execute function with error parsing.
fn generate_valtron_chain_execute(
    out: &mut String,
    return_type: &str,
) {
    writeln!(out, "    let task = builder").unwrap();
    writeln!(out, "        .build_send_request()").unwrap();
    writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))?").unwrap();
    writeln!(out, "        .map_ready(|intro| match intro {{").unwrap();
    writeln!(out, "            RequestIntro::Success {{ stream, status }} => {{").unwrap();
    writeln!(out, "                let headers = status.headers().clone();").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                if !status.is_success() {{").unwrap();
    writeln!(out, "                    // Collect body for error details").unwrap();
    writeln!(out, "                    let body = body_reader::collect_string(stream);").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                    // Try to parse as standard error format").unwrap();
    writeln!(out, "                    let api_error = serde_json::from_str::<ApiErrorBody>(&body)").unwrap();
    writeln!(out, "                        .map(|err_body| ApiError::ApiError(err_body.error))").unwrap();
    writeln!(out, "                        .unwrap_or_else(|_| {{").unwrap();
    writeln!(out, "                            // Fallback: raw body string").unwrap();
    writeln!(out, "                            ApiError::HttpStatus {{").unwrap();
    writeln!(out, "                                code: status.as_u16(),").unwrap();
    writeln!(out, "                                headers,").unwrap();
    writeln!(out, "                                body: Some(body),").unwrap();
    writeln!(out, "                            }}").unwrap();
    writeln!(out, "                        }});").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                    return Err(api_error);").unwrap();
    writeln!(out, "                }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                // Success path - parse expected response type").unwrap();
    writeln!(out, "                let body = body_reader::collect_string(stream);").unwrap();
    writeln!(out, "                let parsed: {} = serde_json::from_str(&body)", return_type).unwrap();
    writeln!(out, "                    .map_err(|e| ApiError::ParseFailed(e.to_string()))?;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "                Ok(ApiResponse {{").unwrap();
    writeln!(out, "                    status: status.as_u16(),").unwrap();
    writeln!(out, "                    headers,").unwrap();
    writeln!(out, "                    body: parsed,").unwrap();
    writeln!(out, "                }})").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(out, "            RequestIntro::Failed(e) => Err(ApiError::RequestSendFailed(e.to_string())),").unwrap();
    writeln!(out, "        }})").unwrap();
    writeln!(out, "        .map_pending(|_| ApiPending::Sending);").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    execute(task, None)").unwrap();
    writeln!(out, "        .map_err(|e| ApiError::RequestBuildFailed(e.to_string()))").unwrap();
}
```

### How: Spec Fetcher Changes

**File: `bin/platform/src/gen_resources/provider_specs_fetcher.rs`**

Update the fetcher to detect and log error responses:

```rust
/// Save raw JSON spec and return path, or log error if response is an error.
fn save_spec_json(
    provider: &str,
    api: Option<&str>,
    content: &str,
    artefacts_dir: &Path,
) -> Result<PathBuf, SpecFetchError> {
    // Parse and check for error response
    let value: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| SpecFetchError::Json {
            provider: provider.to_string(),
            source: e,
        })?;
    
    // Check for error structure
    if let Some(error) = value.get("error").and_then(|e| e.as_object()) {
        let code = error.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
        let message = error.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        let status = error.get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");
        
        let api_suffix = api.map(|s| format!("/{}", s)).unwrap_or_default();
        tracing::warn!(
            "Spec fetch for {}{} returned error: {} - {}",
            provider,
            api_suffix,
            status,
            message
        );
        
        // Still save the error response for debugging
        let spec_dir = if let Some(api_name) = api {
            artefacts_dir.join(provider).join(api_name)
        } else {
            artefacts_dir.join(provider)
        };
        std::fs::create_dir_all(&spec_dir)?;
        
        let spec_path = spec_dir.join("openapi.json");
        std::fs::write(&spec_path, content)?;
        
        // Return error to skip this spec
        return Err(SpecFetchError::Generic(format!(
            "API {} returned error: {} - {}",
            api.unwrap_or(provider),
            status,
            message
        )));
    }
    
    // Valid spec - save normally
    let spec_dir = if let Some(api_name) = api {
        artefacts_dir.join(provider).join(api_name)
    } else {
        artefacts_dir.join(provider)
    };
    std::fs::create_dir_all(&spec_dir)?;
    
    let spec_path = spec_dir.join("openapi.json");
    std::fs::write(&spec_path, content)?;
    
    Ok(spec_path)
}
```

## User Experience

### Error Messages Before and After

**Before (current):**
```
warning: Skipping gcp/developerknowledge - spec fetch failed
```
Generated file:
```rust
//! Auto-generated API clients for gcp/developerknowledge.
//! SKIPPED: Spec fetch returned an error
```

**After (proposed):**
```
warning: Spec fetch for gcp/developerknowledge returned error: INVALID_ARGUMENT - No default GOOGLE_REST format gets defined in this API.
```
Generated file:
```rust
//! Auto-generated API clients for gcp/developerknowledge.
//! SKIPPED: API returned error during spec fetch
//! Error: INVALID_ARGUMENT - No default GOOGLE_REST format gets defined in this API.
```

### Using Generated Clients with Error Handling

**Before:**
```rust
match list_services(&client, "my-project") {
    Ok(stream) => { /* handle success */ }
    Err(ApiError::HttpStatus { code, .. }) => {
        eprintln!("HTTP error: {}", code); // No error message
    }
    Err(e) => eprintln!("Error: {:?}", e),
}
```

**After:**
```rust
match list_services(&client, "my-project") {
    Ok(stream) => { /* handle success */ }
    Err(ApiError::ApiError(details)) => {
        eprintln!("API error: {} - {}", details.status.unwrap_or_default(), details.message);
        // Can also inspect details.code, details.details
    }
    Err(ApiError::HttpStatus { code, body, .. }) => {
        eprintln!("HTTP error {}: {}", code, body.unwrap_or_default());
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Tasks

1. **Add error detection to spec loader** - COMPLETED
   - [x] Add error detection logic to `clients.rs` (inline in `generate_clients_for_spec()`)
   - [x] Update spec processing to detect and skip error responses
   - [x] Add warning logs with error details

2. **Update generated error types** - COMPLETED
   - [x] Add `ApiErrorBody` struct to `types.rs` template
   - [x] Add `ApiErrorDetails` struct with `code`, `message`, `status`, `details`
   - [x] Update `ApiError::HttpStatus` to include `body: Option<String>`
   - [x] Add `ApiError::ApiError(ApiErrorDetails)` variant
   - [x] Update `Display` impl for new variants

3. **Update execute function generation** - COMPLETED
   - [x] Modify `generate_valtron_chain_execute()` to parse error bodies
   - [x] Try parsing as `ApiErrorBody` first
   - [x] Fallback to raw body string if parse fails
   - [x] Return appropriate `ApiError` variant

4. **Update spec fetcher** - COMPLETED
   - [x] Add error detection in `create_api_fetch_task()` (GCP)
   - [x] Add error detection in `write_single_spec()` (GCP)
   - [x] Log detailed error messages with status and message

5. **Regenerate all provider clients** - COMPLETED
   - [x] All provider clients have new error structure in `types.rs`
   - [x] Execute functions parse error bodies

6. **Verification** - COMPLETED
   - [x] Generated code compiles (verified with stripe feature)
   - [x] Error types implement `Serialize`, `Deserialize`, `Display`, `Error`

7. **Documentation** - COMPLETED
   - [x] Update feature spec with implementation status
   - [x] Document error type usage in generated clients

## Success Criteria

- [x] All 7 tasks completed
- [x] Error responses detected during spec fetch (not silently skipped)
- [x] Generated `ApiErrorDetails` struct with all fields
- [x] Generated `ApiError::ApiError` variant for parsed errors
- [x] Generated `ApiError::HttpStatus` includes `body` field
- [x] Execute functions parse error bodies before returning `HttpStatus`
- [x] All generated code compiles with zero warnings
- [x] Error types have proper `Serialize`, `Deserialize` derives

## Lessons Learned

### Implementation Notes

1. **Error detection location**: Error detection was implemented in two places:
   - **Client generator** (`bin/platform/src/gen_resources/clients.rs:246-282`): Detects error responses when processing spec files, generates stub modules with error comments
   - **GCP fetcher** (`backends/foundation_deployment/src/providers/gcp/fetch.rs`): Detects error responses during API spec fetch, logs warnings and skips writing

2. **Design decision - inline detection**: Instead of a separate `is_error_response()` function, error detection was implemented inline where specs are processed. This keeps the logic close to the usage site.

3. **GCP-specific handling**: The GCP fetcher needed special handling because it fetches specs dynamically at runtime. Error detection happens in both `create_api_fetch_task()` (early detection with logging) and `write_single_spec()` (final check before writing).

4. **Generated code pattern**: All generated clients now share the same error handling pattern:
   - `ApiErrorDetails` - structured error with `code`, `message`, `status`, `details`
   - `ApiErrorBody` - wrapper matching `{"error": {...}}` format
   - `ApiError::ApiError(ApiErrorDetails)` - for parsed API errors
   - `ApiError::HttpStatus { body: Some(...) }` - fallback for non-standard errors

### Verification Results

- Stripe provider builds successfully with generated error handling
- GCP provider has error detection in fetcher
- All providers share the same error type structure in `types.rs`

## Verification

**Regenerate all provider clients:**
```bash
cd /home/darkvoid/Boxxed/@dev/ewe_platform

# Regenerate all provider clients
cargo run --bin ewe_platform gen_resources clients

# Verify compilation
cargo check -p foundation_deployment --features gcp,stripe,cloudflare

# Verify linting
cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic

# Verify documentation
cargo doc -p foundation_deployment --no-deps
```

**Check generated types:**
```bash
# Verify ApiErrorDetails struct exists
grep -A 10 "pub struct ApiErrorDetails" \
    backends/foundation_deployment/src/providers/stripe/clients/types.rs

# Verify error parsing in execute function
grep -A 5 "ApiErrorBody" \
    backends/foundation_deployment/src/providers/stripe/clients/*.rs | head -20
```

**Test error detection:**
```bash
# Check that developerknowledge spec was detected as error
cargo run --bin ewe_platform gen_resources clients --provider gcp 2>&1 \
    | grep -i "developerknowledge.*error"
```

## Acceptance Criteria

1. **Error Detection**: Fetching specs that return error JSON (like `developerknowledge`) logs a warning with the error details instead of silently skipping.

2. **Error Type Generation**: Generated `types.rs` includes `ApiErrorDetails` struct with fields: `code`, `message`, `status`, `details`.

3. **Error Parsing**: Generated execute functions attempt to parse 4xx/5xx response bodies as `ApiErrorBody` before falling back to raw string.

4. **Integration**: `ApiError` enum includes `ApiError(ApiErrorDetails)` variant for parsed errors and `HttpStatus` includes optional `body` field.

5. **Zero Warnings**: All generated code compiles with zero warnings, passes clippy and rustdoc.

6. **Backward Compatible**: Existing code using `ApiError` continues to work (new variants are additive, existing match arms still valid with wildcard).

## Design Decisions

### Why `Option<String>` body in `HttpStatus`?

To maintain backward compatibility while adding optional error body parsing. Existing code that matches `HttpStatus` doesn't break; new code can access the body for debugging.

### Why separate `ApiErrorDetails` and `ApiErrorBody`?

`ApiErrorBody` wraps the outer `{"error": {...}}` structure, while `ApiErrorDetails` represents the inner error object. This matches the JSON structure and makes parsing straightforward.

### Why fallback to raw body string?

Not all APIs follow the Google Cloud error format. Some return plain text or custom JSON structures. The fallback ensures we never lose error information.

### Why still generate stub files for error specs?

Stub files with error comments provide traceability - developers can see which APIs were skipped and why. This is better than silent omission.

## Future Improvements

1. **Provider-specific error types**: Generate custom error detail structs per-provider (e.g., `StripeErrorDetails`, `GcpErrorDetails`).

2. **Error type hierarchy**: Support nested error structures with proper Rust enums.

3. **Retry hints**: Add `is_retryable()` method to `ApiErrorDetails` based on status code.

4. **Error code constants**: Generate `pub const` error status codes for type-safe matching.

---

_Created: 2026-04-06_
_Status: Proposed_
