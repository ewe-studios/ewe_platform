use crate::extensions::result_ext::{BoxedError, SendableBoxedError};
use core::fmt;
use derive_more::From;
use std::{
    string::{FromUtf16Error, FromUtf8Error},
    sync::PoisonError,
    time::Duration,
};

pub type Result<T, E> = std::result::Result<T, E>;

use std::io;

/// Error returned when URI parsing fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidUri {
    message: String,
}

impl InvalidUri {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InvalidUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI: {}", self.message)
    }
}

impl std::error::Error for InvalidUri {}

/// Error returned when building a URI from parts fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidUriParts {
    message: String,
}

impl InvalidUriParts {
    /// Creates a new `InvalidUriParts` error.
    ///
    /// Note: This type is reserved for future URI builder functionality.
    /// Currently unused but kept for API completeness.
    #[allow(dead_code)]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for InvalidUriParts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI parts: {}", self.message)
    }
}

impl std::error::Error for InvalidUriParts {}

/// DNS resolution errors.
///
/// These errors occur during DNS hostname resolution.
#[derive(From, Debug)]
pub enum DnsError {
    /// DNS resolution failed for the given hostname.
    #[from(ignore)]
    ResolutionFailed(String),

    /// Invalid hostname provided.
    #[from(ignore)]
    InvalidHost(String),

    /// No addresses found for the given hostname.
    #[from(ignore)]
    NoAddressesFound(String),

    /// I/O error during DNS resolution.
    #[from(ignore)]
    IoError(String),
}

// Manual From implementation for io::Error
impl From<io::Error> for DnsError {
    fn from(err: io::Error) -> Self {
        DnsError::IoError(err.to_string())
    }
}

// Implement Clone manually to handle IoError as String
impl Clone for DnsError {
    fn clone(&self) -> Self {
        match self {
            Self::ResolutionFailed(s) => Self::ResolutionFailed(s.clone()),
            Self::InvalidHost(s) => Self::InvalidHost(s.clone()),
            Self::NoAddressesFound(s) => Self::NoAddressesFound(s.clone()),
            Self::IoError(s) => Self::IoError(s.clone()),
        }
    }
}

impl std::error::Error for DnsError {}

impl core::fmt::Display for DnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ResolutionFailed(host) => {
                write!(f, "DNS resolution failed for host: {host}")
            }
            Self::InvalidHost(host) => {
                write!(f, "Invalid hostname: {host}")
            }
            Self::NoAddressesFound(host) => {
                write!(f, "No addresses found for host: {host}")
            }
            Self::IoError(err) => {
                write!(f, "I/O error during DNS resolution: {err}")
            }
        }
    }
}

/// HTTP client errors.
///
/// These errors can occur during HTTP client operations.
#[derive(From, Debug)]
pub enum HttpClientError {
    NoPool,
    NotImplemented,
    InvalidRequestState,
    NotSupported,
    NoRequestToSend,
    FailedExecution,
    FailedToReadBody,
    InvalidReadState,
    NoPoolProvided,
    ConnectionError,

    // Client sending requests related errors
    InvalidState,
    Unsupported,
    ReadError,
    WriteFailed,
    Timeout,

    /// Too many redirects encountered while following Location headers.
    /// Carries the number of redirects that were attempted.
    TooManyRedirects,

    /// Map SendableBoxedError to HttpClientError.
    FailedWith(SendableBoxedError),

    /// DNS resolution error.
    DnsError(DnsError),

    /// `HttpReader` error.
    ReaderError(HttpReaderError),

    /// I/O error during connection or communication.
    IoError(io::Error),

    /// Reason for failure.
    #[from(ignore)]
    Reason(String),

    /// TLS handshake failed.
    #[from(ignore)]
    TlsHandshakeFailed(String),

    /// Invalid URL scheme (only HTTP and HTTPS are supported).
    #[from(ignore)]
    InvalidScheme(String),

    /// Invalid URL provided.
    #[from(ignore)]
    InvalidUrl(String),

    /// Connection failed.
    #[from(ignore)]
    ConnectionFailed(String),

    /// Connection timeout exceeded.
    #[from(ignore)]
    ConnectionTimeout(String),

    /// Redirects are disallowed by client configuration (policy).
    /// Contains a short explanation or policy name.
    #[from(ignore)]
    RedirectDisallowed(String),

