---
workspace_name: "ewe_platform"
spec_directory: "specifications/10-simple-http-client-enhancements"
feature_directory: "specifications/10-simple-http-client-enhancements/features/03-multiple-response-body-handling"
this_file: "specifications/10-simple-http-client-enhancements/features/03-multiple-response-body-handling/feature.md"

status: pending
priority: high
created: "2026-03-25"
completed: null

depends_on: []

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0
---

# Multiple Response Body Handling

## Overview

This feature documents comprehensive patterns for handling all `SendSafeBody` variants when reading HTTP responses. HTTP servers can return response bodies in different forms depending on `Transfer-Encoding` and `Content-Length` headers.

## WHY: Problem Statement

HTTP responses can return bodies in different forms:

| Variant | When Used |
|---------|-----------|
| `SendSafeBody::Text` | Small text responses, known length |
| `SendSafeBody::Bytes` | Binary responses, known length |
| `SendSafeBody::Stream` | Streaming responses with known Content-Length |
| `SendSafeBody::ChunkedStream` | Chunked transfer encoding |
| `SendSafeBody::LineFeedStream` | Line-delimited streaming (SSE, NDJSON) |
| `SendSafeBody::None` | HEAD requests, 204 No Content |

Users need documented patterns for handling each variant correctly, including:
- UTF-8 conversion with fallback on error
- Iterator consumption for streaming data
- Proper handling of chunked transfer encoding
- Graceful error handling

### Source Pattern Analysis

From `gen_model_descriptors/mod.rs`:

```rust
match body {
    SendSafeBody::Text(t) => {
        // Direct text response
        body_text = t;
    }
    SendSafeBody::Bytes(b) => {
        // Raw bytes - convert to UTF-8 with fallback
        body_text = String::from_utf8(b.clone()).unwrap_or_else(|e| {
            tracing::warn!("Invalid UTF-8 in response: {e}");
            String::new()
        });
    }
    SendSafeBody::Stream(mut opt_iter) => {
        // Consume stream iterator and collect bytes
        if let Some(iter) = opt_iter.take() {
            let mut bytes = Vec::new();
            for chunk_result in iter {
                match chunk_result {
                    Ok(data) => bytes.extend_from_slice(&data),
                    Err(e) => {
                        tracing::warn!("Stream error: {e}");
                        break;
                    }
                }
            }
            body_text = String::from_utf8(bytes).unwrap_or_else(|e| {
                tracing::warn!("Invalid UTF-8 in streamed response: {e}");
                String::new()
            });
        }
    }
    SendSafeBody::ChunkedStream(mut opt_iter) => {
        // Handle chunked transfer encoding
        if let Some(iter) = opt_iter.take() {
            let mut bytes = Vec::new();
            for chunk_result in iter {
                match chunk_result {
                    Ok(ChunkedData::Data(data, _)) => bytes.extend_from_slice(&data),
                    Ok(ChunkedData::Trailers(_)) => {} // Skip trailers
                    Ok(ChunkedData::DataEnded) => break,
                    Err(e) => {
                        tracing::warn!("Chunked stream error: {e}");
                        break;
                    }
                }
            }
            body_text = String::from_utf8(bytes).unwrap_or_else(|e| {
                tracing::warn!("Invalid UTF-8 in chunked response: {e}");
                String::new()
            });
        }
    }
    SendSafeBody::None => {
        tracing::warn!("No body in response");
    }
    SendSafeBody::LineFeedStream(_) => {
        tracing::warn!("LineFeedStream not supported for this use case");
    }
}
```

## WHAT: Solution Overview

Provide comprehensive body handling patterns:

### Pattern 1: Simple Body Reading (Text Responses)

For simple cases where you expect text:

```rust
fn read_body_simple(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> String {
    for part in stream {
        match part {
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                return match body {
                    SendSafeBody::Text(t) => t,
                    SendSafeBody::Bytes(b) => {
                        String::from_utf8(b.clone()).unwrap_or_else(|e| {
                            tracing::warn!("Invalid UTF-8: {e}");
                            String::new()
                        })
                    }
                    _ => String::new(),
                };
            }
            _ => continue,
        }
    }
    String::new()
}
```

### Pattern 2: Comprehensive Body Reading (All Variants)

This pattern provides both a core function returning `Result` for proper error handling,
and a convenience wrapper for graceful degradation.

