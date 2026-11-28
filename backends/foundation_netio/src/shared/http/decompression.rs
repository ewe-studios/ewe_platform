//! Streaming response decoding for `Content-Encoding`.
//!
//! WHY: an HTTP server may compress a response body and label it with
//! `Content-Encoding`. A caller that reads the body as JSON or text must see
//! the decoded bytes, or the codec fails on the gzip magic (`1f 8b`). The HTTP
//! client decodes here, once, so every response path sees plain bytes and a
//! `Content-Encoding` header that no longer describes the body.
//!
//! WHAT: [`response_decoding`] reads the header into a [`ResponseDecoding`]
//! decision. [`decode_body`] wraps a [`SendSafeBody`] in a streaming decoder.
//! The decoder is pulled chunk by chunk, so a large body is never buffered.
//!
//! HOW: The nested body iterator is adapted to `std::io::Read`, wrapped in the
//! existing [`DecompressingReader`], and bounded by
//! [`CompressionConfig::max_decompressed_size`] with [`BoundedDecoder`]. A
//! `Data::Retry` (a non-blocking socket with nothing yet) crosses the decoder
//! as `io::ErrorKind::WouldBlock` and comes back as `Data::Retry`, so the
//! exchange task still parks on readiness.

use std::io::{self, Read};
use std::sync::Mutex;

use bytes::Bytes;
use foundation_core::extensions::result_ext::{BoxedError, SendableBoxedError};
use foundation_core::io::readers::Data;
use foundation_core::valtron::Stream;

use crate::shared::client::body_reader::{SendSafeBodyBytesItem, SendSafeBodyBytesIterator};
use crate::shared::client::compression::{CompressionConfig, ContentEncoding, DecompressingReader};
use crate::shared::http::impls::{Body, SendSafeBody, SimpleHeader, SimpleHeaders};
use crate::shared::http::{HttpClientError, HttpReaderError};

/// What the response reader does with a body's `Content-Encoding`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseDecoding {
    /// No decoding: the header is absent, `identity`, auto-decompression is
    /// off, or this build has no decoder for the encoding.
    None,
    /// Decode the body with this encoding and remove the header.
    Decode(ContentEncoding),
    /// The header names an encoding this client does not know. The body read
    /// fails with a typed error that names it, so compressed bytes never reach
    /// a codec.
    Unsupported(String),
}

/// Read the `Content-Encoding` header into a decoding decision.
///
/// A known encoding whose decoder is not compiled into this build returns
/// [`ResponseDecoding::None`]: the bytes and header pass through unchanged, so
/// a protocol layer with its own decoder (`ConnectRPC`, for example) still works.
#[must_use]
pub fn response_decoding(headers: &SimpleHeaders, config: &CompressionConfig) -> ResponseDecoding {
    if !config.auto_decompress {
        return ResponseDecoding::None;
    }
    let Some(value) = headers
        .get(&SimpleHeader::CONTENT_ENCODING)
        .and_then(|values| values.first())
    else {
        return ResponseDecoding::None;
    };
    // `Content-Encoding` is a comma-separated list of encodings applied in
    // order. A single token is the normal case; a list needs the decoders
    // chained in reverse, which this client does not do.
    let token = value.trim();
    if token.is_empty() || token.eq_ignore_ascii_case("identity") {
        return ResponseDecoding::None;
    }
    match ContentEncoding::from_header(token) {
        ContentEncoding::Identity => ResponseDecoding::None,
        ContentEncoding::Unknown(name) => ResponseDecoding::Unsupported(name),
        encoding if encoding.is_decodable() => ResponseDecoding::Decode(encoding),
        // Known encoding, decoder not compiled in: pass it through.
        _ => ResponseDecoding::None,
    }
}

/// The headers a [`Body`] variant carries.
#[must_use]
pub fn body_headers(body: &Body) -> &SimpleHeaders {
    match body {
        Body::FullBody(headers, _)
        | Body::LimitedBody(_, headers)
        | Body::ChunkedBody(_, headers)
        | Body::LineFeedBody(headers)
        | Body::SseBody(headers) => headers,
    }
}

