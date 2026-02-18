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
    NotImplemented,
    InvalidRequestState,
    NotSupported,
    NoRequestToSend,
    FailedExecution,
    FailedToReadBody,
    InvalidReadState,

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
            Self::FailedToReadBody => {
                write!(f, "Failed to read body from reader, please investigate")
            }
            Self::InvalidRequestState => write!(f, "Invalid state found, please investigate"),
            Self::ReaderError(error) => write!(f, "Failed to read http from reader: {error:?}"),
            Self::NotImplemented => write!(f, "Functionality not implemented"),
            Self::NotSupported => write!(f, "Operation not implemented"),
            Self::NoRequestToSend => write!(f, "Operation failed: no request was sent"),
            Self::FailedExecution => write!(f, "Operation failed: request execution failed"),
            Self::DnsError(err) => write!(f, "DNS error: {err}"),
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WHY: Verify DnsError::ResolutionFailed creates correct error message
    /// WHAT: Tests that the error message includes the hostname
    #[test]
    fn test_dns_error_resolution_failed_display() {
        let error = DnsError::ResolutionFailed("example.com".to_string());
        let display = format!("{}", error);
        assert!(display.contains("DNS resolution failed"));
        assert!(display.contains("example.com"));
    }

    /// WHY: Verify DnsError::InvalidHost creates correct error message
    /// WHAT: Tests that invalid hostname errors are clearly communicated
    #[test]
    fn test_dns_error_invalid_host_display() {
        let error = DnsError::InvalidHost("".to_string());
        let display = format!("{}", error);
        assert!(display.contains("Invalid hostname"));
    }

    /// WHY: Verify DnsError::NoAddressesFound creates correct error message
    /// WHAT: Tests that no addresses error includes the hostname
    #[test]
    fn test_dns_error_no_addresses_display() {
        let error = DnsError::NoAddressesFound("localhost".to_string());
        let display = format!("{}", error);
        assert!(display.contains("No addresses found"));
        assert!(display.contains("localhost"));
    }

    /// WHY: Verify DnsError::IoError wraps std::io::Error correctly
    /// WHAT: Tests that I/O errors are properly converted and displayed
    #[test]
    fn test_dns_error_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::TimedOut, "timeout");
        let dns_error = DnsError::from(io_error);
        let display = format!("{}", dns_error);
        assert!(display.contains("I/O error"));
    }

    /// WHY: Verify HttpClientError::DnsError wraps DnsError correctly
    /// WHAT: Tests that DNS errors are properly converted to HTTP client errors
    #[test]
    fn test_http_client_error_from_dns_error() {
        let dns_error = DnsError::ResolutionFailed("test.com".to_string());
        let http_error = HttpClientError::from(dns_error);
        let display = format!("{}", http_error);
        assert!(display.contains("DNS error"));
        assert!(display.contains("test.com"));
    }

    /// WHY: Verify HttpClientError::ConnectionFailed creates correct message
    /// WHAT: Tests that connection failures are clearly described
    #[test]
    fn test_http_client_error_connection_failed() {
        let error = HttpClientError::ConnectionFailed("connection reset".to_string());
        let display = format!("{}", error);
        assert!(display.contains("Connection failed"));
        assert!(display.contains("connection reset"));
    }

    /// WHY: Verify HttpClientError::InvalidUrl creates correct message
    /// WHAT: Tests that invalid URL errors include the URL
    #[test]
    fn test_http_client_error_invalid_url() {
        let error = HttpClientError::InvalidUrl("not a url".to_string());
        let display = format!("{}", error);
        assert!(display.contains("Invalid URL"));
        assert!(display.contains("not a url"));
    }

    /// WHY: Verify error types implement std::error::Error trait
    /// WHAT: Tests that errors can be used with error handling infrastructure
    #[test]
    fn test_errors_implement_std_error() {
        let dns_error: &dyn std::error::Error = &DnsError::ResolutionFailed("test".to_string());
        let http_error: &dyn std::error::Error = &HttpClientError::InvalidUrl("test".to_string());

        // These should compile and execute without issues
        assert!(!dns_error.to_string().is_empty());
        assert!(!http_error.to_string().is_empty());
    }
}