#### Core Function (Returns Result)

```rust
use foundation_core::wire::simple_http::ChunkedData;

/// StringBodyError - errors that can occur during string body reading.
#[derive(Debug, Display)]
pub enum StringBodyError {
    /// Stream read error
    #[display("stream read error: {0}")]
    StreamRead(HttpReaderError),
    /// No body in response
    #[display("no body in response")]
    NoBody,
    /// Invalid UTF-8 in response
    #[display("invalid UTF-8: {0}")]
    InvalidUtf8(std::string::FromUtf8Error),
}

impl std::error::Error for StringBodyError {}

/// Read response body as a String - core function returning Result.
///
/// WHY: Provides proper error handling so callers can decide how to handle failures.
///
/// WHAT: Returns `Result<String, StringBodyError>` with specific error information.
///
/// HOW: Handles all SendSafeBody variants, propagates errors.
///
/// # Arguments
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
/// * `Ok(String)` - The response body as a String
/// * `Err(StringBodyError::StreamRead(e))` - Stream read error
/// * `Err(StringBodyError::NoBody)` - No body in response
/// * `Err(StringBodyError::InvalidUtf8(e))` - UTF-8 conversion failed
///
/// # Examples
///
/// ```rust
/// // Strict error handling
/// match collect_string_strict(stream) {
///     Ok(body) => process(body),
///     Err(StringBodyError::NoBody) => handle_no_body(),
///     Err(StringBodyError::StreamRead(e)) => handle_error(e),
///     Err(StringBodyError::InvalidUtf8(e)) => handle_utf8_error(e),
/// }
/// ```
pub fn collect_string_strict(
    mut stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<String, StringBodyError> {
    let mut body_text = String::new();

    for part in stream {
        match part {
            // Handle sized and streamed bodies
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                match body {
                    SendSafeBody::Text(t) => {
                        // Direct text - no conversion needed
                        return Ok(t);
                    }

                    SendSafeBody::Bytes(b) => {
                        // Raw bytes - convert to UTF-8, propagate error
                        return String::from_utf8(b.clone())
                            .map_err(StringBodyError::InvalidUtf8);
                    }

                    SendSafeBody::Stream(mut opt_iter) => {
                        // Streaming body with known length
                        if let Some(iter) = opt_iter.take() {
                            let mut bytes = Vec::new();
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => return Err(StringBodyError::StreamRead(e)),
                                }
                            }
                            return String::from_utf8(bytes)
                                .map_err(StringBodyError::InvalidUtf8);
                        }
                    }

                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        // Chunked transfer encoding
                        if let Some(iter) = opt_iter.take() {
                            let mut bytes = Vec::new();
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        bytes.extend_from_slice(&data);
                                    }
                                    Ok(ChunkedData::Trailers(_)) => {
                                        // Silently ignore trailers
                                    }
                                    Ok(ChunkedData::DataEnded) => {
                                        break;
                                    }
                                    Err(e) => return Err(StringBodyError::StreamRead(e)),
                                }
                            }
                            return String::from_utf8(bytes)
                                .map_err(StringBodyError::InvalidUtf8);
                        }
                    }

                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        // Line-delimited streaming (SSE, NDJSON)
                        if let Some(iter) = opt_iter.take() {
                            let mut lines = Vec::new();
                            for line_result in iter {
                                match line_result {
                                    Ok(line) => lines.push(line),
                                    Err(e) => return Err(StringBodyError::StreamRead(e)),
                                }
                            }
                            return Ok(lines.join("\n"));
                        }
                    }

                    SendSafeBody::None => return Err(StringBodyError::NoBody),
                }
            }

            // Skip intro and headers - we want the body
            Ok(IncomingResponseParts::Intro(_, _, _)) => continue,
            Ok(IncomingResponseParts::Headers(_)) => continue,
            Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Err(StringBodyError::NoBody),

            // Stream error
            Err(e) => return Err(StringBodyError::StreamRead(e)),
        }
    }

    Ok(body_text)
}
```

#### Convenience Wrapper (Graceful Degradation)

```rust
/// Read response body as a String - convenience wrapper.
///
/// WHY: Most callers just want the body text and don't need to handle errors explicitly.
///
/// WHAT: Returns `String`, logging warnings and returning empty string on error.
///
/// HOW: Wraps `collect_string_strict()` and handles errors internally.
///
/// # Arguments
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
/// The response body as a String. Returns empty string on error or no body.
///
/// # Notes
/// - UTF-8 conversion uses fallback on error (returns partial data)
/// - Stream errors are logged but don't panic
/// - Chunked trailers are silently ignored
///
/// # Examples
///
/// ```rust
/// // Simple usage - errors logged, empty string returned on failure
/// let body = collect_string(stream);
/// if body.is_empty() {
///     // Check logs for error details
/// }
/// ```
pub fn collect_string(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> String {
    match collect_string_strict(stream) {
        Ok(text) => text,
        Err(StringBodyError::InvalidUtf8(e)) => {
            tracing::warn!("Invalid UTF-8 in response body: {e}");
            String::new()
        }
        Err(e) => {
            tracing::warn!("Body read error: {e}");
            String::new()
        }
    }
}
```

#### Usage Comparison

```rust
// Strict error handling - caller decides what to do
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                match collect_string_strict(stream) {
                    Ok(body) => ProcessResult::Success(body),
                    Err(StringBodyError::NoBody) => ProcessResult::NoBody,
                    Err(StringBodyError::StreamRead(e)) => {
                        ProcessResult::Error(format!("Read error: {}", e))
                    }
                    Err(StringBodyError::InvalidUtf8(e)) => {
                        ProcessResult::Error(format!("UTF-8 error: {}", e))
                    }
                }
            }
            RequestIntro::Failed(e) => ProcessResult::Error(e.to_string()),
        }
    });