/// Wrap a de-framed byte reader in the content decoder for `decoding`.
///
/// WHY: a line-feed or server-sent-event body is parsed on the wire bytes, so
/// its decoder must sit between the transfer decoding (chunked, limited, EOF)
/// and the parser. Decoding after the parser would decode parsed lines.
///
/// Returns the reader unchanged for [`ResponseDecoding::None`]. An encoding
/// this build has no decoder for is a typed error, never compressed bytes
/// handed to a parser.
///
/// # Errors
///
/// Returns [`HttpReaderError::UnsupportedContentEncoding`] for an encoding this
/// crate has no decoder for.
pub fn decoding_stream<T: Read + Send + Sync + 'static>(
    source: T,
    decoding: &ResponseDecoding,
    config: &CompressionConfig,
) -> Result<Box<dyn Read + Send + Sync>, SendableBoxedError> {
    match decoding {
        ResponseDecoding::None => Ok(Box::new(source)),
        ResponseDecoding::Unsupported(name) => Err(Box::new(
            HttpReaderError::UnsupportedContentEncoding(name.clone()),
        )),
        ResponseDecoding::Decode(encoding) => {
            let decoder = DecompressingReader::new(source, encoding).map_err(|err| {
                let message = match err {
                    HttpClientError::UnsupportedEncoding(name) => name,
                    other => other.to_string(),
                };
                Box::new(HttpReaderError::UnsupportedContentEncoding(message)) as SendableBoxedError
            })?;
            let bounded = BoundedDecoder::new(decoder, config.max_decompressed_size);
            // The parser stream that wraps this reader requires `Send + Sync`.
            // A decompressor is `Send` but not always `Sync`, so the lock gives
            // it the second bound (the same reason `DecodedBodyIterator` locks).
            Ok(Box::new(SyncReader(Mutex::new(bounded))))
        }
    }
}

/// A `Read` view over a lock, so a `Send` but not `Sync` reader can sit under a
/// `Send + Sync` parser stream.
struct SyncReader<R>(Mutex<R>);

impl<R: Read> Read for SyncReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut guard) => guard.read(buf),
            Err(poisoned) => poisoned.into_inner().read(buf),
        }
    }
}

/// Wrap a response body with a streaming decoder.
///
/// `encoding` comes from [`response_decoding`]. The returned body yields
/// decoded chunks. A decode failure, or a decoded body past
/// `config.max_decompressed_size`, is a typed [`HttpReaderError`], never a
/// silently truncated body.
///
/// A line-feed or server-sent-event body was already decoded by the extractor,
/// which places the decoder before the line or event parser (see
/// [`decoding_stream`]). It passes through here unchanged.
///
/// # Errors
///
/// Returns [`HttpReaderError::UnsupportedContentEncoding`] for an encoding
/// this crate has no decoder for.
pub fn decode_body(
    body: SendSafeBody,
    decoding: ResponseDecoding,
    config: &CompressionConfig,
    batch_size: usize,
) -> Result<SendSafeBody, SendableBoxedError> {
    let encoding = match decoding {
        ResponseDecoding::None => return Ok(body),
        ResponseDecoding::Unsupported(name) => {
            return Err(Box::new(HttpReaderError::UnsupportedContentEncoding(name)));
        }
        ResponseDecoding::Decode(encoding) => encoding,
    };

    match body {
        SendSafeBody::None => Ok(SendSafeBody::None),
        // The extractor decoded these bodies before their parser saw a byte.
        whole @ (SendSafeBody::LineFeedStream(_) | SendSafeBody::SseStream(_)) => Ok(whole),
        body => {
            let source = BodySource::new(body);
            let decoder = DecompressingReader::new(source, &encoding).map_err(|err| {
                let message = match err {
                    HttpClientError::UnsupportedEncoding(name) => name,
                    other => other.to_string(),
                };
                Box::new(HttpReaderError::UnsupportedContentEncoding(message)) as SendableBoxedError
            })?;
            let bounded = BoundedDecoder::new(decoder, config.max_decompressed_size);
            Ok(SendSafeBody::Stream(Some(Box::new(
                DecodedBodyIterator::new(bounded, batch_size),
            ))))
        }
    }
}

/// Adapt a [`SendSafeBody`] chunk iterator to `std::io::Read`.
///
/// `Stream::Wait` (a non-blocking socket with nothing yet) becomes
/// `WouldBlock`, so the decompressor propagates it and the exchange task can
/// park. A stream error becomes an `io::Error` that carries the message.
struct BodySource {
    inner: SendSafeBodyBytesIterator,
    pending: Option<Bytes>,
    finished: bool,
}

