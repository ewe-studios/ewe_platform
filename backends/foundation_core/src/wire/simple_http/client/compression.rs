//! HTTP Compression and Decompression Support
//!
//! WHY: HTTP clients should automatically handle compressed responses (gzip, deflate, brotli)
//! to reduce bandwidth and improve performance. Standard HTTP content negotiation via
//! Accept-Encoding/Content-Encoding headers.
//!
//! WHAT: Provides streaming decompression of HTTP response bodies based on Content-Encoding
//! header. Includes `ContentEncoding` enum for parsing, `DecompressingReader` for streaming
//! decompression, and `CompressionConfig` for client-level configuration.
//!
//! HOW: Implements `std::io::Read` trait wrapping compression libraries (flate2, brotli).
//! Feature-gated to allow optional compression support. No buffering - pure streaming.

use std::io::{self, Read};

use crate::wire::simple_http::HttpClientError;

#[cfg(feature = "gzip")]
use flate2::read::GzDecoder;

#[cfg(any(feature = "gzip", feature = "deflate"))]
use flate2::read::DeflateDecoder;

#[cfg(feature = "brotli")]
use brotli::Decompressor as BrotliDecoder;

/// Content encoding types supported by HTTP compression.
///
/// WHY: HTTP Content-Encoding header indicates how response body is compressed.
/// Client must parse this to apply correct decompression algorithm.
///
/// WHAT: Enum representing standard HTTP content encodings: gzip, deflate, brotli.
/// Identity means no compression. Unknown captures unsupported encodings.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::ContentEncoding;
///
/// let encoding = ContentEncoding::from_header("gzip");
/// assert!(matches!(encoding, ContentEncoding::Gzip));
///
/// let encoding = ContentEncoding::from_header("br");
/// assert!(matches!(encoding, ContentEncoding::Brotli));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEncoding {
    /// No compression (default)
    Identity,
    /// Gzip compression (RFC 1952)
    Gzip,
    /// Deflate compression (RFC 1951)
    Deflate,
    /// Brotli compression (RFC 7932)
    Brotli,
    /// Unsupported or unknown encoding
    Unknown(String),
}

impl ContentEncoding {
    /// Parses Content-Encoding header value into enum variant.
    ///
    /// WHY: Content-Encoding header is case-insensitive per HTTP spec (RFC 7230).
    /// Must normalize to standard encoding names.
    ///
    /// WHAT: Case-insensitive parsing of standard encoding names:
    /// - "gzip" -> Gzip
    /// - "deflate" -> Deflate
    /// - "br" -> Brotli
    /// - "identity" -> Identity
    /// - anything else -> Unknown(value)
    ///
    /// # Arguments
    ///
    /// * `value` - Content-Encoding header value (e.g., "gzip", "br")
    ///
    /// # Returns
    ///
    /// `ContentEncoding` variant matching the header value.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::ContentEncoding;
    ///
    /// assert_eq!(ContentEncoding::from_header("gzip"), ContentEncoding::Gzip);
    /// assert_eq!(ContentEncoding::from_header("GZIP"), ContentEncoding::Gzip);
    /// assert_eq!(ContentEncoding::from_header("br"), ContentEncoding::Brotli);
    /// assert_eq!(ContentEncoding::from_header("deflate"), ContentEncoding::Deflate);
    /// assert_eq!(ContentEncoding::from_header("identity"), ContentEncoding::Identity);
    ///
    /// match ContentEncoding::from_header("unknown") {
    ///     ContentEncoding::Unknown(s) => assert_eq!(s, "unknown"),
    ///     _ => panic!("Expected Unknown variant"),
    /// }
    /// ```
    #[must_use]
    pub fn from_header(value: &str) -> Self {
        match value.to_lowercase().as_str() {
            "gzip" => Self::Gzip,
            "deflate" => Self::Deflate,
            "br" => Self::Brotli,
            "identity" => Self::Identity,
            other => Self::Unknown(other.to_string()),
        }
    }
}

/// Compression configuration for HTTP client.
///
/// WHY: Client needs configurable compression behavior: which encodings to accept,
/// whether to auto-add Accept-Encoding header, and whether to auto-decompress responses.
///
/// WHAT: Configuration struct controlling compression/decompression behavior.
/// Includes supported encodings in preference order.
///
/// HOW: Used by client to decide when to add Accept-Encoding header and whether
/// to wrap response body readers with decompression.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::{CompressionConfig, ContentEncoding};
///
/// // Default config (compression enabled)
/// let config = CompressionConfig::default();
/// assert!(config.add_accept_encoding);
/// assert!(config.auto_decompress);
///
/// // Disabled compression
/// let config = CompressionConfig::disabled();
/// assert!(!config.add_accept_encoding);
/// assert!(!config.auto_decompress);
/// ```
#[derive(Clone, Debug)]
pub struct CompressionConfig {
    /// Enable automatic Accept-Encoding header addition
    pub add_accept_encoding: bool,

