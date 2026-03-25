//! Body reading utilities for HTTP responses.
//!
//! # Overview
//!
//! This module provides reusable helpers for reading HTTP response bodies.
//! It follows a dual-API pattern:
//!
//! - **Strict functions** (`*_strict`) - Return `Result<T, Error>` for proper error handling
//! - **Convenience wrappers** - Return `T` with graceful degradation (empty/default on error)
//!
//! # Example Usage
//!
//! ```rust
//! // Strict error handling - caller decides what to do
//! match collect_bytes_strict(stream) {
//!     Ok(bytes) => process(bytes),
//!     Err(BodyReaderError::NoBody) => handle_no_body(),
//!     Err(BodyReaderError::StreamRead(e)) => handle_error(e),
//! }
//!
//! // Graceful degradation - errors logged, empty result on failure
//! let bytes = collect_bytes(stream);
//! if bytes.is_empty() {
//!     // Check logs for error details
//! }
//! ```

use crate::extensions::result_ext::{BoxedError, SendableBoxedError};
use crate::wire::simple_http::{
    ChunkedData, HttpReaderError, IncomingResponseParts, SendSafeBody,
};
use serde::de::DeserializeOwned;

// ============================================================================
// Error Types
// ============================================================================

/// Errors for string body reading.
///
/// WHY: Provides structured, actionable error reporting for string body operations.
///
/// WHAT: Covers stream read errors, no body, UTF-8 conversion failures, and nested iterator errors.
///
/// HOW: Uses `derive_more` for Display/From to avoid boilerplate.
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum StringBodyError {
    /// Stream read error
    #[display("stream read error: {_0}")]
    StreamRead(HttpReaderError),
    /// No body in response
    #[display("no body in response")]
    NoBody,
    /// Invalid UTF-8 in response
    #[display("invalid UTF-8: {_0}")]
    InvalidUtf8(std::string::FromUtf8Error),
    /// Nested iterator error (chunked/line streams) - wrapped to preserve context
    #[display("stream iterator error: {_0}")]
    StreamIteratorError(SendableBoxedError),
}

impl std::error::Error for StringBodyError {}

/// Errors for byte body reading.
///
/// WHY: Provides structured, actionable error reporting for byte body operations.
///
/// WHAT: Covers stream read errors, no body, unexpected body variants, and nested iterator errors.
///
/// HOW: Uses `derive_more` for Display/From to avoid boilerplate.
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum BodyReaderError {
    /// Stream read error
    #[display("stream read error: {_0}")]
    StreamRead(HttpReaderError),
    /// No body in response
    #[display("no body in response")]
    NoBody,
    /// Unexpected body variant
    #[display("unexpected body variant")]
    UnexpectedVariant,
    /// Nested iterator error (chunked/line streams) - wrapped to preserve context
    #[display("stream iterator error: {_0}")]
    StreamIteratorError(SendableBoxedError),
}

impl std::error::Error for BodyReaderError {}

/// Errors for JSON parsing.
///
/// WHY: Distinguishes between body read errors and JSON parse errors.
///
/// WHAT: Covers both failure modes with proper error chaining.
///
/// HOW: Wraps underlying errors for debugging.
#[derive(Debug, derive_more::Display, derive_more::From)]
pub enum JsonParseError {
    /// Failed to read response body
    #[display("failed to read body: {_0}")]
    BodyRead(BodyReaderError),
    /// Failed to parse JSON
    #[display("failed to parse JSON: {_0}")]
    JsonParse(serde_json::Error),
}

impl std::error::Error for JsonParseError {}

// ============================================================================
// Result Types
// ============================================================================

/// Result of streaming body processing.
///
/// WHY: Distinguishes between successful completion, callback-initiated stops,
/// and actual errors.
///
/// WHAT: Simple enum for streaming operation outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStreamResult {
    /// Processing completed successfully
    Completed,
    /// Processing stopped by callback (returned false)
    StoppedByCallback,
    /// No body to process
    NoBody,
}

// ============================================================================
// String Body Reading
// ============================================================================