// Graceful degradation - errors logged, empty string returned on failure
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => collect_string(stream),
            RequestIntro::Failed(_) => String::new(),
        }
    });
```

### Pattern 3: Binary Body Reading

This pattern provides both a core function returning `Result` for proper error handling,
and a convenience wrapper for graceful degradation.

#### Core Function (Returns Result)

```rust
use foundation_core::wire::simple_http::ChunkedData;

/// BodyReaderError - errors that can occur during body reading.
#[derive(Debug, Display)]
pub enum BodyReaderError {
    /// Stream read error
    #[display("stream read error: {0}")]
    StreamRead(HttpReaderError),
    /// No body in response
    #[display("no body in response")]
    NoBody,
    /// Unexpected body variant
    #[display("unexpected body variant")]
    UnexpectedVariant,
}

impl std::error::Error for BodyReaderError {}

/// Read response body as bytes - core function returning Result.
///
/// WHY: Provides proper error handling so callers can decide how to handle failures.
///
/// WHAT: Returns `Result<Vec<u8>, BodyReaderError>` with specific error information.
///
/// HOW: Collects bytes from any body variant, errors on stream failures.
///
/// # Arguments
/// * `stream` - Response stream iterator
///
/// # Returns
/// * `Ok(Vec<u8>)` - Raw bytes from response
/// * `Err(BodyReaderError)` - Specific error information
///
/// # Examples
///
/// ```rust
/// // Strict error handling
/// match collect_bytes_strict(stream) {
///     Ok(bytes) => process(bytes),
///     Err(BodyReaderError::NoBody) => handle_no_body(),
///     Err(BodyReaderError::StreamRead(e)) => handle_error(e),
///     Err(BodyReaderError::UnexpectedVariant) => handle_variant_error(),
/// }
/// ```
pub fn collect_bytes_strict(
    mut stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<Vec<u8>, BodyReaderError> {
    let mut bytes = Vec::new();

    for part in stream {
        match part {
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                match body {
                    SendSafeBody::Text(t) => return Ok(t.into_bytes()),
                    SendSafeBody::Bytes(b) => return Ok(b.clone()),
                    SendSafeBody::Stream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(bytes);
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        bytes.extend_from_slice(&data);
                                    }
                                    Ok(ChunkedData::DataEnded) => break,
                                    Ok(ChunkedData::Trailers(_)) => {}
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(bytes);
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(line) => bytes.extend_from_slice(line.as_bytes()),
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(bytes);
                    }
                    SendSafeBody::None => return Err(BodyReaderError::NoBody),
                }
            }
            Ok(IncomingResponseParts::NoBody) => return Err(BodyReaderError::NoBody),
            Ok(IncomingResponseParts::Intro(_, _, _)
            | IncomingResponseParts::Headers(_)
            | IncomingResponseParts::SKIP) => continue,
            Err(e) => return Err(BodyReaderError::StreamRead(e)),
        }
    }

    Ok(bytes)
}
```

#### Convenience Wrapper (Graceful Degradation)

```rust
/// Read response body as bytes - convenience wrapper with graceful degradation.
///
/// WHY: Most callers just want the bytes and don't need to handle errors explicitly.
///
/// WHAT: Returns `Vec<u8>`, logging warnings and returning empty vec on error.
///
/// HOW: Wraps `collect_bytes_strict()` and handles errors internally.
///
/// # Arguments
/// * `stream` - Response stream iterator
///
/// # Returns
/// Vec<u8> containing raw bytes. Returns empty vec on error or no body.
///
/// # Examples
///
/// ```rust
/// // Simple usage - errors logged, empty vec returned on failure
/// let bytes = collect_bytes(stream);
/// if bytes.is_empty() {
///     // Check logs for error details
/// }
/// ```
pub fn collect_bytes(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Vec<u8> {
    match collect_bytes_strict(stream) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!("Body read error: {e}");
            Vec::new()
        }
    }
}
```

#### Usage Comparison

```rust
// Strict error handling - caller decides what to do
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                match collect_bytes_strict(stream) {
                    Ok(bytes) => ProcessResult::Success(bytes),
                    Err(BodyReaderError::NoBody) => ProcessResult::NoBody,
                    Err(e) => ProcessResult::Error(e.to_string()),
                }
            }
            RequestIntro::Failed(e) => ProcessResult::Error(e.to_string()),
        }
    });