    /// Enable automatic response decompression
    pub auto_decompress: bool,

    /// Supported encodings in preference order
    pub supported_encodings: Vec<ContentEncoding>,
}

impl CompressionConfig {
    /// Creates a new compression config with specified settings.
    ///
    /// # Arguments
    ///
    /// * `add_accept_encoding` - Whether to add Accept-Encoding header automatically
    /// * `auto_decompress` - Whether to decompress responses automatically
    /// * `supported_encodings` - List of encodings to support, in preference order
    ///
    /// # Returns
    ///
    /// A new `CompressionConfig` with the specified settings.
    #[must_use]
    pub fn new(
        add_accept_encoding: bool,
        auto_decompress: bool,
        supported_encodings: Vec<ContentEncoding>,
    ) -> Self {
        Self {
            add_accept_encoding,
            auto_decompress,
            supported_encodings,
        }
    }

    /// Creates a config with compression disabled.
    ///
    /// WHY: Some use cases need to disable compression entirely (e.g., debugging,
    /// already-compressed content, bandwidth not a concern).
    ///
    /// # Returns
    ///
    /// A `CompressionConfig` with all compression features disabled.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            add_accept_encoding: false,
            auto_decompress: false,
            supported_encodings: vec![],
        }
    }

    /// Generates Accept-Encoding header value from supported encodings.
    ///
    /// WHY: Accept-Encoding header must be formatted as comma-separated list
    /// of encoding names (per RFC 7231 section 5.3.4).
    ///
    /// WHAT: Converts `supported_encodings` into header value string.
    /// Only includes concrete encodings (Gzip, Deflate, Brotli), not Identity or Unknown.
    ///
    /// # Returns
    ///
    /// Accept-Encoding header value (e.g., "br, gzip, deflate")
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::CompressionConfig;
    ///
    /// let config = CompressionConfig::default();
    /// let header_value = config.accept_encoding_value();
    /// assert!(header_value.contains("gzip"));
    /// assert!(header_value.contains("deflate"));
    /// ```
    #[must_use]
    pub fn accept_encoding_value(&self) -> String {
        self.supported_encodings
            .iter()
            .filter_map(|e| match e {
                ContentEncoding::Gzip => Some("gzip"),
                ContentEncoding::Deflate => Some("deflate"),
                ContentEncoding::Brotli => Some("br"),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl Default for CompressionConfig {
    /// Creates default compression config (compression enabled).
    ///
    /// WHY: Most HTTP clients benefit from compression by default.
    /// Brotli first (best compression), then gzip, then deflate.
    ///
    /// # Returns
    ///
    /// Default config with compression enabled and all encodings supported.
    fn default() -> Self {
        Self {
            add_accept_encoding: true,
            auto_decompress: true,
            supported_encodings: vec![
                ContentEncoding::Brotli,
                ContentEncoding::Gzip,
                ContentEncoding::Deflate,
            ],
        }
    }
}

/// Internal decompressor variants for streaming decompression.
///
/// WHY: Need to wrap different decompression libraries (flate2, brotli) with unified interface.
/// Each library has different APIs, this enum provides abstraction.
///
/// WHAT: Enum holding actual decompressor implementations. Identity is pass-through (no decompression).
/// Gzip and Deflate use flate2, Brotli uses brotli crate.
///
/// HOW: Pattern matches in Read implementation delegate to appropriate decompressor.
enum DecompressorKind<R: Read> {
    /// No decompression (pass-through)
    Identity(R),

    /// Gzip decompression via flate2
    #[cfg(feature = "gzip")]
    Gzip(Box<GzDecoder<R>>),

    /// Deflate decompression via flate2
    #[cfg(any(feature = "gzip", feature = "deflate"))]
    Deflate(Box<DeflateDecoder<R>>),

    /// Brotli decompression via brotli crate
    #[cfg(feature = "brotli")]
    Brotli(Box<BrotliDecoder<R>>),
}

/// Streaming decompressor for HTTP response bodies.
///
/// WHY: HTTP response bodies can be compressed. Client must decompress transparently
/// while maintaining streaming behavior (no buffering entire response).
///
/// WHAT: Wrapper implementing Read trait that decompresses data on-the-fly.
/// Supports gzip, deflate, brotli via feature gates. Identity is pass-through.
///
/// HOW: Wraps underlying reader with appropriate decompressor based on Content-Encoding.
/// Read calls are forwarded to decompressor, which pulls compressed data from inner reader.
///
/// # Type Parameters
///
/// * `R` - Inner reader type (must implement Read)
///
/// # Examples
///
/// ```ignore
/// use foundation_core::wire::simple_http::client::{DecompressingReader, ContentEncoding};
/// use std::io::Read;
///
/// // Gzip decompression
/// let compressed_data = /* ... gzip compressed bytes ... */;
/// let mut reader = DecompressingReader::new(
///     std::io::Cursor::new(compressed_data),
///     ContentEncoding::Gzip
/// )?;
///
/// let mut decompressed = String::new();
/// reader.read_to_string(&mut decompressed)?;
/// ```
pub struct DecompressingReader<R: Read> {
    inner: DecompressorKind<R>,
}

impl<R: Read> DecompressingReader<R> {
    /// Creates a new decompressing reader for the given encoding.
    ///
    /// WHY: Response Content-Encoding header determines which decompressor to use.
    /// Must construct appropriate decompressor based on encoding type.
    ///
    /// WHAT: Factory method creating `DecompressingReader` with correct internal decompressor.
    /// Identity = pass-through, Gzip/Deflate/Brotli = feature-gated decompressors.
    ///
    /// # Arguments
    ///
    /// * `inner` - The inner reader providing compressed data
    /// * `encoding` - Content-Encoding indicating compression algorithm
    ///
    /// # Returns
    ///
    /// A new `DecompressingReader` wrapping the inner reader, or error if encoding unsupported.
    ///
    /// # Errors
    ///
    /// * `HttpClientError::UnsupportedEncoding` - If encoding not supported by enabled features
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::client::{DecompressingReader, ContentEncoding};
    /// use std::io::Cursor;
    ///
    /// let data = b"Hello, World!";
    /// let reader = DecompressingReader::new(
    ///     Cursor::new(data.as_ref()),
    ///     &ContentEncoding::Identity
    /// ).unwrap();
    /// ```
    pub fn new(inner: R, encoding: &ContentEncoding) -> Result<Self, HttpClientError> {
        let decompressor = match encoding {
            ContentEncoding::Identity => DecompressorKind::Identity(inner),

            #[cfg(feature = "gzip")]
            ContentEncoding::Gzip => DecompressorKind::Gzip(Box::new(GzDecoder::new(inner))),

            #[cfg(any(feature = "gzip", feature = "deflate"))]
            ContentEncoding::Deflate => {
                DecompressorKind::Deflate(Box::new(DeflateDecoder::new(inner)))
            }

            #[cfg(feature = "brotli")]
            ContentEncoding::Brotli => {
                DecompressorKind::Brotli(Box::new(BrotliDecoder::new(inner, 4096)))
            }

            ContentEncoding::Unknown(ref enc) => {
                return Err(HttpClientError::UnsupportedEncoding(enc.clone()));
            }

            #[cfg(not(feature = "gzip"))]
            ContentEncoding::Gzip => {
                return Err(HttpClientError::UnsupportedEncoding(
                    "gzip (feature not enabled)".to_string(),
                ));
            }

            #[cfg(not(any(feature = "gzip", feature = "deflate")))]
            ContentEncoding::Deflate => {
                return Err(HttpClientError::UnsupportedEncoding(
                    "deflate (feature not enabled)".to_string(),
                ));
            }

            #[cfg(not(feature = "brotli"))]
            ContentEncoding::Brotli => {
                return Err(HttpClientError::UnsupportedEncoding(
                    "brotli (feature not enabled)".to_string(),
                ));
            }
        };

        Ok(Self {
            inner: decompressor,
        })
    }
}

impl<R: Read> Read for DecompressingReader<R> {
    /// Reads decompressed data into buffer.
    ///
    /// WHY: Read trait implementation allows `DecompressingReader` to be used anywhere
    /// a Read is expected. Maintains streaming behavior.
    ///
    /// WHAT: Delegates to appropriate decompressor's `read()` method based on encoding.
    /// Decompressor pulls compressed data from inner reader, decompresses, writes to buf.
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to write decompressed data into
    ///
    /// # Returns
    ///
    /// Number of bytes read into buffer, or I/O error.
    ///
    /// # Errors
    ///
    /// * I/O errors from inner reader
    /// * Decompression errors (invalid compressed data, corruption)
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            DecompressorKind::Identity(r) => r.read(buf),

            #[cfg(feature = "gzip")]
            DecompressorKind::Gzip(d) => d.read(buf),

            #[cfg(any(feature = "gzip", feature = "deflate"))]
            DecompressorKind::Deflate(d) => d.read(buf),

            #[cfg(feature = "brotli")]
            DecompressorKind::Brotli(d) => d.read(buf),
        }
    }
}