/// Read response body as a String - core function returning Result.
///
/// WHY: Provides proper error handling so callers can decide how to handle failures.
///
/// WHAT: Returns `Result<String, StringBodyError>` with specific error information.
///
/// HOW: Handles all SendSafeBody variants, propagates errors.
///
/// # Arguments
///
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
///
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
    for part in stream {
        match part {
            // Handle sized and streamed bodies
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                return match body {
                    SendSafeBody::Text(t) => Ok(t),
                    SendSafeBody::Bytes(b) => {
                        String::from_utf8(b.clone()).map_err(StringBodyError::InvalidUtf8)
                    }
                    SendSafeBody::Stream(mut opt_iter) => {
                        let mut bytes = Vec::new();
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => return Err(StringBodyError::StreamIteratorError(SendableBoxedError::from(e))),
                                }
                            }
                        }
                        String::from_utf8(bytes).map_err(StringBodyError::InvalidUtf8)
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        let mut bytes = Vec::new();
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        bytes.extend_from_slice(&data);
                                    }
                                    Ok(ChunkedData::Trailers(_)) => {
                                        // Silently ignore trailers
                                    }
                                    Ok(ChunkedData::DataEnded) => break,
                                    Err(e) => return Err(StringBodyError::StreamIteratorError(SendableBoxedError::from(e))),
                                }
                            }
                        }
                        String::from_utf8(bytes).map_err(StringBodyError::InvalidUtf8)
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        let mut lines = Vec::new();
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(line) => lines.push(line),
                                    Err(e) => return Err(StringBodyError::StreamRead(e)),
                                }
                            }
                        }
                        Ok(lines.join("\n"))
                    }
                    SendSafeBody::None => Err(StringBodyError::NoBody),
                };
            }
            // Skip intro and headers - we want the body
            Ok(IncomingResponseParts::Intro(_, _, _))
            | Ok(IncomingResponseParts::Headers(_))
            | Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Err(StringBodyError::NoBody),
            // Stream error
            Err(e) => return Err(StringBodyError::StreamRead(e)),
        }
    }
    Err(StringBodyError::NoBody)
}

/// Read response body as a String - convenience wrapper.
///
/// WHY: Most callers just want the body text and don't need to handle errors explicitly.
///
/// WHAT: Returns `String`, logging warnings and returning empty string on error.
///
/// HOW: Wraps `collect_string_strict()` and handles errors internally.
///
/// # Arguments
///
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
///
/// The response body as a String. Returns empty string on error or no body.
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

// ============================================================================
// Byte Body Reading
// ============================================================================

/// Collect response body as raw bytes - core function returning Result.
///
/// WHY: Provides proper error handling for binary data operations.
///
/// WHAT: Returns `Result<Vec<u8>, BodyReaderError>` with specific error information.
///
/// HOW: Handles all SendSafeBody variants, propagates errors.
///
/// # Arguments
///
/// * `stream` - Iterator over IncomingResponseParts
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - Raw bytes from response
/// * `Err(BodyReaderError::StreamRead(e))` - Stream read error
/// * `Err(BodyReaderError::NoBody)` - No body in response
///
/// # Examples
///
/// ```rust
/// // Strict error handling for binary data
/// match collect_bytes_strict(stream) {
///     Ok(bytes) => save_to_file(bytes),
///     Err(BodyReaderError::NoBody) => handle_no_body(),
///     Err(BodyReaderError::StreamRead(e)) => handle_error(e),
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
                return match body {
                    SendSafeBody::Text(t) => Ok(t.into_bytes()),
                    SendSafeBody::Bytes(b) => Ok(b.clone()),
                    SendSafeBody::Stream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        Ok(bytes)
                    }
                    SendSafeBody::ChunkedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(ChunkedData::Data(data, _)) => {
                                        bytes.extend_from_slice(&data);
                                    }
                                    Ok(ChunkedData::Trailers(_)) => {
                                        // Silently ignore trailers
                                    }
                                    Ok(ChunkedData::DataEnded) => break,
                                    Err(e) => return Err(BodyReaderError::StreamRead(e)),
                                }
                            }
                        }
                        Ok(bytes)
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
                        Ok(bytes)
                    }
                    SendSafeBody::None => Err(BodyReaderError::NoBody),
                };
            }
            Ok(IncomingResponseParts::Intro(_, _, _))
            | Ok(IncomingResponseParts::Headers(_))
            | Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Err(BodyReaderError::NoBody),
            Err(e) => return Err(BodyReaderError::StreamRead(e)),
        }
    }
    Ok(bytes)
}

/// Collect response body as raw bytes - convenience wrapper.
///
/// WHY: Most callers just want the bytes and don't need to handle errors explicitly.
///
/// WHAT: Returns `Vec<u8>`, logging warnings and returning empty vec on error.
///
/// HOW: Wraps `collect_bytes_strict()` and handles errors internally.
///
/// # Arguments
///
/// * `stream` - Response stream iterator
///
/// # Returns
///
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