// Graceful degradation - errors logged, empty result on failure
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => collect_bytes(stream),
            RequestIntro::Failed(_) => Vec::new(),
        }
    });
```

### Pattern 3b: Direct Byte Collection (No UTF-8 Conversion)

For cases where you need raw bytes without any UTF-8 conversion overhead - this is the most efficient way to collect binary data:

```rust
use foundation_core::wire::simple_http::ChunkedData;

/// Collect response body as raw bytes without any UTF-8 conversion.
///
/// WHY: Provides the most efficient path for binary data (images, files, etc.)
/// by avoiding UTF-8 conversion entirely.
///
/// WHAT: Direct byte collection from any body variant.
///
/// HOW: Handles Text by calling `.into_bytes()`, Bytes by cloning,
/// and streams by collecting chunks directly.
///
/// # Arguments
/// * `stream` - Response stream iterator
///
/// # Returns
/// Vec<u8> containing raw response bytes. Returns empty vec on error or no body.
///
/// # Examples
///
/// ```rust
/// // Download an image
/// let task = SendRequestTask::new(request, 5, pool, config)
///     .map_ready(|intro| {
///         match intro {
///             RequestIntro::Success { stream, .. } => {
///                 collect_bytes_direct(stream)
///             }
///             RequestIntro::Failed(_) => Vec::new(),
///         }
///     });
///
/// let mut result_stream = execute(task, None)?;
/// for item in result_stream {
///     if let Stream::Next(image_bytes) = item {
///         std::fs::write("download.png", &image_bytes)?;
///     }
/// }
/// ```
pub fn collect_bytes_direct(
    mut stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Vec<u8> {
    let mut bytes = Vec::new();

    for part in stream {
        match part {
            // Handle sized and streamed bodies - collect bytes directly
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                match body {
                    SendSafeBody::Text(t) => {
                        // Text to bytes - no UTF-8 decoding needed
                        return t.into_bytes();
                    }
                    SendSafeBody::Bytes(b) => {
                        // Already bytes - direct clone, no conversion
                        return b.clone();
                    }
                    SendSafeBody::Stream(mut opt_iter) => {
                        // Stream: collect chunks directly as bytes
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => {
                                        tracing::warn!("Stream error during byte collection: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                        return bytes;
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        // Chunked: collect data chunks, skip trailers
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        bytes.extend_from_slice(&data);
                                    }
                                    Ok(ChunkedData::Trailers(_)) => {
                                        // Silently ignore trailers
                                    }
                                    Ok(ChunkedData::DataEnded) => {
                                        break;
                                    }
                                    Err(e) => {
                                        tracing::warn!("Chunked stream error: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                        return bytes;
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        // Line-delimited: collect lines as bytes
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(line) => {
                                        bytes.extend_from_slice(line.as_bytes());
                                        bytes.push(b'\n');
                                    }
                                    Err(e) => {
                                        tracing::warn!("Line stream error: {e}");
                                        break;
                                    }
                                }
                            }
                        }
                        return bytes;
                    }
                    SendSafeBody::None => {
                        tracing::debug!("Response has no body");
                        return Vec::new();
                    }
                }
            }

            // Skip intro/headers, return on no body
            Ok(IncomingResponseParts::Intro(_, _, _)) => continue,
            Ok(IncomingResponseParts::Headers(_)) => continue,
            Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Vec::new(),

            // Stream error
            Err(e) => {
                tracing::warn!("Error reading stream for byte collection: {e}");
                return Vec::new();
            }
        }
    }

    bytes
}
```

**Key difference from `read_body_as_bytes()`:** This function explicitly handles all variants including `LineFeedStream` and properly handles `ChunkedData::Trailers`. Use this when you need complete, correct byte collection.

### Pattern 4: JSON Body Parsing

This pattern provides both a core function returning `Result` for proper error handling,
and a convenience wrapper for graceful degradation.

#### Core Function (Returns Result)

```rust
use serde::de::DeserializeOwned;