    /// Invalid Location header encountered while resolving redirects.
    /// Contains the raw Location value or a brief diagnostic.
    #[from(ignore)]
    InvalidLocation(String),

    /// Decompression of response body failed.
    /// Contains error message from decompression library.
    #[from(ignore)]
    DecompressionFailed(String),

    /// Unsupported Content-Encoding encountered.
    /// Contains the encoding name that is not supported.
    #[from(ignore)]
    UnsupportedEncoding(String),

    /// Retry needed for transient failure.
    /// Contains current attempt number, calculated delay, and optional status code.
    #[from(ignore)]
    RetryNeeded {
        attempt: u32,
        delay: Duration,
        status_code: Option<u16>,
    },

    /// Proxy connection failed.
    /// Contains error message describing connection failure.
    #[from(ignore)]
    ProxyConnectionFailed(String),

    /// Proxy authentication failed (407 Proxy Authentication Required).
    /// Contains error message or reason.
    #[from(ignore)]
    ProxyAuthenticationFailed(String),

    /// Proxy tunnel establishment failed.
    /// Contains HTTP status code and error message from proxy.
    #[from(ignore)]
    ProxyTunnelFailed {
        status: u16,
        message: String,
    },

    /// Invalid proxy URL format.
    /// Contains parsing error message.
    #[from(ignore)]
    InvalidProxyUrl(String),

    /// SOCKS5 protocol error.
    /// Contains error message from SOCKS5 handshake.
    #[from(ignore)]
    Socks5Error(String),
}

impl std::error::Error for HttpClientError {}

impl<T> From<PoisonError<T>> for HttpClientError {
    fn from(_: PoisonError<T>) -> Self {
        Self::ReadError
    }
}

impl From<InvalidUri> for HttpClientError {
    fn from(err: InvalidUri) -> Self {
        HttpClientError::InvalidUrl(err.to_string())
    }
}

impl core::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FailedWith(err) => {
                write!(f, "Failed with error: {}", err)
            }
            Self::InvalidState => {
                write!(f, "Invalid state, please investigate")
            }
            Self::Unsupported => {
                write!(f, "Unsupported operation, please investigate")
            }
            Self::ReadError => {
                write!(f, "Read error occurred, please investigate")
            }
            Self::Reason(reason) => {
                write!(f, "Failed due to Reason: {}", reason)
            }
            Self::ConnectionError => {
                write!(f, "Connection error occurred, please investigate")
            }
            Self::WriteFailed => {
                write!(f, "Write failed, please investigate")
            }
            Self::Timeout => {
                write!(f, "Timeout occurred, please investigate")
            }
            Self::InvalidReadState => {
                write!(
                    f,
                    "Invalid read state found from reader, please investigate"
                )
            }
            Self::NoPool => {
                write!(
                    f,
                    "Failed because no connection pool was provided, please investigate"
                )
            }
            Self::FailedToReadBody => {
                write!(f, "Failed to read body from reader, please investigate")
            }
            Self::NoPoolProvided => write!(
                f,
                "Expected connection pool to be provided, please investigate"
            ),
            Self::InvalidRequestState => write!(f, "Invalid state found, please investigate"),
            Self::ReaderError(error) => write!(f, "Failed to read http from reader: {error:?}"),
            Self::NotImplemented => write!(f, "Functionality not implemented"),
            Self::NotSupported => write!(f, "Operation not implemented"),
            Self::NoRequestToSend => write!(f, "Operation failed: no request was sent"),
            Self::FailedExecution => write!(f, "Operation failed: request execution failed"),
            Self::ConnectionFailed(msg) => write!(f, "Connection failed: {msg}"),
            Self::ConnectionTimeout(msg) => write!(f, "Connection timeout: {msg}"),
            Self::TlsHandshakeFailed(msg) => write!(f, "TLS handshake failed: {msg}"),
            Self::InvalidScheme(scheme) => write!(
                f,
                "Invalid URL scheme: {scheme} (only HTTP and HTTPS are supported)"
            ),
            Self::InvalidUrl(url) => write!(f, "Invalid URL: {url}"),
            Self::IoError(err) => write!(f, "I/O error: {err}"),
            Self::DnsError(err) => write!(f, "DNS error: {err}"),
            Self::InvalidLocation(inner) => write!(f, "Invalid location: {inner:}"),
            Self::RedirectDisallowed(inner) => write!(f, "Redirection disallowed: {inner:}"),
            Self::TooManyRedirects => write!(f, "Too many redirects"),
            Self::DecompressionFailed(msg) => write!(f, "Decompression failed: {msg}"),
            Self::UnsupportedEncoding(encoding) => {
                write!(f, "Unsupported content encoding: {encoding}")
            }
            Self::RetryNeeded {
                attempt,
                delay,
                status_code,
            } => {
                if let Some(code) = status_code {
                    write!(
                        f,
                        "Retry needed: attempt {}, delay {}ms, status code {}",
                        attempt,
                        delay.as_millis(),
                        code
                    )
                } else {
                    write!(
                        f,
                        "Retry needed: attempt {}, delay {}ms",
                        attempt,
                        delay.as_millis()
                    )
                }
            }
            Self::ProxyConnectionFailed(msg) => write!(f, "Proxy connection failed: {msg}"),
            Self::ProxyAuthenticationFailed(msg) => {
                write!(f, "Proxy authentication failed: {msg}")
            }
            Self::ProxyTunnelFailed { status, message } => {
                write!(f, "Proxy tunnel failed with status {status}: {message}")
            }
            Self::InvalidProxyUrl(msg) => write!(f, "Invalid proxy URL: {msg}"),
            Self::Socks5Error(msg) => write!(f, "SOCKS5 error: {msg}"),
        }
    }
}