/// Collect response body as raw bytes without any UTF-8 conversion overhead.
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
///
/// * `stream` - Response stream iterator
///
/// # Returns
///
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
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => match body {
                SendSafeBody::Text(t) => return t.into_bytes(),
                SendSafeBody::Bytes(b) => return b.clone(),
                SendSafeBody::Stream(mut opt_iter) => {
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
                    if let Some(iter) = opt_iter.take() {
                        for chunk_result in iter {
                            match chunk_result {
                                Ok(ChunkedData::Data(data, _)) => {
                                    bytes.extend_from_slice(&data);
                                }
                                Ok(ChunkedData::Trailers(_)) => {
                                    // Silently ignore trailers
                                }
                                Ok(ChunkedData::DataEnded) => break,
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
            },
            Ok(IncomingResponseParts::Intro(_, _, _))
            | Ok(IncomingResponseParts::Headers(_))
            | Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Vec::new(),
            Err(e) => {
                tracing::warn!("Error reading stream for byte collection: {e}");
                return Vec::new();
            }
        }
    }

    bytes
}

// ============================================================================
// JSON Body Parsing
// ============================================================================

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
///
/// * `T` - Target type for deserialization
///
/// # Arguments
///
/// * `stream` - Response stream iterator
///
/// # Returns
///
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
pub fn parse_json_strict<T: DeserializeOwned>(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<T, JsonParseError> {
    let body_text = collect_string_strict(stream).map_err(JsonParseError::BodyRead)?;
    serde_json::from_str(&body_text).map_err(JsonParseError::JsonParse)
}

/// Read and parse JSON response body - convenience wrapper.
///
/// WHY: Most callers just want the parsed data and don't need to handle errors explicitly.
///
/// WHAT: Returns `Option<T>`, logging warnings and returning None on error.
///
/// HOW: Wraps `parse_json_strict()` and handles errors internally.
///
/// # Type Parameters
///
/// * `T` - Target type for deserialization
///
/// # Arguments
///
/// * `stream` - Response stream iterator
///
/// # Returns
///
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

// ============================================================================
// Streaming Body Processing
// ============================================================================

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
///
/// * `stream` - Response stream iterator
/// * `processor` - Callback function called for each chunk. Returns false to stop.
///
/// # Returns
///
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
    F: FnMut(&[u8]) -> bool,
{
    for part in stream {
        match part {
            Ok(IncomingResponseParts::SizedBody(body))
            | Ok(IncomingResponseParts::StreamedBody(body)) => {
                return match body {
                    SendSafeBody::Text(t) => {
                        if processor(t.as_bytes()) {
                            Ok(ProcessStreamResult::Completed)
                        } else {
                            Ok(ProcessStreamResult::StoppedByCallback)
                        }
                    }
                    SendSafeBody::Bytes(b) => {
                        if processor(&b) {
                            Ok(ProcessStreamResult::Completed)
                        } else {
                            Ok(ProcessStreamResult::StoppedByCallback)
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
                        Ok(ProcessStreamResult::Completed)
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
                        Ok(ProcessStreamResult::Completed)
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
                        Ok(ProcessStreamResult::Completed)
                    }
                    SendSafeBody::None => Ok(ProcessStreamResult::NoBody),
                };
            }
            Ok(IncomingResponseParts::Intro(_, _, _))
            | Ok(IncomingResponseParts::Headers(_))
            | Ok(IncomingResponseParts::SKIP) => continue,
            Ok(IncomingResponseParts::NoBody) => return Ok(ProcessStreamResult::NoBody),
            Err(e) => return Err(BodyReaderError::StreamRead(e)),
        }
    }
    Ok(ProcessStreamResult::Completed)
}

/// Process streaming body with a callback - convenience wrapper.
///
/// WHY: Most callers just want to process the stream and don't need detailed error info.
///
/// WHAT: Returns `bool` - true if processing completed (or stopped by callback), false on error.
///
/// HOW: Wraps `process_streaming_body_strict()` and handles errors internally.
///
/// # Arguments
///
/// * `stream` - Response stream iterator
/// * `processor` - Callback function called for each chunk. Returns false to stop.
///
/// # Returns
///
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_stream_result_debug() {
        // Basic smoke test for the Result type
        let result = ProcessStreamResult::Completed;
        assert_eq!(result, ProcessStreamResult::Completed);
    }
}