/// Read and parse JSON response body - core function returning Result.
///
/// WHY: Provides proper error handling so callers can distinguish between
/// body read errors and JSON parse errors.
///
/// WHAT: Returns `Result<T, JsonParseError>` with specific error information.
///
/// HOW: First reads body as string, then parses JSON.
///
/// # Type Parameters
/// * `T` - Target type for deserialization
///
/// # Arguments
/// * `stream` - Response stream iterator
///
/// # Returns
/// * `Ok(T)` - Successfully parsed JSON
/// * `Err(JsonParseError::BodyRead(e))` - Failed to read response body
/// * `Err(JsonParseError::JsonParse(e))` - Failed to parse JSON
///
/// # Examples
///
/// ```rust
/// // Strict error handling
/// match parse_json_strict::<ApiResponse>(stream) {
///     Ok(response) => process(response),
///     Err(JsonParseError::BodyRead(e)) => handle_read_error(e),
///     Err(JsonParseError::JsonParse(e)) => handle_parse_error(e),
/// }
/// ```
#[derive(Debug, Display)]
pub enum JsonParseError {
    #[display("failed to read body: {0}")]
    BodyRead(BodyReaderError),
    #[display("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),
}

impl std::error::Error for JsonParseError {}

pub fn parse_json_strict<T: DeserializeOwned>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<T, JsonParseError> {
    let body_text = collect_string_strict(stream)
        .map_err(JsonParseError::BodyRead)?;

    serde_json::from_str(&body_text)
        .map_err(JsonParseError::JsonParse)
}
```

#### Convenience Wrapper (Graceful Degradation)

```rust
/// Read and parse JSON response body - convenience wrapper.
///
/// WHY: Most callers just want the parsed data and don't need to handle errors explicitly.
///
/// WHAT: Returns `Option<T>`, logging warnings and returning None on error.
///
/// HOW: Wraps `parse_json_strict()` and handles errors internally.
///
/// # Type Parameters
/// * `T` - Target type for deserialization
///
/// # Arguments
/// * `stream` - Response stream iterator
///
/// # Returns
/// * `Some(T)` - Successfully parsed JSON
/// * `None` - Error occurred (logged)
///
/// # Examples
///
/// ```rust
/// // Simple usage - errors logged, None returned on failure
/// if let Some(response) = parse_json::<ApiResponse>(stream) {
///     println!("Got {} items", response.items.len());
/// } else {
///     // Check logs for error details
/// }
/// ```
pub fn parse_json<T: DeserializeOwned>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Option<T> {
    match parse_json_strict::<T>(stream) {
        Ok(value) => Some(value),
        Err(e) => {
            tracing::warn!("JSON parse error: {e}");
            None
        }
    }
}
```

#### Usage Comparison

```rust
// Strict error handling - caller distinguishes error types
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                match parse_json_strict::<ApiResponse>(stream) {
                    Ok(response) => ProcessResult::Success(response),
                    Err(JsonParseError::BodyRead(e)) => {
                        ProcessResult::Error(format!("Body read failed: {}", e))
                    }
                    Err(JsonParseError::JsonParse(e)) => {
                        ProcessResult::Error(format!("JSON parse failed: {}", e))
                    }
                }
            }
            RequestIntro::Failed(e) => ProcessResult::Error(e.to_string()),
        }
    });