#[derive(From, Debug)]
pub enum HttpReaderError {
    #[from(ignore)]
    InvalidLine(String),

    #[from(ignore)]
    UnknownLine(String),

    #[from(ignore)]
    BodyBuildFailed(SendableBoxedError),

    #[from(ignore)]
    ProtoBuildFailed(SendableBoxedError),

    #[from(ignore)]
    LineReadFailed(SendableBoxedError),

    #[from(ignore)]
    InvalidContentSizeValue(Box<std::num::ParseIntError>),

    ZeroBodySizeNotAllowed,
    ExpectedSizedBodyViaContentLength,
    GuardedResourceAccess,
    SeeTrailerBeforeLastChunk,
    TrailerShouldNotOccurHere,
    OnlyTrailersAreAllowedHere,
    InvalidTailerWithNoValue,
    InvalidChunkSize,
    ReadFailed,
    InvalidHeaderKey,
    InvalidHeaderValueStarter,
    InvalidHeaderValueEnder,
    InvalidHeaderValue,
    InvalidTransferEncodingValue,
    HeaderValueStartingWithCR,
    HeaderValueStartingWithLF,
    HeaderValueContainsEncodedCRLF,
    HeaderKeyTooLong,
    HeaderValueTooLong,
    HeaderValuesHasTooManyItems,
    HeaderKeyContainsEncodedCRLF,
    HeaderKeyContainsNotAllowedChars,
    #[from(ignore)]
    HeaderKeyGreaterThanLimit(usize),
    #[from(ignore)]
    HeaderValueGreaterThanLimit(usize),
    BodyContentSizeIsGreaterThanLimit(usize),
    InvalidHeaderLine,

    #[from(ignore)]
    LimitReached(usize),
    BothTransferEncodingAndContentLengthNotAllowed,
    UnknownTransferEncodingHeaderValue,
    ChunkedEncodingMustBeLast,
    UnsupportedTransferEncodingType,

    // Hardening error types
    #[from(ignore)]
    UriTooLong(usize),
    #[from(ignore)]
    TotalHeaderSizeTooLarge(usize),
    #[from(ignore)]
    ChunkSizeTooLarge(usize),
    #[from(ignore)]
    ReadTimeout(Duration),
}

impl std::error::Error for HttpReaderError {}

impl core::fmt::Display for HttpReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<ChunkStateError> for HttpReaderError {
    fn from(err: ChunkStateError) -> Self {
        match err {
            ChunkStateError::ChunkSizeTooLarge(size) => HttpReaderError::ChunkSizeTooLarge(size),
            ChunkStateError::ParseFailed => HttpReaderError::InvalidChunkSize,
            ChunkStateError::ReadErrors => HttpReaderError::ReadFailed,
            ChunkStateError::InvalidByte(_) => HttpReaderError::InvalidChunkSize,
            ChunkStateError::InvalidOctetSizeByte(_) => HttpReaderError::InvalidChunkSize,
            ChunkStateError::ChunkSizeNotFound => HttpReaderError::InvalidChunkSize,
            ChunkStateError::InvalidChunkEnding => HttpReaderError::InvalidChunkSize,
            ChunkStateError::InvalidChunkEndingExpectedCRLF => HttpReaderError::InvalidChunkSize,
            ChunkStateError::ExtensionWithNoValue => HttpReaderError::InvalidChunkSize,
            ChunkStateError::InvalidOctetBytes(_) => HttpReaderError::InvalidChunkSize,
        }
    }
}

