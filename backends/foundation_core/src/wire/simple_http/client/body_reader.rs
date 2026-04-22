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
//! ```rust,ignore
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

use crate::extensions::result_ext::BoxedError;
use crate::wire::simple_http::{
    ChunkedData, HttpReaderError, IncomingResponseParts, LineFeed, SendSafeBody,
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
    /// Nested iterator error (chunked/line streams) - error message preserved
    #[display("stream iterator error: {_0}")]
    StreamIteratorError(Box<str>),
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
    /// Nested iterator error (chunked/line streams) - error message preserved
    #[display("stream iterator error: {_0}")]
    StreamIteratorError(Box<str>),
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
    BodyRead(StringBodyError),
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
/// HOW: Handles all `SendSafeBody` variants, propagates errors.
///
/// # Arguments
///
/// * `stream` - Iterator over `IncomingResponseParts`
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
/// ```rust,ignore,ignore
/// // Strict error handling
/// match collect_string_strict(stream) {
///     Ok(body) => process(body),
///     Err(StringBodyError::NoBody) => handle_no_body(),
///     Err(StringBodyError::StreamRead(e)) => handle_error(e),
///     Err(StringBodyError::InvalidUtf8(e)) => handle_utf8_error(e),
/// }
/// ```
pub fn collect_string_strict(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<String, StringBodyError> {
    for part in stream {
        match part {
            // Handle sized and streamed bodies
            Ok(
                IncomingResponseParts::SizedBody(body) | IncomingResponseParts::StreamedBody(body),
            ) => {
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
                                    Err(e) => {
                                        return Err(StringBodyError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
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
                                    Err(e) => {
                                        return Err(StringBodyError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
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
                                    Ok(LineFeed::Line(line)) => lines.push(line),
                                    Ok(LineFeed::SKIP | LineFeed::END) => continue,
                                    Err(e) => {
                                        return Err(StringBodyError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
                                }
                            }
                        }
                        Ok(lines.join("\n"))
                    }
                    SendSafeBody::None => Err(StringBodyError::NoBody),
                };
            }
            // Skip intro and headers - we want the body
            Ok(
                IncomingResponseParts::Intro(_, _, _)
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => continue,
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
/// * `stream` - Iterator over `IncomingResponseParts`
///
/// # Returns
///
/// The response body as a String. Returns empty string on error or no body.
///
/// # Examples
///
/// ```rust,ignore
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
/// HOW: Handles all `SendSafeBody` variants, propagates errors.
///
/// # Arguments
///
/// * `stream` - Iterator over `IncomingResponseParts`
///
/// # Returns
///
/// * `Ok(Vec<u8>)` - Raw bytes from response
/// * `Err(BodyReaderError::StreamRead(e))` - Stream read error
/// * `Err(BodyReaderError::NoBody)` - No body in response
///
/// # Examples
///
/// ```rust,ignore
/// // Strict error handling for binary data
/// match collect_bytes_strict(stream) {
///     Ok(bytes) => save_to_file(bytes),
///     Err(BodyReaderError::NoBody) => handle_no_body(),
///     Err(BodyReaderError::StreamRead(e)) => handle_error(e),
/// }
/// ```
pub fn collect_bytes_strict(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<Vec<u8>, BodyReaderError> {
    let mut bytes = Vec::new();

    for part in stream {
        match part {
            Ok(
                IncomingResponseParts::SizedBody(body) | IncomingResponseParts::StreamedBody(body),
            ) => {
                return match body {
                    SendSafeBody::Text(t) => Ok(t.into_bytes()),
                    SendSafeBody::Bytes(b) => Ok(b.clone()),
                    SendSafeBody::Stream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for chunk_result in iter {
                                match chunk_result {
                                    Ok(data) => bytes.extend_from_slice(&data),
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
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
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
                                }
                            }
                        }
                        Ok(bytes)
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(LineFeed::Line(line)) => {
                                        bytes.extend_from_slice(line.as_bytes());
                                    }
                                    Ok(LineFeed::SKIP | LineFeed::END) => continue,
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
                                }
                            }
                        }
                        Ok(bytes)
                    }
                    SendSafeBody::None => Err(BodyReaderError::NoBody),
                };
            }
            Ok(
                IncomingResponseParts::Intro(_, _, _)
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => continue,
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
/// `Vec<u8>` containing raw bytes. Returns empty vec on error or no body.
///
/// # Examples
///
/// ```rust,ignore
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
/// `Vec<u8>` containing raw response bytes. Returns empty vec on error or no body.
///
/// # Examples
///
/// ```rust,ignore
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
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Vec<u8> {
    let mut bytes = Vec::new();

    for part in stream {
        match part {
            Ok(
                IncomingResponseParts::SizedBody(body) | IncomingResponseParts::StreamedBody(body),
            ) => match body {
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
                                Ok(LineFeed::Line(line)) => {
                                    bytes.extend_from_slice(line.as_bytes());
                                    bytes.push(b'\n');
                                }
                                Ok(LineFeed::SKIP | LineFeed::END) => continue,
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
            Ok(
                IncomingResponseParts::Intro(_, _, _)
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => continue,
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
// Internal Helper Functions for SendSafeBody Processing
// ============================================================================

/// Process a stream iterator, collecting bytes into a Vec.
/// Used internally by collect_bytes_from_send_safe for Stream variant.
fn collect_from_stream<I>(iter: I) -> Vec<u8>
where
    I: Iterator<Item = Result<Vec<u8>, BoxedError>>,
{
    let mut bytes = Vec::new();
    for chunk_result in iter {
        match chunk_result {
            Ok(data) => bytes.extend_from_slice(&data),
            Err(e) => {
                tracing::warn!("Stream error during byte collection: {e}");
                break;
            }
        }
    }
    bytes
}

/// Process a chunked stream iterator, collecting bytes into a Vec.
/// Used internally by collect_bytes_from_send_safe for ChunkedStream variant.
fn collect_from_chunked_stream<I>(iter: I) -> Vec<u8>
where
    I: Iterator<Item = Result<ChunkedData, BoxedError>>,
{
    let mut bytes = Vec::new();
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
    bytes
}

/// Process a line-feed stream iterator, collecting bytes into a Vec.
/// Used internally by collect_bytes_from_send_safe for LineFeedStream variant.
fn collect_from_linefeed_stream<I>(iter: I) -> Vec<u8>
where
    I: Iterator<Item = Result<LineFeed, BoxedError>>,
{
    let mut bytes = Vec::new();
    for line_result in iter {
        match line_result {
            Ok(LineFeed::Line(line)) => {
                bytes.extend_from_slice(line.as_bytes());
                bytes.push(b'\n');
            }
            Ok(LineFeed::SKIP | LineFeed::END) => continue,
            Err(e) => {
                tracing::warn!("Line stream error: {e}");
                break;
            }
        }
    }
    bytes
}

/// Process a stream iterator, writing bytes to a writer.
/// Returns total bytes written or first error encountered.
fn write_from_stream<I, W>(
    iter: I,
    writer: &mut W,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    I: Iterator<Item = Result<Vec<u8>, BoxedError>>,
    W: std::io::Write,
{
    let mut total_bytes: u64 = 0;
    for chunk_result in iter {
        match chunk_result {
            Ok(data) => {
                writer.write_all(&data)?;
                total_bytes += data.len() as u64;
            }
            Err(e) => {
                return Err(format!("Stream error during write: {e}").into());
            }
        }
    }
    Ok(total_bytes)
}

/// Process a chunked stream iterator, writing bytes to a writer.
/// Returns total bytes written or first error encountered.
fn write_from_chunked_stream<I, W>(
    iter: I,
    writer: &mut W,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    I: Iterator<Item = Result<ChunkedData, BoxedError>>,
    W: std::io::Write,
{
    let mut total_bytes: u64 = 0;
    for chunk_result in iter {
        match chunk_result {
            Ok(ChunkedData::Data(data, _)) => {
                writer.write_all(&data)?;
                total_bytes += data.len() as u64;
            }
            Ok(ChunkedData::Trailers(_)) => {
                // Silently ignore trailers
            }
            Ok(ChunkedData::DataEnded) => break,
            Err(e) => {
                return Err(format!("Chunked stream error during write: {e}").into());
            }
        }
    }
    Ok(total_bytes)
}

/// Process a line-feed stream iterator, writing bytes to a writer.
/// Returns total bytes written or first error encountered.
fn write_from_linefeed_stream<I, W>(
    iter: I,
    writer: &mut W,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    I: Iterator<Item = Result<LineFeed, BoxedError>>,
    W: std::io::Write,
{
    let mut total_bytes: u64 = 0;
    for line_result in iter {
        match line_result {
            Ok(LineFeed::Line(line)) => {
                writer.write_all(line.as_bytes())?;
                total_bytes += line.len() as u64;
                writer.write_all(b"\n")?;
                total_bytes += 1;
            }
            Ok(LineFeed::SKIP | LineFeed::END) => continue,
            Err(e) => {
                return Err(format!("Line stream error during write: {e}").into());
            }
        }
    }
    Ok(total_bytes)
}

// ============================================================================
// SendSafeBody Byte Collection
// ============================================================================

/// Collect response body as bytes directly from a `SendSafeBody`.
///
/// WHY: When you already have a `SendSafeBody` (e.g., from a collected response),
/// this provides a unified way to extract bytes without repeating match logic.
///
/// WHAT: Handles all `SendSafeBody` variants (Text, Bytes, Stream, ChunkedStream, LineFeedStream, None).
///
/// HOW: Matches on the body variant and collects bytes appropriately.
/// Returns empty Vec for None body.
///
/// # Arguments
///
/// * `body` - The `SendSafeBody` to collect bytes from
///
/// # Returns
///
/// `Vec<u8>` containing the body bytes. Returns empty vec for NoBody.
///
/// # Examples
///
/// ```rust,ignore
/// let response = client.get(url).send()?;
/// let bytes = collect_bytes_from_send_safe(response.take_body());
/// ```
pub fn collect_bytes_from_send_safe(body: SendSafeBody) -> Vec<u8> {
    match body {
        SendSafeBody::Text(t) => t.into_bytes(),
        SendSafeBody::Bytes(b) => b,
        SendSafeBody::None => Vec::new(),
        SendSafeBody::Stream(mut opt_iter) => {
            opt_iter.take().map_or(Vec::new(), collect_from_stream)
        }
        SendSafeBody::ChunkedStream(mut opt_iter) => opt_iter
            .take()
            .map_or(Vec::new(), collect_from_chunked_stream),
        SendSafeBody::LineFeedStream(mut opt_iter) => opt_iter
            .take()
            .map_or(Vec::new(), collect_from_linefeed_stream),
    }
}

/// Stream response body from `SendSafeBody` directly into an `io::Write` implementer.
///
/// WHY: For large files or streaming scenarios, writing directly to a file or socket
/// avoids allocating the entire body in memory.
///
/// WHAT: Handles all `SendSafeBody` variants and streams data directly to the writer.
/// Returns errors to the caller for proper error handling.
///
/// HOW: Matches on the body variant and writes chunks directly to the writer.
/// Returns the total bytes written on success, or the first error encountered.
///
/// # Type Parameters
///
/// * `W` - Writer type implementing `std::io::Write`
///
/// # Arguments
///
/// * `body` - The `SendSafeBody` to stream from
/// * `writer` - The writer to stream data into
///
/// # Returns
///
/// `Result<u64, Box<dyn std::error::Error + Send + Sync>>` - Total bytes written on success, error on failure.
///
/// # Examples
///
/// ```rust,ignore
/// // Download directly to file
/// let response = client.get(url).send()?;
/// let (_, _, body, ..) = response.into_parts();
/// let mut file = std::fs::File::create("download.bin")?;
/// let bytes_written = collect_bytes_into(body, &mut file)?;
/// ```
pub fn collect_bytes_into<W: std::io::Write>(
    body: SendSafeBody,
    writer: &mut W,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let mut total_bytes: u64 = 0;

    match body {
        SendSafeBody::Text(t) => {
            let bytes = t.as_bytes();
            writer.write_all(bytes)?;
            total_bytes = bytes.len() as u64;
        }
        SendSafeBody::Bytes(b) => {
            writer.write_all(&b)?;
            total_bytes = b.len() as u64;
        }
        SendSafeBody::None => {
            // No body to write
        }
        SendSafeBody::Stream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                total_bytes = write_from_stream(iter, writer)?;
            }
        }
        SendSafeBody::ChunkedStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                total_bytes = write_from_chunked_stream(iter, writer)?;
            }
        }
        SendSafeBody::LineFeedStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                total_bytes = write_from_linefeed_stream(iter, writer)?;
            }
        }
    }

    Ok(total_bytes)
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
/// ```rust,ignore
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
/// ```rust,ignore
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
// Parser Trait and Helpers
// ============================================================================

/// Parser trait for HTTP response bodies.
///
/// WHY: Provides a reusable abstraction for parsing HTTP responses with
/// consistent error handling and source identification.
///
/// WHAT: Trait for types that can be parsed from HTTP response bodies.
///
/// HOW: Implement for your response types, use with `parse_json` helper.
/// Graceful degradation - returns empty/default on error, never panics.
///
/// # Examples
///
/// ```rust,ignore
/// use foundation_core::wire::simple_http::client::body_reader::ResponseParser;
///
/// #[derive(serde::Deserialize)]
/// struct ApiResponse {
///     data: Vec<Item>,
/// }
///
/// impl ResponseParser for ApiResponse {
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

/// Generic JSON parser helper for string bodies.
///
/// WHY: Convenience helper for parsing JSON from string bodies with logging.
///
/// WHAT: Parses JSON from a string body with source identification.
///
/// HOW: Wraps `serde_json::from_str` with error logging.
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
///
/// # Examples
///
/// ```rust,ignore
/// if let Some(data) = parse_json::<MyType>(&body, "api.example.com") {
///     // Process parsed data
/// } else {
///     // Handle parse failure (already logged)
/// }
/// ```
pub fn parse_json_str<T: DeserializeOwned>(body: &str, source: &str) -> Option<T> {
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
/// WHY: Sometimes you need a custom fallback value instead of None/default.
///
/// WHAT: Parses JSON, calling fallback function on error.
///
/// HOW: Wraps `serde_json::from_str` with custom fallback.
///
/// # Type Parameters
/// * `T` - Target type for deserialization
/// * `F` - Fallback function type
///
/// # Arguments
/// * `body` - Response body to parse
/// * `source` - Source identifier for logging
/// * `fallback` - Function to produce fallback value on error
///
/// # Returns
/// Parsed value or fallback result.
///
/// # Examples
///
/// ```rust,ignore
/// let config = parse_with_fallback(
///     &body,
///     "config-api",
///     || Config::default(),
/// );
/// ```
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
/// ```rust,ignore
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
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
    mut processor: F,
) -> Result<ProcessStreamResult, BodyReaderError>
where
    F: FnMut(&[u8]) -> bool,
{
    for part in stream {
        match part {
            Ok(
                IncomingResponseParts::SizedBody(body) | IncomingResponseParts::StreamedBody(body),
            ) => {
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
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
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
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
                                }
                            }
                        }
                        Ok(ProcessStreamResult::Completed)
                    }
                    SendSafeBody::LineFeedStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for line_result in iter {
                                match line_result {
                                    Ok(LineFeed::Line(line)) => {
                                        if !processor(line.as_bytes()) {
                                            return Ok(ProcessStreamResult::StoppedByCallback);
                                        }
                                    }
                                    Ok(LineFeed::SKIP | LineFeed::END) => continue,
                                    Err(e) => {
                                        return Err(BodyReaderError::StreamIteratorError(
                                            e.to_string().into_boxed_str(),
                                        ))
                                    }
                                }
                            }
                        }
                        Ok(ProcessStreamResult::Completed)
                    }
                    SendSafeBody::None => Ok(ProcessStreamResult::NoBody),
                };
            }
            Ok(
                IncomingResponseParts::Intro(_, _, _)
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => continue,
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
/// ```rust,ignore
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
        Ok(
            ProcessStreamResult::Completed
            | ProcessStreamResult::StoppedByCallback
            | ProcessStreamResult::NoBody,
        ) => true,
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
    use crate::extensions::result_ext::{BoxedError, SendableBoxedError};
    use serde::Deserialize;

    #[test]
    fn test_process_stream_result_debug() {
        // Basic smoke test for the Result type
        let result = ProcessStreamResult::Completed;
        assert_eq!(result, ProcessStreamResult::Completed);
    }

    #[test]
    fn test_parse_json_str() {
        let json = r#"{"name": "test", "value": 42}"#;

        #[derive(Deserialize, Debug, PartialEq)]
        struct TestObj {
            name: String,
            value: i32,
        }

        let result: Option<TestObj> = parse_json_str(json, "test-source");
        assert!(result.is_some());
        let obj = result.unwrap();
        assert_eq!(obj.name, "test");
        assert_eq!(obj.value, 42);
    }

    #[test]
    fn test_parse_json_str_invalid() {
        let invalid_json = r#"{"name": "test", invalid}"#;

        #[derive(Deserialize, Debug, PartialEq)]
        struct TestObj {
            name: String,
            value: i32,
        }

        let result: Option<TestObj> = parse_json_str(invalid_json, "test-source");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_with_fallback_success() {
        let json = r#"{"items": [1, 2, 3]}"#;

        #[derive(Deserialize, Debug, PartialEq)]
        struct TestResponse {
            items: Vec<i32>,
        }

        let fallback_called = std::cell::RefCell::new(false);
        let result = parse_with_fallback(json, "test-source", || {
            *fallback_called.borrow_mut() = true;
            TestResponse { items: vec![] }
        });

        assert!(!*fallback_called.borrow());
        assert_eq!(result.items, vec![1, 2, 3]);
    }

    #[test]
    fn test_parse_with_fallback_error() {
        let invalid_json = r#"{"items": invalid}"#;

        #[derive(Deserialize, Debug, PartialEq)]
        struct TestResponse {
            items: Vec<i32>,
        }

        let fallback_called = std::cell::RefCell::new(false);
        let result = parse_with_fallback(invalid_json, "test-source", || {
            *fallback_called.borrow_mut() = true;
            TestResponse { items: vec![0] }
        });

        assert!(*fallback_called.borrow());
        assert_eq!(result.items, vec![0]);
    }

    #[test]
    fn test_response_parser_trait() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct ApiResponse {
            data: Vec<String>,
        }

        impl ResponseParser for ApiResponse {
            type Output = Vec<String>;

            fn parse(body: &str, _source: &str) -> Self::Output {
                match serde_json::from_str::<Self>(body) {
                    Ok(response) => response.data,
                    Err(_) => Vec::new(),
                }
            }
        }

        let valid_json = r#"{"data": ["a", "b", "c"]}"#;
        let result = ApiResponse::parse(valid_json, "test");
        assert_eq!(result, vec!["a", "b", "c"]);

        let invalid_json = r#"{"data": invalid}"#;
        let result = ApiResponse::parse(invalid_json, "test");
        assert_eq!(result, Vec::<String>::new());
    }

    // ========================================================================
    // Tests for internal helper functions
    // ========================================================================

    /// Helper to create a BoxedError for testing
    fn make_error(msg: &str) -> BoxedError {
        msg.to_string().into()
    }

    #[test]
    fn test_collect_from_stream_success() {
        let data = vec![
            Ok(b"hello".to_vec()),
            Ok(b" ".to_vec()),
            Ok(b"world".to_vec()),
        ];
        let result = collect_from_stream(data.into_iter());
        assert_eq!(result, b"hello world".to_vec());
    }

    #[test]
    fn test_collect_from_stream_error() {
        let data: Vec<Result<Vec<u8>, BoxedError>> =
            vec![Ok(b"hello".to_vec()), Err(make_error("stream error"))];
        let result = collect_from_stream(data.into_iter());
        // Should collect what it got before error
        assert_eq!(result, b"hello".to_vec());
    }

    #[test]
    fn test_collect_from_stream_empty() {
        let data: Vec<Result<Vec<u8>, BoxedError>> = vec![];
        let result = collect_from_stream(data.into_iter());
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_from_chunked_stream_success() {
        let data = vec![
            Ok(ChunkedData::Data(b"chunk1".to_vec(), None)),
            Ok(ChunkedData::Data(b"chunk2".to_vec(), None)),
            Ok(ChunkedData::DataEnded),
        ];
        let result = collect_from_chunked_stream(data.into_iter());
        assert_eq!(result, b"chunk1chunk2".to_vec());
    }

    #[test]
    fn test_collect_from_chunked_stream_with_trailers() {
        let data = vec![
            Ok(ChunkedData::Data(b"data".to_vec(), None)),
            Ok(ChunkedData::Trailers(vec![])),
            Ok(ChunkedData::DataEnded),
        ];
        let result = collect_from_chunked_stream(data.into_iter());
        assert_eq!(result, b"data".to_vec());
    }

    #[test]
    fn test_collect_from_chunked_stream_error() {
        let data: Vec<Result<ChunkedData, BoxedError>> = vec![
            Ok(ChunkedData::Data(b"hello".to_vec(), None)),
            Err(make_error("chunked error")),
        ];
        let result = collect_from_chunked_stream(data.into_iter());
        assert_eq!(result, b"hello".to_vec());
    }

    #[test]
    fn test_collect_from_linefeed_stream_success() {
        let data = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Ok(LineFeed::Line("line2".to_string())),
            Ok(LineFeed::END),
        ];
        let result = collect_from_linefeed_stream(data.into_iter());
        // Each line gets \n appended
        assert_eq!(result, b"line1\nline2\n".to_vec());
    }

    #[test]
    fn test_collect_from_linefeed_stream_with_skip() {
        let data = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Ok(LineFeed::SKIP),
            Ok(LineFeed::Line("line2".to_string())),
            Ok(LineFeed::END),
        ];
        let result = collect_from_linefeed_stream(data.into_iter());
        assert_eq!(result, b"line1\nline2\n".to_vec());
    }

    #[test]
    fn test_collect_from_linefeed_stream_error() {
        let data: Vec<Result<LineFeed, BoxedError>> = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Err(make_error("line error")),
        ];
        let result = collect_from_linefeed_stream(data.into_iter());
        assert_eq!(result, b"line1\n".to_vec());
    }

    // ========================================================================
    // Tests for write_* helper functions
    // ========================================================================

    #[test]
    fn test_write_from_stream_success() {
        let data = vec![Ok(b"hello".to_vec()), Ok(b" world".to_vec())];
        let mut output = Vec::new();
        let result = write_from_stream(data.into_iter(), &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 11);
        assert_eq!(output, b"hello world".to_vec());
    }

    #[test]
    fn test_write_from_stream_error() {
        let data: Vec<Result<Vec<u8>, BoxedError>> =
            vec![Ok(b"hello".to_vec()), Err(make_error("write error"))];
        let mut output = Vec::new();
        let result = write_from_stream(data.into_iter(), &mut output);
        assert!(result.is_err());
        assert_eq!(output, b"hello".to_vec());
    }

    #[test]
    fn test_write_from_chunked_stream_success() {
        let data = vec![
            Ok(ChunkedData::Data(b"chunk1".to_vec(), None)),
            Ok(ChunkedData::Data(b"chunk2".to_vec(), None)),
            Ok(ChunkedData::DataEnded),
        ];
        let mut output = Vec::new();
        let result = write_from_chunked_stream(data.into_iter(), &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);
        assert_eq!(output, b"chunk1chunk2".to_vec());
    }

    #[test]
    fn test_write_from_linefeed_stream_success() {
        let data = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Ok(LineFeed::Line("line2".to_string())),
            Ok(LineFeed::END),
        ];
        let mut output = Vec::new();
        let result = write_from_linefeed_stream(data.into_iter(), &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);
        assert_eq!(output, b"line1\nline2\n".to_vec());
    }

    // ========================================================================
    // Tests for collect_bytes_from_send_safe
    // ========================================================================

    #[test]
    fn test_collect_bytes_from_send_safe_text() {
        let body = SendSafeBody::Text("hello world".to_string());
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"hello world".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_bytes() {
        let body = SendSafeBody::Bytes(b"binary data".to_vec());
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"binary data".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_none() {
        let body = SendSafeBody::None;
        let result = collect_bytes_from_send_safe(body);
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_stream() {
        // Use SendableBoxedError which has Send + Sync for the iterator
        let stream_data: Vec<Result<Vec<u8>, SendableBoxedError>> =
            vec![Ok(b"chunk1".to_vec()), Ok(b"chunk2".to_vec())];
        // Cast to BoxedError iterator via Box<dyn Iterator + Send>
        let send_iter: Box<dyn Iterator<Item = Result<Vec<u8>, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::Stream(Some(send_iter));
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"chunk1chunk2".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_stream_none() {
        let body = SendSafeBody::Stream(None);
        let result = collect_bytes_from_send_safe(body);
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_chunked_stream() {
        let stream_data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
            Ok(ChunkedData::Data(b"data1".to_vec(), None)),
            Ok(ChunkedData::Data(b"data2".to_vec(), None)),
            Ok(ChunkedData::DataEnded),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::ChunkedStream(Some(send_iter));
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"data1data2".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_chunked_stream_none() {
        let body = SendSafeBody::ChunkedStream(None);
        let result = collect_bytes_from_send_safe(body);
        assert!(result.is_empty());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_linefeed_stream() {
        let stream_data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Ok(LineFeed::Line("line2".to_string())),
            Ok(LineFeed::END),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::LineFeedStream(Some(send_iter));
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"line1\nline2\n".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_linefeed_stream_none() {
        let body = SendSafeBody::LineFeedStream(None);
        let result = collect_bytes_from_send_safe(body);
        assert!(result.is_empty());
    }

    // ========================================================================
    // Tests for collect_bytes_into
    // ========================================================================

    #[test]
    fn test_collect_bytes_into_text() {
        let body = SendSafeBody::Text("hello".to_string());
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5);
        assert_eq!(output, b"hello".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_bytes() {
        let body = SendSafeBody::Bytes(b"binary".to_vec());
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 6);
        assert_eq!(output, b"binary".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_none() {
        let body = SendSafeBody::None;
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
        assert!(output.is_empty());
    }

    #[test]
    fn test_collect_bytes_into_stream() {
        let stream_data: Vec<Result<Vec<u8>, SendableBoxedError>> =
            vec![Ok(b"chunk1".to_vec()), Ok(b"chunk2".to_vec())];
        let send_iter: Box<dyn Iterator<Item = Result<Vec<u8>, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::Stream(Some(send_iter));
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);
        assert_eq!(output, b"chunk1chunk2".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_chunked_stream() {
        let stream_data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
            Ok(ChunkedData::Data(b"data1".to_vec(), None)),
            Ok(ChunkedData::Data(b"data2".to_vec(), None)),
            Ok(ChunkedData::DataEnded),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::ChunkedStream(Some(send_iter));
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
        assert_eq!(output, b"data1data2".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_linefeed_stream() {
        let stream_data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
            Ok(LineFeed::Line("line1".to_string())),
            Ok(LineFeed::Line("line2".to_string())),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send> = Box::new(
            stream_data
                .into_iter()
                .map(|r| r.map_err(|e| e as BoxedError)),
        );
        let body = SendSafeBody::LineFeedStream(Some(send_iter));
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12);
        assert_eq!(output, b"line1\nline2\n".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_file() {
        // Test writing to an actual file
        let body = SendSafeBody::Bytes(b"file content".to_vec());
        let temp_dir = std::env::temp_dir();
        let test_file = temp_dir.join("test_collect_bytes_into.txt");

        {
            let mut file = std::fs::File::create(&test_file).unwrap();
            let result = collect_bytes_into(body, &mut file);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), 12);
        }

        // Verify file contents
        let contents = std::fs::read(&test_file).unwrap();
        assert_eq!(contents, b"file content".to_vec());

        // Cleanup
        let _ = std::fs::remove_file(&test_file);
    }
}