// Graceful degradation - errors logged, None returned on failure
let task = SendRequestTask::new(request, 5, pool, config)
    .map_ready(|intro| {
        match intro {
            RequestIntro::Success { stream, .. } => {
                parse_json::<ApiResponse>(stream)
                    .unwrap_or_else(|| ApiResponse::default())
            }
            RequestIntro::Failed(_) => ApiResponse::default(),
        }
    });
```

### Pattern 5: Streaming Body Processing

This pattern provides both a core function returning `Result` for proper error handling,
and a convenience wrapper for graceful degradation.

#### Core Function (Returns Result)

```rust
/// Result of streaming body processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStreamResult {
    /// Processing completed successfully
    Completed,
    /// Processing stopped by callback (returned false)
    StoppedByCallback,
    /// No body to process
    NoBody,
}

/// Process streaming body with a callback - core function returning Result.
///
/// WHY: Provides proper error handling so callers can distinguish between
/// callback-initiated stops and actual errors.
///
/// WHAT: Returns `Result<ProcessStreamResult, BodyReaderError>` with specific outcome.
///
/// HOW: Calls callback for each chunk, propagates stream errors.
///
/// # Arguments
/// * `stream` - Response stream iterator
/// * `processor` - Callback function called for each chunk. Returns false to stop.
///
/// # Returns
/// * `Ok(ProcessStreamResult::Completed)` - All chunks processed successfully
/// * `Ok(ProcessStreamResult::StoppedByCallback)` - Callback returned false
/// * `Ok(ProcessStreamResult::NoBody)` - No body to process
/// * `Err(BodyReaderError)` - Stream read error
///
/// # Examples
///
/// ```rust
/// // Strict error handling
/// match process_streaming_body_strict(stream, |chunk| {
///     println!("Received {} bytes", chunk.len());
///     true // Continue
/// }) {
///     Ok(ProcessStreamResult::Completed) => println!("Done"),
///     Ok(ProcessStreamResult::StoppedByCallback) => println!("Stopped early"),
///     Ok(ProcessStreamResult::NoBody) => println!("No body"),
///     Err(BodyReaderError::StreamRead(e)) => handle_error(e),
/// }
/// ```
pub fn process_streaming_body_strict<F>(
    mut stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
    mut processor: F,
) -> Result<ProcessStreamResult, BodyReaderError>
where
    F: FnMut(&[u8]) -> bool, // Returns false to stop processing
{
    for part in stream {
        match part {
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                match body {
                    SendSafeBody::Text(t) => {
                        if processor(t.as_bytes()) {
                            return Ok(ProcessStreamResult::Completed);
                        } else {
                            return Ok(ProcessStreamResult::StoppedByCallback);
                        }
                    }
                    SendSafeBody::Bytes(b) => {
                        if processor(&b) {
                            return Ok(ProcessStreamResult::Completed);
                        } else {
                            return Ok(ProcessStreamResult::StoppedByCallback);
                        }
                    }
                    SendSafeBody::Stream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => {
                                        if !processor(&data) {
                                            return Ok(ProcessStreamResult::StoppedByCallback);
                                        }
                                    }
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(ProcessStreamResult::Completed);
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        if !processor(&data) {
                                            return Ok(ProcessStreamResult::StoppedByCallback);
                                        }
                                    }
                                    Ok(ChunkedData::DataEnded) => break,
                                    Ok(ChunkedData::Trailers(_)) => {}
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(ProcessStreamResult::Completed);
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(line) => {
                                        if !processor(line.as_bytes()) {
                                            return Ok(ProcessStreamResult::StoppedByCallback);
                                        }
                                    }
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        return Ok(ProcessStreamResult::Completed);
                    }
                    SendSafeBody::None => return Ok(ProcessStreamResult::NoBody),
                }
            }
            Ok(IncomingResponseParts::NoBody) => return Ok(ProcessStreamResult::NoBody),
            Ok(IncomingResponseParts::Intro(_, _, _)
            | IncomingResponseParts::Headers(_)
            | IncomingResponseParts::SKIP) => continue,
            Err(e) => return Err(BodyReaderError::StreamRead(e)),
        }
    }
    Ok(ProcessStreamResult::Completed)
}
```

#### Convenience Wrapper (Graceful Degradation)

```rust
/// Process streaming body with a callback - convenience wrapper.
///
/// WHY: Most callers just want to process the stream and don't need detailed error info.
///
/// WHAT: Returns `bool` - true if processing completed (or stopped by callback), false on error.
///
/// HOW: Wraps `process_streaming_body_strict()` and handles errors internally.
///
/// # Arguments
/// * `stream` - Response stream iterator
/// * `processor` - Callback function called for each chunk. Returns false to stop.
///
/// # Returns
/// * `true` - Processing completed successfully or stopped by callback
/// * `false` - Stream error occurred (logged)
///
/// # Examples
///
/// ```rust
/// // Simple usage - errors logged, false returned on failure
/// if process_streaming_body(stream, |chunk| {
///     println!("Received {} bytes", chunk.len());
///     true // Continue
/// }) {
///     println!("Processing complete");
/// } else {
///     // Check logs for error details
/// }
/// ```
pub fn process_streaming_body<F>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
    processor: F,
) -> bool
where
    F: FnMut(&[u8]) -> bool,
{
    match process_streaming_body_strict(stream, processor) {
        Ok(ProcessStreamResult::Completed)
        | Ok(ProcessStreamResult::StoppedByCallback)
        | Ok(ProcessStreamResult::NoBody) => true,
        Err(e) => {
            tracing::warn!("Stream processing error: {e}");
            false
        }
    }
}
```

#### Usage Comparison

```rust
// Strict error handling - caller distinguishes all outcomes
let result = process_streaming_body_strict(stream, |chunk| {
    hasher.update(chunk);
    true // Continue hashing
});

