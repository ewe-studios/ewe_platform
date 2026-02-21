use crate::extensions::result_ext::BoxedError;
use crate::wire::simple_http::url::InvalidUri;
use crate::wire::simple_http::HttpReaderError;
use derive_more::From;
use std::io;

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

    /// DNS resolution error.
    #[from]
    DnsError(DnsError),

    /// HttpReader error.
    #[from(ignore)]
    ReaderError(HttpReaderError),

    /// Connection failed.
    #[from(ignore)]
    ConnectionFailed(String),

    /// Connection timeout exceeded.
    #[from(ignore)]
    ConnectionTimeout(String),

    /// TLS handshake failed.
    #[from(ignore)]
    TlsHandshakeFailed(String),

    /// Invalid URL scheme (only HTTP and HTTPS are supported).
    #[from(ignore)]
    InvalidScheme(String),

    /// Invalid URL provided.
    #[from(ignore)]
    InvalidUrl(String),

    /// Invalid Location header encountered while resolving redirects.
    /// Contains the raw Location value or a brief diagnostic.
    InvalidLocation(String),

    /// Too many redirects encountered while following Location headers.
    /// Carries the number of redirects that were attempted.
    TooManyRedirects(u8),

    /// Redirects are disallowed by client configuration (policy).
    /// Contains a short explanation or policy name.
    RedirectDisallowed(String),

    /// I/O error during connection or communication.
    #[from]
    IoError(io::Error),

    /// Generic error with boxed error type.
    #[from(ignore)]
    Other(BoxedError),
}

impl std::error::Error for HttpClientError {}

impl From<InvalidUri> for HttpClientError {
    fn from(err: InvalidUri) -> Self {
        HttpClientError::InvalidUrl(err.to_string())
    }
}

impl core::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidReadState => {
                write!(
                    f,
                    "Invalid read state found from reader, please investigate"
                )
            }
            Self::NoPool => {
                write!(
                    f,
                    "Failed becaue no connection pool was provided, please investigate"
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
            Self::Other(err) => write!(f, "HTTP client error: {err}"),
            Self::DnsError(err) => write!(f, "DNS error: {err}"),
            Self::InvalidLocation(inner) => write!(f, "Invalid location: {inner:}"),
            Self::RedirectDisallowed(inner) => write!(f, "Redirection disallowed: {inner:}"),
            Self::TooManyRedirects(count) => write!(f, "Too many redirects: {count}"),
        }
    }
}