#[derive(From, Debug)]
pub enum StringHandlingError {
    Unknown,
    Failed,
}

impl std::error::Error for StringHandlingError {}

impl core::fmt::Display for StringHandlingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum RenderHttpError {
    #[from(ignore)]
    UTF8Error(FromUtf8Error),
    #[from(ignore)]
    UTF16Error(FromUtf16Error),

    #[from(ignore)]
    IOFailed(std::io::Error),

    #[from(ignore)]
    EncodingError(SendableBoxedError),

    InvalidHttpType,
}

impl std::error::Error for RenderHttpError {}

impl core::fmt::Display for RenderHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum Http11RenderError {
    #[from(ignore)]
    UTF8Encoding(FromUtf8Error),

    #[from(ignore)]
    UTF16Encoding(FromUtf16Error),

    #[from(ignore)]
    Failed(BoxedError),

    #[from(ignore)]
    IOFailed(std::io::Error),

    HeadersRequired,
    InvalidSituationUsedIterator,
    InvalidState(String),
}

impl From<BoxedError> for Http11RenderError {
    fn from(value: BoxedError) -> Self {
        Self::Failed(value)
    }
}

impl From<std::io::Error> for Http11RenderError {
    fn from(value: std::io::Error) -> Self {
        Self::IOFailed(value)
    }
}

impl From<FromUtf8Error> for Http11RenderError {
    fn from(value: FromUtf8Error) -> Self {
        Self::UTF8Encoding(value)
    }
}

impl From<FromUtf16Error> for Http11RenderError {
    fn from(value: FromUtf16Error) -> Self {
        Self::UTF16Encoding(value)
    }
}

impl std::error::Error for Http11RenderError {}

impl core::fmt::Display for Http11RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type SimpleHttpResult<T> = std::result::Result<T, SimpleHttpError>;

#[derive(From, Debug)]
pub enum SimpleHttpError {
    NoRouteProvided,
    NoBodyProvided,
}

impl std::error::Error for SimpleHttpError {}

impl core::fmt::Display for SimpleHttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum ChunkStateError {
    ParseFailed,
    ReadErrors,

    #[from(ignore)]
    InvalidByte(u8),

    #[from(ignore)]
    InvalidOctetSizeByte(u8),

    ChunkSizeNotFound,
    InvalidChunkEnding,
    InvalidChunkEndingExpectedCRLF,
    ExtensionWithNoValue,
    InvalidOctetBytes(FromUtf8Error),

    #[from(ignore)]
    ChunkSizeTooLarge(usize),
}

impl<T> From<PoisonError<T>> for ChunkStateError {
    fn from(_: PoisonError<T>) -> Self {
        Self::ReadErrors
    }
}

impl From<std::io::Error> for ChunkStateError {
    fn from(_: std::io::Error) -> Self {
        Self::ReadErrors
    }
}

impl std::error::Error for ChunkStateError {}

impl core::fmt::Display for ChunkStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum LineFeedError {
    ParseFailed,
    ReadErrors,

    #[from(ignore)]
    InvalidByte(u8),

    #[from(ignore)]
    InvalidUTF(FromUtf8Error),
}

impl<T> From<PoisonError<T>> for LineFeedError {
    fn from(_: PoisonError<T>) -> Self {
        Self::ReadErrors
    }
}

impl From<std::io::Error> for LineFeedError {
    fn from(_: std::io::Error) -> Self {
        Self::ReadErrors
    }
}

impl std::error::Error for LineFeedError {}

impl core::fmt::Display for LineFeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(From, Debug)]
pub enum WireErrors {
    LineFeeds(LineFeedError),
    ChunkState(ChunkStateError),
    SimpleHttp(SimpleHttpError),
    RenderError(RenderHttpError),
    Http11Render(Http11RenderError),
    HttpReaderError(HttpReaderError),
    HttpClientError(HttpClientError),
}