match result {
    Ok(ProcessStreamResult::Completed) => println!("Hash: {:?}", hasher.finalize()),
    Ok(ProcessStreamResult::StoppedByCallback) => println!("Stopped early"),
    Ok(ProcessStreamResult::NoBody) => println!("No body to hash"),
    Err(BodyReaderError::StreamRead(e)) => eprintln!("Read error: {}", e),
}

// Graceful degradation - simple success/failure
let mut hasher = Hasher::new();
if process_streaming_body(stream, |chunk| {
    hasher.update(chunk);
    true
}) {
    println!("Hash: {:?}", hasher.finalize());
} else {
    // Error already logged, use default
    println!("Hash: (unavailable)");
}
```

## HOW: Implementation

### File Structure

```
backends/foundation_core/src/wire/simple_http/client/
└── body_reader.rs (NEW - Body reading helpers)
```

### Implementation

```rust
//! Body reading utilities for HTTP responses.
//!
//! WHY: Provides reusable helpers for reading HTTP response bodies.
//!
//! WHAT: Exports core functions returning Result and convenience wrappers.
//!
//! HOW: Handles all SendSafeBody variants with graceful error handling.

use crate::wire::simple_http::{
    ChunkedData, HttpReaderError, IncomingResponseParts, SendSafeBody,
};
use serde::de::DeserializeOwned;

// === Error Types ===

/// Errors for string body reading.
#[derive(Debug, Display)]
pub enum StringBodyError {
    #[display("stream read error: {0}")]
    StreamRead(HttpReaderError),
    #[display("no body in response")]
    NoBody,
    #[display("invalid UTF-8: {0}")]
    InvalidUtf8(std::string::FromUtf8Error),
}

/// Errors for byte body reading.
#[derive(Debug, Display)]
pub enum BodyReaderError {
    #[display("stream read error: {0}")]
    StreamRead(HttpReaderError),
    #[display("no body in response")]
    NoBody,
    #[display("unexpected body variant")]
    UnexpectedVariant,
}

/// Errors for JSON parsing.
#[derive(Debug, Display)]
pub enum JsonParseError {
    #[display("failed to read body: {0}")]
    BodyRead(BodyReaderError),
    #[display("failed to parse JSON: {0}")]
    JsonParse(serde_json::Error),
}

// === String Body Reading ===

