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

use crate::event_source::shared::ParseResult;
use crate::event_source::Event;
use crate::shared::http::{
    ChunkedData, HttpReaderError, IncomingResponseParts, LineFeed, SendSafeBody,
};
use bytes::Bytes;
use foundation_core::extensions::result_ext::{BoxedError, SendableBoxedError};
use foundation_core::io::readers::{Data, DataBytesIterator};
use foundation_core::valtron::{BoxedSendableDataIterator, BoxedSendableIterator, Stream};
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
                            for chunk_result in DataBytesIterator::new(iter) {
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
                    SendSafeBody::SseStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            let bytes = collect_from_sse_stream(iter);
                            String::from_utf8(bytes).map_err(StringBodyError::InvalidUtf8)
                        } else {
                            Err(StringBodyError::NoBody)
                        }
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

pub fn drain_stream_iterator_from_send_safe(body: SendSafeBody) -> Result<(), BodyReaderError> {
    match body {
        SendSafeBody::Text(_) => Ok(()),
        SendSafeBody::Bytes(_) => Ok(()),
        SendSafeBody::Stream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                for chunk_result in DataBytesIterator::new(iter) {
                    match chunk_result {
                        Ok(_) => continue,
                        Err(e) => {
                            return Err(BodyReaderError::StreamIteratorError(
                                e.to_string().into_boxed_str(),
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
        SendSafeBody::ChunkedStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                for chunk_result in iter {
                    match chunk_result {
                        Ok(_) => continue,
                        Err(e) => {
                            return Err(BodyReaderError::StreamIteratorError(
                                e.to_string().into_boxed_str(),
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
        SendSafeBody::LineFeedStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                for line_result in iter {
                    match line_result {
                        Ok(_) => continue,
                        Err(e) => {
                            return Err(BodyReaderError::StreamIteratorError(
                                e.to_string().into_boxed_str(),
                            ))
                        }
                    }
                }
            }
            Ok(())
        }
        SendSafeBody::SseStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                drain_from_sse_stream(iter).map_err(|e| {
                    BodyReaderError::StreamIteratorError(e.to_string().into_boxed_str())
                })
            } else {
                Err(BodyReaderError::NoBody)
            }
        }
        SendSafeBody::None => Ok(()),
    }
}

pub fn drain_stream_iterator(
    stream: Box<dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send>,
) -> Result<(), BodyReaderError> {
    for part in stream {
        match part {
            Ok(
                IncomingResponseParts::SizedBody(body) | IncomingResponseParts::StreamedBody(body),
            ) => return drain_stream_iterator_from_send_safe(body),
            Ok(
                IncomingResponseParts::Intro(_, _, _)
                | IncomingResponseParts::NoBody
                | IncomingResponseParts::Headers(_)
                | IncomingResponseParts::SKIP,
            ) => continue,
            Err(e) => return Err(BodyReaderError::StreamRead(e)),
        }
    }
    Ok(())
}

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
                            for chunk_result in DataBytesIterator::new(iter) {
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
                    SendSafeBody::SseStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            Ok(collect_from_sse_stream(iter))
                        } else {
                            Err(BodyReaderError::NoBody)
                        }
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
                        for chunk_result in DataBytesIterator::new(iter) {
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
                SendSafeBody::SseStream(mut opt_iter) => {
                    if let Some(iter) = opt_iter.take() {
                        return collect_from_sse_stream(iter);
                    }
                    return Vec::new();
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
/// Used internally by `collect_bytes_from_send_safe` for Stream variant.
/// Handles Data-exposing iterators by wrapping with `DataBytesIterator`.
fn collect_from_stream<I>(iter: I) -> Vec<u8>
where
    I: Iterator<Item = Result<Data, BoxedError>>,
{
    let mut bytes = Vec::new();
    for chunk_result in DataBytesIterator::new(iter) {
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
/// Used internally by `collect_bytes_from_send_safe` for `ChunkedStream` variant.
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
/// Used internally by `collect_bytes_from_send_safe` for `LineFeedStream` variant.
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

fn drain_from_sse_stream<I>(iter: I) -> Result<(), SendableBoxedError>
where
    I: Iterator<Item = Result<crate::event_source::ParseResult, SendableBoxedError>>,
{
    for result in iter {
        match result {
            Ok(_) => continue,
            Err(e) => {
                tracing::warn!("SSE stream error: {e}");
                return Err(e);
            }
        }
    }
    Ok(())
}

/// Process an SSE stream iterator (yields `ParseResult`), collecting event data bytes into a Vec.
/// Used internally by `collect_bytes_from_send_safe` for `SseStream` variant.
fn collect_from_sse_stream<I>(iter: I) -> Vec<u8>
where
    I: Iterator<Item = Result<ParseResult, SendableBoxedError>>,
{
    let mut bytes = Vec::new();
    for result in iter {
        match result {
            Ok(pr) => match pr.event {
                Event::Message { data, .. } => {
                    bytes.extend_from_slice(data.as_bytes());
                    bytes.push(b'\n');
                }
                Event::Comment(_) | Event::Reconnect => continue,
            },
            Err(e) => {
                tracing::warn!("SSE stream error: {e}");
                break;
            }
        }
    }
    bytes
}

/// Strict variant: processes an SSE stream and propagates errors to the caller.
/// Generic over the error type — callers map it to whatever they need.
fn collect_from_sse_stream_strict<I, E>(iter: I) -> Result<Vec<u8>, E>
where
    I: Iterator<Item = Result<crate::event_source::ParseResult, E>>,
{
    let mut bytes = Vec::new();
    for result in iter {
        {
            let pr = result?;
            match pr.event {
                Event::Message { data, .. } => {
                    bytes.extend_from_slice(data.as_bytes());
                    bytes.push(b'\n');
                }
                Event::Comment(_) | Event::Reconnect => continue,
            }
        }
    }
    Ok(bytes)
}

/// Process a stream iterator, writing bytes to a writer.
/// Returns total bytes written or first error encountered.
/// Handles Data-exposing iterators by wrapping with `DataBytesIterator`.
fn write_from_stream<I, W>(
    iter: I,
    writer: &mut W,
) -> Result<u64, Box<dyn std::error::Error + Send + Sync>>
where
    I: Iterator<Item = Result<Data, BoxedError>>,
    W: std::io::Write,
{
    let mut total_bytes: u64 = 0;
    for chunk_result in DataBytesIterator::new(iter) {
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
/// WHAT: Handles all `SendSafeBody` variants (Text, Bytes, Stream, `ChunkedStream`, `LineFeedStream`, None).
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
/// `Vec<u8>` containing the body bytes. Returns empty vec for `NoBody`.
///
/// # Examples
///
/// ```rust,ignore
/// let response = client.get(url).send()?;
/// let bytes = collect_bytes_from_send_safe(response.take_body());
/// ```
#[tracing::instrument]
pub fn collect_bytes_from_send_safe(body: SendSafeBody) -> Vec<u8> {
    match body {
        SendSafeBody::Text(t) => t.into_bytes(),
        SendSafeBody::Bytes(b) => b,
        SendSafeBody::None => Vec::new(),
        SendSafeBody::Stream(mut opt_iter) => {
            tracing::trace!("Pulling bytes from SendSafeBody::Stream");
            opt_iter.take().map_or(Vec::new(), collect_from_stream)
        }
        SendSafeBody::ChunkedStream(mut opt_iter) => {
            tracing::trace!("Pulling bytes from SendSafeBody::ChunkedStream");
            opt_iter
                .take()
                .map_or(Vec::new(), collect_from_chunked_stream)
        }
        SendSafeBody::LineFeedStream(mut opt_iter) => {
            tracing::trace!("Pulling bytes from SendSafeBody::LineFeedStream");
            opt_iter
                .take()
                .map_or(Vec::new(), collect_from_linefeed_stream)
        }
        SendSafeBody::SseStream(mut opt_iter) => {
            tracing::trace!("Pulling bytes from SendSafeBody::SseStream");
            opt_iter.take().map_or(Vec::new(), collect_from_sse_stream)
        }
    }
}

/// Collect response body as String directly from a `SendSafeBody`.
///
/// WHY: When you already have a `SendSafeBody` (e.g., from a collected response),
/// this provides a unified way to extract bytes without repeating match logic.
///
/// WHAT: Handles all `SendSafeBody` variants (Text, Bytes, Stream, `ChunkedStream`, `LineFeedStream`, None).
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
/// `String` containing the body bytes. Returns empty vec for `NoBody`.
///
/// # Examples
///
/// ```rust,ignore
/// let response = client.get(url).send()?;
/// let bytes = collect_bytes_from_send_safe(response.take_body());
/// ```
pub fn collect_strings_from_send_safe(body: SendSafeBody) -> Result<String, StringBodyError> {
    let collected = collect_bytes_from_send_safe(body);
    String::from_utf8(collected).map_err(StringBodyError::InvalidUtf8)
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
        SendSafeBody::SseStream(mut opt_iter) => {
            if let Some(iter) = opt_iter.take() {
                let bytes = collect_from_sse_stream_strict(iter)
                    .map_err(|e| format!("SSE stream error: {e}"))?;
                writer.write_all(&bytes)?;
                total_bytes = bytes.len() as u64;
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
/// use foundation_netio::wire::simple_http::client::body_reader::ResponseParser;
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
                            for chunk_result in DataBytesIterator::new(iter) {
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
                    SendSafeBody::SseStream(mut opt_iter) => {
                        if let Some(iter) = opt_iter.take() {
                            for event_result in iter {
                                match event_result {
                                    Ok(pr) => match pr.event {
                                        Event::Message { data, .. } => {
                                            if !processor(data.as_bytes()) {
                                                return Ok(ProcessStreamResult::StoppedByCallback);
                                            }
                                        }
                                        Event::Comment(_) | Event::Reconnect => continue,
                                    },
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

// Re-export from shared so the import path `client::body_reader::ContentLengthEnforcingIterator`
// continues to work for existing callers
pub use crate::shared::http::ContentLengthEnforcingIterator;

// ============================================================================
// LineFeed Content-Length Enforcement (body_reader-internal)
// ============================================================================
/// WHY: Same as `ContentLengthEnforcingIterator` but for `LineFeed` streams.
/// WHAT: Tracks byte count from `LineFeed::Line` items and validates at EOF.
#[allow(unused)]
pub struct LineFeedContentLengthEnforcer<I> {
    inner: Option<I>,
    expected: usize,
    bytes_read: usize,
}

#[allow(unused)]
impl<I> LineFeedContentLengthEnforcer<I> {
    fn new(inner: I, expected: usize) -> Self {
        Self {
            inner: Some(inner),
            expected,
            bytes_read: 0,
        }
    }
}

impl<I> Iterator for LineFeedContentLengthEnforcer<I>
where
    I: Iterator<Item = Result<LineFeed, BoxedError>>,
{
    type Item = Result<LineFeed, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut inner = self.inner.take()?;

        match inner.next() {
            Some(Ok(LineFeed::Line(line))) => {
                // +1 for the newline that was stripped
                self.bytes_read += line.len() + 1;
                self.inner = Some(inner);
                Some(Ok(LineFeed::Line(line)))
            }
            Some(Ok(LineFeed::SKIP | LineFeed::END)) => {
                self.inner = Some(inner);
                Some(Ok(LineFeed::SKIP))
            }
            Some(Err(e)) => {
                self.inner = Some(inner);
                Some(Err(e))
            }
            None => {
                if self.bytes_read == self.expected {
                    None
                } else {
                    Some(Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!(
                            "body truncated: expected {} bytes per Content-Length, got {}",
                            self.expected, self.bytes_read
                        ),
                    ))))
                }
            }
        }
    }
}

// ============================================================================
// Collect body as Result<Vec<u8>> / Result<String> (propagates enforcement errors)
// ============================================================================

/// WHY: `collect_bytes_from_send_safe` silently discards stream errors.
/// When a `ContentLengthEnforcingIterator` detects a mismatch, the error must
/// propagate to the caller. This method returns `Result` so enforcement errors
/// are surfaced.
///
/// WHAT: Drains all body variants into `Vec<u8>`, returning the first error
/// encountered from the inner iterator.
///
/// HOW: For eager bodies (`Text`/`Bytes`) it returns immediately. For streaming
/// bodies it iterates until `None`, returning any `Err` from the stream.
pub fn try_collect_bytes(body: SendSafeBody) -> Result<Vec<u8>, BoxedError> {
    match body {
        SendSafeBody::Text(t) => Ok(t.into_bytes()),
        SendSafeBody::Bytes(b) => Ok(b),
        SendSafeBody::None => Ok(Vec::new()),
        SendSafeBody::Stream(mut opt_iter) => {
            let Some(iter) = opt_iter.take() else {
                return Ok(Vec::new());
            };
            let mut buf = Vec::new();
            for item in iter {
                match item? {
                    Data::Bytes(bytes) => buf.extend_from_slice(&bytes),
                    Data::Retry => {}
                }
            }
            Ok(buf)
        }
        SendSafeBody::ChunkedStream(mut opt_iter) => {
            let Some(iter) = opt_iter.take() else {
                return Ok(Vec::new());
            };
            let mut buf = Vec::new();
            for item in iter {
                match item? {
                    ChunkedData::Data(bytes, _) => buf.extend_from_slice(&bytes),
                    ChunkedData::Trailers(_) | ChunkedData::DataEnded => {}
                }
            }
            Ok(buf)
        }
        SendSafeBody::LineFeedStream(mut opt_iter) => {
            let Some(iter) = opt_iter.take() else {
                return Ok(Vec::new());
            };
            let mut buf = Vec::new();
            for item in iter {
                match item? {
                    LineFeed::Line(line) => {
                        buf.extend_from_slice(line.as_bytes());
                        buf.push(b'\n');
                    }
                    LineFeed::SKIP | LineFeed::END => {}
                }
            }
            Ok(buf)
        }
        SendSafeBody::SseStream(mut opt_iter) => {
            let Some(iter) = opt_iter.take() else {
                return Ok(Vec::new());
            };
            let mut buf = Vec::new();
            for item in iter {
                match item.map_err(|e| e as BoxedError)?.event {
                    Event::Message { data, .. } => {
                        buf.extend_from_slice(data.as_bytes());
                        buf.push(b'\n');
                    }
                    Event::Comment(_) | Event::Reconnect => {}
                }
            }
            Ok(buf)
        }
    }
}

/// WHY: Same as `try_collect_bytes` but returns a `String`.
///
/// WHAT: Collects body bytes via `try_collect_bytes`, then validates UTF-8.
pub fn try_collect_string(body: SendSafeBody) -> Result<String, BoxedError> {
    let bytes = try_collect_bytes(body)?;
    String::from_utf8(bytes).map_err(|e| Box::new(e) as BoxedError)
}

// ============================================================================
// SendSafeBodyBytesIterator — streaming body chunk iterator
// ============================================================================

/// A chunk of body data or a stream error from [`SendSafeBodyBytesIterator`].
///
/// Used as the `D` in `Stream<D, ()>` — callers match `Next(Bytes)` for data
/// or `Next(Error)` for stream failure.
pub enum SendSafeBodyBytesItem {
    Chunk(Bytes),
    StreamError(BoxedError),
}

/// WHY: `try_collect_bytes` buffers the entire response body into a `Vec<u8>` —
/// fine for small payloads but wasteful for streaming/large responses. Transport
/// pumps need a chunk-at-a-time iterator wrapping any `SendSafeBody` variant.
///
/// WHAT: Wraps a `SendSafeBody` and yields `Stream<SendSafeBodyBytesItem, ()>`:
/// `Next(Bytes)` per data chunk, `Next(Error)` for stream errors, `Ignore` for
/// transient non-data items, `None` when exhausted. All six variants handled.
///
/// HOW: Takes ownership of the `SendSafeBody` — the body IS the state. On each
/// `next()` call, matches the current variant and extracts the next chunk.
pub struct SendSafeBodyBytesIterator(SendSafeBody);

impl SendSafeBodyBytesIterator {
    /// Wrap a `SendSafeBody`. Takes ownership — the body cannot be read again.
    #[must_use]
    pub fn new(body: SendSafeBody) -> Self {
        Self(body)
    }
}

impl Iterator for SendSafeBodyBytesIterator {
    type Item = Stream<SendSafeBodyBytesItem, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = SendSafeBodyBytesItem::Chunk;
        let err_item = SendSafeBodyBytesItem::StreamError;
        match &mut self.0 {
            SendSafeBody::Bytes(v) => {
                let bytes = std::mem::take(v);
                if bytes.is_empty() {
                    None
                } else {
                    Some(Stream::Next(item(Bytes::from(bytes))))
                }
            }
            SendSafeBody::Text(t) => {
                let text = std::mem::take(t);
                if text.is_empty() {
                    None
                } else {
                    Some(Stream::Next(item(Bytes::from(text.into_bytes()))))
                }
            }
            SendSafeBody::None => None,
            SendSafeBody::Stream(ref mut opt) => match opt {
                Some(iter) => match iter.next() {
                    Some(Ok(Data::Bytes(bytes))) => Some(Stream::Next(item(Bytes::from(bytes)))),
                    Some(Ok(Data::Retry)) => Some(Stream::Ignore),
                    Some(Err(e)) => Some(Stream::Next(err_item(e))),
                    None => None,
                },
                None => None,
            },
            SendSafeBody::ChunkedStream(ref mut opt) => match opt {
                Some(iter) => match iter.next() {
                    Some(Ok(data)) => match data {
                        ChunkedData::Data(bytes, _) => Some(Stream::Next(item(Bytes::from(bytes)))),
                        ChunkedData::Trailers(_) | ChunkedData::DataEnded => Some(Stream::Ignore),
                    },
                    Some(Err(e)) => Some(Stream::Next(err_item(e))),
                    None => None,
                },
                None => None,
            },
            SendSafeBody::LineFeedStream(ref mut opt) => match opt {
                Some(iter) => match iter.next() {
                    Some(Ok(data)) => match data {
                        LineFeed::Line(line) => {
                            let mut bytes = line.into_bytes();
                            bytes.push(b'\n');
                            Some(Stream::Next(item(Bytes::from(bytes))))
                        }
                        LineFeed::SKIP | LineFeed::END => Some(Stream::Ignore),
                    },
                    Some(Err(e)) => Some(Stream::Next(err_item(e))),
                    None => None,
                },
                None => None,
            },
            SendSafeBody::SseStream(ref mut opt) => match opt {
                Some(iter) => match iter.next() {
                    Some(Ok(parse_result)) => match parse_result.event {
                        Event::Message { data, .. } => Some(Stream::Next(item(Bytes::from(data)))),
                        Event::Comment(_) | Event::Reconnect => Some(Stream::Ignore),
                    },
                    Some(Err(e)) => Some(Stream::Next(err_item(e))),
                    None => None,
                },
                None => None,
            },
        }
    }
}

// ============================================================================
// Async Body Reading — AsyncSendSafeBody
// ============================================================================

use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::Stream as FuturesStream;

/// Async stream that reads body bytes from a `SendSafeBody`.
///
/// - `Text` / `Bytes` → yields one chunk then ends.
/// - `Stream` / `ChunkedStream` / `LineFeedStream` / `SseStream` → yields each
///   chunk from the inner iterator.
/// - `None` → yields nothing.
///
/// Since the inner iterators are synchronous, each poll returns `Poll::Ready`.
/// The wrapper implements `futures_core::Stream` so it integrates with
/// `.collect().await`, `StreamExt`, etc.
pub struct AsyncSendSafeBody {
    inner: SendSafeBodyState,
}

enum SendSafeBodyState {
    Done,
    Text(Vec<u8>),
    Bytes(Vec<u8>),
    Stream(BoxedSendableDataIterator<BoxedError>),
    ChunkedStream(BoxedSendableIterator<ChunkedData, BoxedError>),
    LineFeedStream(BoxedSendableIterator<LineFeed, BoxedError>),
    SseStream(BoxedSendableIterator<crate::event_source::ParseResult, SendableBoxedError>),
}

impl From<SendSafeBody> for AsyncSendSafeBody {
    fn from(body: SendSafeBody) -> Self {
        let inner = match body {
            SendSafeBody::Text(t) => SendSafeBodyState::Text(t.into_bytes()),
            SendSafeBody::Bytes(b) => SendSafeBodyState::Bytes(b),
            SendSafeBody::None => SendSafeBodyState::Done,
            SendSafeBody::Stream(Some(iter)) => SendSafeBodyState::Stream(iter),
            SendSafeBody::Stream(None) => SendSafeBodyState::Done,
            SendSafeBody::ChunkedStream(Some(iter)) => SendSafeBodyState::ChunkedStream(iter),
            SendSafeBody::ChunkedStream(None) => SendSafeBodyState::Done,
            SendSafeBody::LineFeedStream(Some(iter)) => SendSafeBodyState::LineFeedStream(iter),
            SendSafeBody::LineFeedStream(None) => SendSafeBodyState::Done,
            SendSafeBody::SseStream(Some(iter)) => SendSafeBodyState::SseStream(iter),
            SendSafeBody::SseStream(None) => SendSafeBodyState::Done,
        };
        Self { inner }
    }
}

impl FuturesStream for AsyncSendSafeBody {
    type Item = Result<Vec<u8>, BoxedError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // SAFETY: we never move out of self, and the inner state is not self-referential.
        let this = unsafe { self.get_unchecked_mut() };

        match std::mem::replace(&mut this.inner, SendSafeBodyState::Done) {
            SendSafeBodyState::Done => Poll::Ready(None),
            SendSafeBodyState::Text(bytes) => Poll::Ready(Some(Ok(bytes))),
            SendSafeBodyState::Bytes(bytes) => Poll::Ready(Some(Ok(bytes))),
            SendSafeBodyState::Stream(mut iter) => {
                match iter.next() {
                    Some(Ok(Data::Bytes(b))) => {
                        this.inner = SendSafeBodyState::Stream(iter);
                        Poll::Ready(Some(Ok(b)))
                    }
                    Some(Ok(Data::Retry)) => {
                        this.inner = SendSafeBodyState::Stream(iter);
                        // Retry means "no data yet but stream alive" — poll again immediately.
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    Some(Err(e)) => Poll::Ready(Some(Err(e))),
                    None => Poll::Ready(None),
                }
            }
            SendSafeBodyState::ChunkedStream(mut iter) => match iter.next() {
                Some(Ok(ChunkedData::Data(b, _))) => {
                    this.inner = SendSafeBodyState::ChunkedStream(iter);
                    Poll::Ready(Some(Ok(b)))
                }
                Some(Ok(ChunkedData::Trailers(_))) => {
                    this.inner = SendSafeBodyState::ChunkedStream(iter);
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Some(Ok(ChunkedData::DataEnded)) => Poll::Ready(None),
                Some(Err(e)) => Poll::Ready(Some(Err(e))),
                None => Poll::Ready(None),
            },
            SendSafeBodyState::LineFeedStream(mut iter) => {
                match iter.next() {
                    Some(Ok(LineFeed::Line(line))) => {
                        this.inner = SendSafeBodyState::LineFeedStream(iter);
                        // Include the stripped newline in the output
                        let mut bytes = line.into_bytes();
                        bytes.push(b'\n');
                        Poll::Ready(Some(Ok(bytes)))
                    }
                    Some(Ok(LineFeed::SKIP | LineFeed::END)) => {
                        this.inner = SendSafeBodyState::LineFeedStream(iter);
                        cx.waker().wake_by_ref();
                        Poll::Pending
                    }
                    Some(Err(e)) => Poll::Ready(Some(Err(e))),
                    None => Poll::Ready(None),
                }
            }
            SendSafeBodyState::SseStream(mut iter) => match iter.next() {
                Some(Ok(pr)) => {
                    this.inner = SendSafeBodyState::SseStream(iter);
                    match pr.event {
                        crate::event_source::Event::Message { data, .. } => {
                            let mut bytes = data.into_bytes();
                            bytes.push(b'\n');
                            Poll::Ready(Some(Ok(bytes)))
                        }
                        crate::event_source::Event::Comment(_)
                        | crate::event_source::Event::Reconnect => {
                            cx.waker().wake_by_ref();
                            Poll::Pending
                        }
                    }
                }
                Some(Err(e)) => Poll::Ready(Some(Err(e))),
                None => Poll::Ready(None),
            },
        }
    }
}

/// Collect all body bytes from an `AsyncSendSafeBody` in async context.
pub async fn collect_bytes_async(mut body: AsyncSendSafeBody) -> Result<Vec<u8>, BoxedError> {
    use core::future::poll_fn;
    let mut all = Vec::new();
    loop {
        let chunk = poll_fn(|cx| Pin::new(&mut body).poll_next(cx)).await;
        match chunk {
            Some(Ok(bytes)) => all.extend(bytes),
            Some(Err(e)) => return Err(e),
            None => break,
        }
    }
    Ok(all)
}

/// Collect body as a String in async context.
pub async fn collect_string_async(body: AsyncSendSafeBody) -> Result<String, BoxedError> {
    let bytes = collect_bytes_async(body).await?;
    String::from_utf8(bytes).map_err(|e| Box::new(e) as BoxedError)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use foundation_core::extensions::result_ext::{BoxedError, SendableBoxedError};
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

    /// Helper to create a SendableBoxedError for testing
    fn make_sendable_error(msg: &str) -> SendableBoxedError {
        msg.to_string().into()
    }

    #[test]
    fn test_collect_from_stream_success() {
        let data = vec![
            Ok(Data::Bytes(b"hello".to_vec())),
            Ok(Data::Bytes(b" ".to_vec())),
            Ok(Data::Bytes(b"world".to_vec())),
        ];
        let result = collect_from_stream(data.into_iter());
        assert_eq!(result, b"hello world".to_vec());
    }

    #[test]
    fn test_collect_from_stream_error() {
        let data: Vec<Result<Data, BoxedError>> = vec![
            Ok(Data::Bytes(b"hello".to_vec())),
            Err(make_error("stream error")),
        ];
        let result = collect_from_stream(data.into_iter());
        // Should collect what it got before error
        assert_eq!(result, b"hello".to_vec());
    }

    #[test]
    fn test_collect_from_stream_empty() {
        let data: Vec<Result<Data, BoxedError>> = vec![];
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
        let data = vec![
            Ok(Data::Bytes(b"hello".to_vec())),
            Ok(Data::Bytes(b" world".to_vec())),
        ];
        let mut output = Vec::new();
        let result = write_from_stream(data.into_iter(), &mut output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 11);
        assert_eq!(output, b"hello world".to_vec());
    }

    #[test]
    fn test_write_from_stream_error() {
        let data: Vec<Result<Data, BoxedError>> = vec![
            Ok(Data::Bytes(b"hello".to_vec())),
            Err(make_error("write error")),
        ];
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
        let stream_data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"chunk1".to_vec())),
            Ok(Data::Bytes(b"chunk2".to_vec())),
        ];
        // Cast to BoxedError iterator via Box<dyn Iterator + Send>
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> = Box::new(
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
        let stream_data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"chunk1".to_vec())),
            Ok(Data::Bytes(b"chunk2".to_vec())),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> = Box::new(
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

    // ========================================================================
    // Tests for ContentLengthEnforcingIterator
    // ========================================================================

    #[test]
    fn test_content_length_enforcing_exact_match() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"hello".to_vec())),
            Ok(Data::Bytes(b"world".to_vec())),
        ];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
        // First chunk: "hello" (5 bytes)
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"hello"),
            _ => panic!("expected Bytes"),
        }
        // Second chunk: "world" (5 bytes)
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"world"),
            _ => panic!("expected Bytes"),
        }
        // Inner exhausted, bytes_read == expected → None
        assert!(enforcing.next().is_none());
    }

    #[test]
    fn test_content_length_enforcing_truncated() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![Ok(Data::Bytes(b"short".to_vec()))];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"short"),
            _ => panic!("expected Bytes"),
        }
        // Inner exhausted, bytes_read (5) != expected (10) → error
        let result = enforcing.next();
        assert!(result.is_some());
        let err = result.unwrap().unwrap_err();
        assert_eq!(
            err.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn test_content_length_enforcing_retry_passthrough() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"a".to_vec())),
            Ok(Data::Retry),
            Ok(Data::Bytes(b"b".to_vec())),
        ];
        // "a" (1 byte) + "b" (1 byte) = 2 bytes total; Retry doesn't count
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 2);
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"a"),
            _ => panic!("expected Bytes"),
        }
        assert!(matches!(enforcing.next().unwrap().unwrap(), Data::Retry));
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"b"),
            _ => panic!("expected Bytes"),
        }
        // Inner exhausted, bytes_read == expected (2) → None
        assert!(enforcing.next().is_none());
    }

    #[test]
    fn test_content_length_enforcing_error_passthrough() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"a".to_vec())),
            Err(make_sendable_error("boom")),
        ];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 10);
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"a"),
            _ => panic!("expected Bytes"),
        }
        let result = enforcing.next();
        assert!(result.is_some());
        assert_eq!(result.unwrap().unwrap_err().to_string(), "boom");
    }

    #[test]
    fn test_content_length_enforcing_zero_expected_empty_body() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 0);
        assert!(enforcing.next().is_none());
    }

    #[test]
    fn test_content_length_enforcing_stops_after_exhaustion() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![Ok(Data::Bytes(b"a".to_vec()))];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut enforcing = ContentLengthEnforcingIterator::new(inner, 5);
        match enforcing.next().unwrap().unwrap() {
            Data::Bytes(b) => assert_eq!(&b, b"a"),
            _ => panic!("expected Bytes"),
        }
        // Should return error on inner None (bytes_read != expected)
        assert!(enforcing.next().is_some_and(|r| r.is_err()));
        // Should return None after error was consumed (inner is now None)
        assert!(enforcing.next().is_none());
    }

    // ========================================================================
    // Tests for try_collect_bytes
    // ========================================================================

    #[test]
    fn test_try_collect_bytes_text() {
        let body = SendSafeBody::Text("hello".to_string());
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"hello".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_bytes() {
        let body = SendSafeBody::Bytes(b"\x00\x01\x02".to_vec());
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"\x00\x01\x02".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_none() {
        let body = SendSafeBody::None;
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_try_collect_bytes_stream_success() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"abc".to_vec())),
            Ok(Data::Bytes(b"def".to_vec())),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let body = SendSafeBody::Stream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"abcdef".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_stream_error_propagates() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"ab".to_vec())),
            Err(make_sendable_error("stream failed")),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let body = SendSafeBody::Stream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "stream failed");
    }

    #[test]
    fn test_try_collect_bytes_stream_none() {
        let body = SendSafeBody::Stream(None);
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_try_collect_bytes_chunked_stream_success() {
        let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
            Ok(ChunkedData::Data(b"foo".to_vec(), None)),
            Ok(ChunkedData::Data(b"bar".to_vec(), None)),
            Ok(ChunkedData::DataEnded),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let body = SendSafeBody::ChunkedStream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"foobar".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_chunked_stream_none() {
        let body = SendSafeBody::ChunkedStream(None);
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_try_collect_bytes_linefeed_stream_success() {
        let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
            Ok(LineFeed::Line("alpha".to_string())),
            Ok(LineFeed::Line("beta".to_string())),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let body = SendSafeBody::LineFeedStream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"alpha\nbeta\n".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_linefeed_stream_none() {
        let body = SendSafeBody::LineFeedStream(None);
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_try_collect_bytes_sse_stream_success() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: Some("1".to_string()),
                    event_type: None,
                    data: "event one".to_string(),
                    retry: None,
                },
                Some("1".to_string()),
            )),
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: None,
                    event_type: Some("custom".to_string()),
                    data: "event two".to_string(),
                    retry: None,
                },
                Some("1".to_string()),
            )),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let body = SendSafeBody::SseStream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        // Each message data followed by \n
        assert_eq!(result.unwrap(), b"event one\nevent two\n".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_sse_stream_skips_comments() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: None,
                    event_type: None,
                    data: "real data".to_string(),
                    retry: None,
                },
                None,
            )),
            Ok(ParseResult::new(
                SseEvent::Comment("keepalive".to_string()),
                None,
            )),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let body = SendSafeBody::SseStream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        // Only the message event data should appear
        assert_eq!(result.unwrap(), b"real data\n".to_vec());
    }

    #[test]
    fn test_try_collect_bytes_sse_stream_empty() {
        let body = SendSafeBody::SseStream(None);
        let result = try_collect_bytes(body);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ========================================================================
    // Tests for try_collect_string
    // ========================================================================

    #[test]
    fn test_try_collect_string_valid_utf8() {
        let body = SendSafeBody::Text("hello world".to_string());
        let result = try_collect_string(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "hello world");
    }

    #[test]
    fn test_try_collect_string_invalid_utf8() {
        // Invalid UTF-8: 0xFF is never valid in UTF-8
        let body = SendSafeBody::Bytes(vec![0xFF, 0xFE]);
        let result = try_collect_string(body);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_collect_string_sse_stream() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
            SseEvent::Message {
                id: None,
                event_type: None,
                data: "message".to_string(),
                retry: None,
            },
            None,
        ))];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let body = SendSafeBody::SseStream(Some(send_iter));
        let result = try_collect_string(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "message\n");
    }

    // ========================================================================
    // Tests for collect_strings_from_send_safe
    // ========================================================================

    #[test]
    fn test_collect_strings_from_send_safe_text() {
        let body = SendSafeBody::Text("unicode: \u{2764}".to_string());
        let result = collect_strings_from_send_safe(body);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "unicode: \u{2764}");
    }

    #[test]
    fn test_collect_strings_from_send_safe_invalid_utf8() {
        let body = SendSafeBody::Bytes(vec![0xC0, 0x80]);
        let result = collect_strings_from_send_safe(body);
        assert!(result.is_err());
    }

    // ========================================================================
    // Tests for try_collect_bytes with enforcement integration
    // ========================================================================

    #[test]
    fn test_try_collect_bytes_propagates_content_length_enforcement() {
        // Simulate a LimitedBatchStreamReader that returns fewer bytes than expected
        let data: Vec<Result<Data, SendableBoxedError>> =
            vec![Ok(Data::Bytes(b"partial".to_vec()))];
        let inner: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let enforcing = ContentLengthEnforcingIterator::new(inner, 100);
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(enforcing);
        let body = SendSafeBody::Stream(Some(send_iter));
        let result = try_collect_bytes(body);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        assert!(err.to_string().contains("expected 100 bytes"));
        assert!(err.to_string().contains("got 7"));
    }

    // ========================================================================
    // Tests for SSE stream handling in collection functions
    // ========================================================================

    #[allow(dead_code)]
    fn make_sse_parse_result_vec(
        events: Vec<Result<crate::event_source::ParseResult, SendableBoxedError>>,
    ) -> Vec<Result<crate::event_source::ParseResult, SendableBoxedError>> {
        events
    }

    fn make_sse_stream(
        data: Vec<Result<crate::event_source::ParseResult, SendableBoxedError>>,
    ) -> SendSafeBody {
        let send_iter: Box<
            dyn Iterator<Item = Result<crate::event_source::ParseResult, SendableBoxedError>>
                + Send,
        > = Box::new(data.into_iter());
        SendSafeBody::SseStream(Some(send_iter))
    }

    #[test]
    fn test_collect_bytes_from_send_safe_sse_stream() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let body = make_sse_stream(vec![
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: Some("1".to_string()),
                    event_type: None,
                    data: "event one".to_string(),
                    retry: None,
                },
                Some("1".to_string()),
            )),
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: Some("2".to_string()),
                    event_type: None,
                    data: "event two".to_string(),
                    retry: None,
                },
                Some("2".to_string()),
            )),
        ]);
        let result = collect_bytes_from_send_safe(body);
        // Each event data followed by \n
        assert_eq!(result, b"event one\nevent two\n".to_vec());
    }

    #[test]
    fn test_collect_bytes_from_send_safe_sse_skips_comments() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let body = make_sse_stream(vec![
            Ok(ParseResult::new(
                SseEvent::Message {
                    id: None,
                    event_type: None,
                    data: "real".to_string(),
                    retry: None,
                },
                None,
            )),
            Ok(ParseResult::new(
                SseEvent::Comment("keepalive".to_string()),
                None,
            )),
        ]);
        let result = collect_bytes_from_send_safe(body);
        assert_eq!(result, b"real\n".to_vec());
    }

    #[test]
    fn test_collect_bytes_into_sse_stream() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let body = make_sse_stream(vec![Ok(ParseResult::new(
            SseEvent::Message {
                id: None,
                event_type: None,
                data: "sse data".to_string(),
                retry: None,
            },
            None,
        ))]);
        let mut output = Vec::new();
        let result = collect_bytes_into(body, &mut output);
        assert!(result.is_ok());
        assert_eq!(output, b"sse data\n".to_vec());
    }

    #[test]
    fn test_collect_string_strict_sse_stream() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
            SseEvent::Message {
                id: None,
                event_type: None,
                data: "sse message".to_string(),
                retry: None,
            },
            None,
        ))];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let stream: Box<
            dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send,
        > = Box::new(std::iter::once(Ok(IncomingResponseParts::StreamedBody(
            SendSafeBody::SseStream(Some(send_iter)),
        ))));
        let result = collect_string_strict(stream);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "sse message\n");
    }

    #[test]
    fn test_collect_bytes_strict_sse_stream() {
        use crate::event_source::Event as SseEvent;
        use crate::event_source::ParseResult;

        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![Ok(ParseResult::new(
            SseEvent::Message {
                id: None,
                event_type: None,
                data: "bytes".to_string(),
                retry: None,
            },
            None,
        ))];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let stream: Box<
            dyn Iterator<Item = Result<IncomingResponseParts, HttpReaderError>> + Send,
        > = Box::new(std::iter::once(Ok(IncomingResponseParts::StreamedBody(
            SendSafeBody::SseStream(Some(send_iter)),
        ))));
        let result = collect_bytes_strict(stream);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"bytes\n".to_vec());
    }

    // ========================================================================
    // Tests for SendSafeBodyBytesIterator
    // ========================================================================

    #[test]
    fn test_send_safe_body_bytes_iterator_bytes() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(b"hello".to_vec()));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"hello"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_bytes_empty() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(vec![]));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_text() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Text("world".into()));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"world"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_text_empty() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Text(String::new()));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_none() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::None);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_stream_data_chunks() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"a".to_vec())),
            Ok(Data::Retry),
            Ok(Data::Bytes(b"b".to_vec())),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(Some(send_iter)));

        // Chunk "a"
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"a"
        ));
        // Retry → Ignore
        assert!(matches!(iter.next(), Some(Stream::Ignore)));
        // Chunk "b"
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"b"
        ));
        // Exhausted
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_stream_error() {
        let data: Vec<Result<Data, SendableBoxedError>> = vec![
            Ok(Data::Bytes(b"ok".to_vec())),
            Err(make_sendable_error("boom")),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<Data, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"ok"
        ));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "boom"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_stream_none() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Stream(None));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_chunked_stream() {
        let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
            Ok(ChunkedData::Data(b"x".to_vec(), None)),
            Ok(ChunkedData::Trailers(vec![])),
            Ok(ChunkedData::DataEnded),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"x"
        ));
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // Trailers
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // DataEnded
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_chunked_stream_error() {
        let data: Vec<Result<ChunkedData, SendableBoxedError>> = vec![
            Ok(ChunkedData::Data(b"ok".to_vec(), None)),
            Err(make_sendable_error("fail")),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ChunkedData, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::ChunkedStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"ok"
        ));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_linefeed_stream() {
        let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
            Ok(LineFeed::Line("hello".into())),
            Ok(LineFeed::SKIP),
            Ok(LineFeed::END),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter =
            SendSafeBodyBytesIterator::new(SendSafeBody::LineFeedStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"hello\n"
        ));
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // SKIP
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // END
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_linefeed_stream_error() {
        let data: Vec<Result<LineFeed, SendableBoxedError>> = vec![
            Ok(LineFeed::Line("a".into())),
            Err(make_sendable_error("fail")),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<LineFeed, BoxedError>> + Send> =
            Box::new(data.into_iter().map(|r| r.map_err(|e| e as BoxedError)));
        let mut iter =
            SendSafeBodyBytesIterator::new(SendSafeBody::LineFeedStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"a\n"
        ));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_sse_stream() {
        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
            Ok(ParseResult::new(
                Event::Message {
                    id: None,
                    event_type: None,
                    data: "payload".to_string(),
                    retry: None,
                },
                None,
            )),
            Ok(ParseResult::new(Event::Comment("ignore".to_string()), None)),
            Ok(ParseResult::new(Event::Reconnect, None)),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::SseStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"payload"
        ));
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // Comment
        assert!(matches!(iter.next(), Some(Stream::Ignore))); // Reconnect
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_sse_stream_error() {
        let data: Vec<Result<ParseResult, SendableBoxedError>> = vec![
            Ok(ParseResult::new(
                Event::Message {
                    id: None,
                    event_type: None,
                    data: "msg".to_string(),
                    retry: None,
                },
                None,
            )),
            Err(make_sendable_error("fail")),
        ];
        let send_iter: Box<dyn Iterator<Item = Result<ParseResult, SendableBoxedError>> + Send> =
            Box::new(data.into_iter());
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::SseStream(Some(send_iter)));

        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::Chunk(ref b))) if b.as_ref() == b"msg"
        ));
        assert!(matches!(
            iter.next(),
            Some(Stream::Next(SendSafeBodyBytesItem::StreamError(ref e))) if e.to_string() == "fail"
        ));
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_send_safe_body_bytes_iterator_exhaust_is_idempotent() {
        let mut iter = SendSafeBodyBytesIterator::new(SendSafeBody::Bytes(b"once".to_vec()));
        assert!(iter.next().is_some()); // chunk
        assert!(iter.next().is_none()); // exhausted
        assert!(iter.next().is_none()); // idempotent
        assert!(iter.next().is_none()); // still idempotent
    }
}