impl BodySource {
    fn new(body: SendSafeBody) -> Self {
        Self {
            inner: SendSafeBodyBytesIterator::new(body),
            pending: None,
            finished: false,
        }
    }
}

impl Read for BodySource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if let Some(chunk) = self.pending.take() {
            let n = chunk.len().min(buf.len());
            buf[..n].copy_from_slice(&chunk[..n]);
            if n < chunk.len() {
                self.pending = Some(chunk.slice(n..));
            }
            return Ok(n);
        }
        if self.finished {
            return Ok(0);
        }
        loop {
            match self.inner.next() {
                None => {
                    self.finished = true;
                    return Ok(0);
                }
                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(chunk))) => {
                    if chunk.is_empty() {
                        continue;
                    }
                    let n = chunk.len().min(buf.len());
                    buf[..n].copy_from_slice(&chunk[..n]);
                    if n < chunk.len() {
                        self.pending = Some(chunk.slice(n..));
                    }
                    return Ok(n);
                }
                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(err))) => {
                    self.finished = true;
                    return Err(io::Error::new(io::ErrorKind::InvalidData, err.to_string()));
                }
                Some(Stream::Wait) => {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                // Bookkeeping items (a chunk-end marker, trailers, a skipped
                // line) carry no data: pull the next one.
                Some(_) => {}
            }
        }
    }
}

/// A typed error for a decoded body that grew past its configured cap.
#[derive(Debug)]
struct DecodedBodyTooLarge {
    limit: u64,
}

impl std::fmt::Display for DecodedBodyTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "decoded response body exceeds the {}-byte limit",
            self.limit
        )
    }
}

impl std::error::Error for DecodedBodyTooLarge {}

/// A [`DecompressingReader`] that refuses a decoded body past a byte cap.
struct BoundedDecoder<R: Read> {
    inner: DecompressingReader<R>,
    decoded: u64,
    limit: u64,
}

impl<R: Read> BoundedDecoder<R> {
    fn new(inner: DecompressingReader<R>, limit: usize) -> Self {
        Self {
            inner,
            decoded: 0,
            limit: limit as u64,
        }
    }
}

impl<R: Read> Read for BoundedDecoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n == 0 {
            return Ok(0);
        }
        let decoded = self.decoded.saturating_add(n as u64);
        if self.limit != 0 && decoded > self.limit {
            // Do not report these bytes: the caller sees an error, not a
            // truncated body.
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DecodedBodyTooLarge { limit: self.limit },
            ));
        }
        self.decoded = decoded;
        Ok(n)
    }
}

/// A decoded body chunk iterator.
///
/// The interior lock gives the boxed iterator alias its `Sync` bound without
/// changing `DecompressingReader`.
struct DecodedBodyIterator {
    decoder: Mutex<BoundedDecoder<BodySource>>,
    batch_size: usize,
    finished: bool,
}

impl DecodedBodyIterator {
    fn new(decoder: BoundedDecoder<BodySource>, batch_size: usize) -> Self {
        Self {
            decoder: Mutex::new(decoder),
            batch_size: batch_size.max(1),
            finished: false,
        }
    }
}

fn map_read_error(err: &io::Error) -> BoxedError {
    if let Some(inner) = err.get_ref() {
        if let Some(too_large) = inner.downcast_ref::<DecodedBodyTooLarge>() {
            #[allow(clippy::cast_possible_truncation)]
            return Box::new(HttpReaderError::DecompressedBodyTooLarge(
                too_large.limit as usize,
            ));
        }
    }
    Box::new(HttpReaderError::DecompressionFailed(err.to_string()))
}

impl Iterator for DecodedBodyIterator {
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let mut buf = vec![0u8; self.batch_size];
        let mut decoder = match self.decoder.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match decoder.read(&mut buf) {
            Ok(0) => {
                self.finished = true;
                None
            }
            Ok(n) => {
                buf.truncate(n);
                Some(Ok(Data::Bytes(buf)))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => Some(Ok(Data::Retry)),
            Err(err) => {
                self.finished = true;
                Some(Err(map_read_error(&err)))
            }
        }
    }
}