/// Read response body as a String - core function returning Result.
pub fn collect_string_strict(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<String, StringBodyError> {
    // Implementation as shown in Pattern 2
}

/// Read response body as a String - convenience wrapper.
pub fn collect_string(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> String {
    // Implementation as shown in Pattern 2
}

// === Byte Body Reading ===

/// Collect response body as raw bytes - core function returning Result.
pub fn collect_bytes_strict(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<Vec<u8>, BodyReaderError> {
    // Implementation as shown in Pattern 3
}

/// Collect response body as raw bytes - convenience wrapper.
pub fn collect_bytes(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Vec<u8> {
    // Implementation as shown in Pattern 3
}

/// Collect response body as raw bytes without any UTF-8 conversion.
/// Direct collection for maximum efficiency.
pub fn collect_bytes_direct(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Vec<u8> {
    // Implementation as shown in Pattern 3b
}

// === JSON Body Parsing ===

/// Read and parse JSON response body - core function returning Result.
pub fn parse_json_strict<T: DeserializeOwned>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<T, JsonParseError> {
    // Implementation as shown in Pattern 4
}

/// Read and parse JSON response body - convenience wrapper.
pub fn parse_json<T: DeserializeOwned>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Option<T> {
    // Implementation as shown in Pattern 4
}

// === Streaming Body Processing ===

/// Result of streaming body processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStreamResult {
    Completed,
    StoppedByCallback,
    NoBody,
}

/// Process streaming body with a callback - core function returning Result.
pub fn process_streaming_body_strict<F>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
    processor: F,
) -> Result<ProcessStreamResult, BodyReaderError>
where
    F: FnMut(&[u8]) -> bool,
{
    // Implementation as shown in Pattern 5
}

/// Process streaming body with a callback - convenience wrapper.
pub fn process_streaming_body<F>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
    processor: F,
) -> bool
where
    F: FnMut(&[u8]) -> bool,
{
    // Implementation as shown in Pattern 5
}
```

## Success Criteria

- [ ] All SendSafeBody variants documented with examples
- [ ] Error types defined (`StringBodyError`, `BodyReaderError`, `JsonParseError`)
- [ ] Core functions returning `Result` implemented:
  - [ ] `collect_string_strict()` - Returns `Result<String, StringBodyError>`
  - [ ] `collect_bytes_strict()` - Returns `Result<Vec<u8>, BodyReaderError>`
  - [ ] `parse_json_strict()` - Returns `Result<T, JsonParseError>`
  - [ ] `process_streaming_body_strict()` - Returns `Result<ProcessStreamResult, BodyReaderError>`
- [ ] Convenience wrappers implemented:
  - [ ] `collect_string()` - Returns `String`, empty on error
  - [ ] `collect_bytes()` - Returns `Vec<u8>`, empty on error
  - [ ] `collect_bytes_direct()` - Returns `Vec<u8>`, empty on error
  - [ ] `parse_json()` - Returns `Option<T>`, None on error
  - [ ] `process_streaming_body()` - Returns `bool`, false on error
- [ ] UTF-8 conversion with fallback demonstrated
- [ ] Iterator consumption patterns shown
- [ ] ChunkedData handling covered (including Trailers)
- [ ] Error handling is graceful (no panics)
- [ ] Usage comparison examples provided (strict vs. convenience)
- [ ] Module exported in `client::mod.rs`

## Verification Commands

```bash
# Build the module
cargo build --package foundation_core

# Run tests
cargo test --package foundation_core -- wire::simple_http::client::body_reader

# Check formatting
cargo fmt -- --check

# Run clippy
cargo clippy --package foundation_core -- -D warnings
```

## Notes for Agents

### Important Considerations

1. **UTF-8 Fallback**: When converting bytes to String, use `unwrap_or_else()` to provide a fallback rather than panicking.

2. **Stream Consumption**: Always consume the entire stream iterator, even if you only need partial data. This ensures proper connection cleanup.

3. **Chunked Trailers**: HTTP chunked responses may include trailers after the data. These should be silently ignored for simple body reading.

4. **Error Logging**: Log errors with `tracing::warn!` but return graceful defaults (empty string, empty vec) rather than propagating errors.

### Common Pitfalls

1. Not handling all SendSafeBody variants (especially LineFeedStream)
2. Panicking on invalid UTF-8 instead of using fallback
3. Not consuming the entire stream (connection leak)
4. Not handling chunked trailers
5. Forgetting to handle the `None` body case

---

_Created: 2026-03-25_
_Source: gen_model_descriptors body handling analysis_
