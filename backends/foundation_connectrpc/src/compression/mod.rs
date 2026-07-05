//! Compression system + buffer pools (Decision 06).
//!
//! WHY: ConnectRPC compresses per message (each streaming envelope independently)
//! and negotiates the algorithm via `Accept-Encoding` / `Content-Encoding`.
//! Decompression is size-bounded to stop compression bombs. The write path uses
//! per-worker buffer pools to cut allocation pressure without a `Mutex`.
//!
//! WHAT: the [`Compressor`] trait + [`GzipCompressor`] (default; `zstd`/`brotli`
//! behind features), [`CompressionRegistry`], [`negotiate_compression`] /
//! [`NegotiatedCompression`], [`SizeLimits`], and the [`BufferPool`] /
//! [`with_worker_buffer`] machinery ([`pool`]). Domain [`CompressionError`]s map
//! into the RPC error at the boundary (Decision 03).

mod gzip;
pub mod pool;

#[cfg(feature = "brotli")]
mod brotli_impl;
#[cfg(feature = "zstd")]
mod zstd_impl;

pub use gzip::{GzipCompressor, GzipStreamCompressor};
pub use pool::{with_worker_buffer, worker_freeze, BufferPool};

#[cfg(feature = "brotli")]
pub use brotli_impl::BrotliCompressor;
#[cfg(feature = "zstd")]
pub use zstd_impl::ZstdCompressor;

use std::collections::HashMap;
use std::error::Error as StdError;
use std::io::Read;
use std::sync::Arc;

use foundation_errstacks::ErrorTrace;

use crate::error::{Code, ConnectError, ConnectResult};

/// Synchronous per-message compressor.
pub trait Compressor: Send + Sync + 'static {
    /// Algorithm name used in headers (`"gzip"`, `"zstd"`, `"br"`).
    fn name(&self) -> &str;
    /// Compress `input`.
    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, CompressionError>;
    /// Decompress `input`, rejecting output larger than `max_bytes`
    /// (`0` = unlimited — the connect-go-parity default). Guards against
    /// compression bombs.
    fn decompress(&self, input: &[u8], max_bytes: usize) -> Result<Vec<u8>, CompressionError>;
}

/// A compression failure below the RPC surface (Decision 03: decompress →
/// [`Code::InvalidArgument`], oversize → [`Code::ResourceExhausted`], compress →
/// [`Code::Internal`], unsupported negotiation → [`Code::Unimplemented`]).
#[derive(Debug)]
pub enum CompressionError {
    /// Compressing failed.
    Compress {
        /// Algorithm name.
        algorithm: &'static str,
        /// Human-readable cause.
        message: String,
    },
    /// Decompressing failed (malformed input).
    Decompress {
        /// Algorithm name.
        algorithm: &'static str,
        /// Human-readable cause.
        message: String,
    },
    /// Decompressed output exceeded the configured `read_max_bytes` limit.
    TooLarge {
        /// Algorithm name.
        algorithm: &'static str,
        /// The limit that was exceeded.
        limit: usize,
    },
    /// A negotiated encoding is not supported by this registry.
    Unsupported {
        /// The requested encoding.
        requested: String,
        /// The comma-joined supported encodings.
        supported: String,
    },
}

impl CompressionError {
    fn code(&self) -> Code {
        match self {
            CompressionError::Compress { .. } => Code::Internal,
            CompressionError::Decompress { .. } => Code::InvalidArgument,
            CompressionError::TooLarge { .. } => Code::ResourceExhausted,
            CompressionError::Unsupported { .. } => Code::Unimplemented,
        }
    }

    fn to_connect_error(&self) -> ConnectError {
        ConnectError::new(self.code(), self.to_string())
    }
}

impl core::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CompressionError::Compress { algorithm, message } => {
                write!(f, "{algorithm}: compress failed: {message}")
            }
            CompressionError::Decompress { algorithm, message } => {
                write!(f, "{algorithm}: decompress failed: {message}")
            }
            CompressionError::TooLarge { algorithm, limit } => {
                write!(f, "{algorithm}: decompressed size exceeds limit of {limit} bytes")
            }
            CompressionError::Unsupported { requested, supported } => {
                write!(f, "unsupported encoding {requested:?}, supported: [{supported}]")
            }
        }
    }
}

impl StdError for CompressionError {}

impl From<CompressionError> for ConnectError {
    fn from(err: CompressionError) -> Self {
        err.to_connect_error()
    }
}

impl From<CompressionError> for ErrorTrace<ConnectError> {
    fn from(err: CompressionError) -> Self {
        let connect = err.to_connect_error();
        ErrorTrace::new(err).change_context(connect)
    }
}

/// Read `reader` to end into a `Vec`, failing with [`CompressionError::TooLarge`]
/// if the output would exceed `max_bytes` (`0` = unlimited). Shared by the
/// built-in decompressors so the bomb guard is applied identically.
pub(crate) fn read_to_limit<R: Read>(
    mut reader: R,
    max_bytes: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, CompressionError> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = reader.read(&mut chunk).map_err(|e| CompressionError::Decompress {
            algorithm,
            message: e.to_string(),
        })?;
        if n == 0 {
            break;
        }
        if max_bytes != 0 && out.len() + n > max_bytes {
            return Err(CompressionError::TooLarge {
                algorithm,
                limit: max_bytes,
            });
        }
        out.extend_from_slice(&chunk[..n]);
    }
    Ok(out)
}

/// Per-message size limits (Decision 06). Defaults are **unlimited** (connect-go
/// parity, P17/Q7) — a cap is opt-in.
#[derive(Debug, Clone, Copy)]
pub struct SizeLimits {
    /// Maximum decompressed message size to accept (`0` = unlimited).
    pub read_max_bytes: usize,
    /// Maximum message size to send, checked **after** compression (`0` = unlimited).
    pub send_max_bytes: usize,
    /// Minimum message size before compression is applied (`0` = always compress
    /// when compression is negotiated).
    pub compress_min_bytes: usize,
}

impl Default for SizeLimits {
    fn default() -> Self {
        Self {
            read_max_bytes: 0,
            send_max_bytes: 0,
            compress_min_bytes: 0,
        }
    }
}

impl SizeLimits {
    /// Whether a message of `len` bytes should be compressed under these limits
    /// when compression is negotiated (Decision 06 H10).
    #[must_use]
    pub fn should_compress(&self, len: usize) -> bool {
        len >= self.compress_min_bytes
    }

    /// Check an outgoing (already-compressed) payload against `send_max_bytes`
    /// (Decision 06 H10).
    ///
    /// # Errors
    /// Returns a [`ConnectError`]-carrying trace ([`Code::ResourceExhausted`])
    /// when the payload exceeds the send cap.
    pub fn check_send(&self, len: usize) -> ConnectResult<()> {
        if self.send_max_bytes != 0 && len > self.send_max_bytes {
            return Err(ConnectError::resource_exhausted(format!(
                "message of {len} bytes exceeds send_max_bytes of {}",
                self.send_max_bytes
            ))
            .into());
        }
        Ok(())
    }
}

/// Registry of compressors keyed by algorithm name (Decision 06). Built with
/// gzip registered, then frozen behind an `Arc` (RS7).
#[derive(Clone)]
pub struct CompressionRegistry {
    compressors: HashMap<String, Arc<dyn Compressor>>,
    /// Supported names in registration order (for `Accept-Encoding`).
    supported_names: Vec<String>,
    /// Cached comma-joined `Accept-Encoding` value.
    accept_encoding: String,
}

impl CompressionRegistry {
    /// Create a registry with gzip registered.
    #[must_use]
    pub fn new() -> Self {
        let mut registry = Self {
            compressors: HashMap::new(),
            supported_names: Vec::new(),
            accept_encoding: String::new(),
        };
        registry.register(Arc::new(GzipCompressor::default()));
        registry
    }

    /// Create an empty registry (no compressors — identity only).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            compressors: HashMap::new(),
            supported_names: Vec::new(),
            accept_encoding: String::new(),
        }
    }

    /// Register (or replace) a compressor.
    pub fn register(&mut self, compressor: Arc<dyn Compressor>) {
        let name = compressor.name().to_string();
        if !self.compressors.contains_key(&name) {
            self.supported_names.push(name.clone());
        }
        self.compressors.insert(name, compressor);
        self.rebuild_header();
    }

    /// Remove a compressor by name.
    pub fn remove(&mut self, name: &str) {
        if self.compressors.remove(name).is_some() {
            self.supported_names.retain(|n| n != name);
            self.rebuild_header();
        }
    }

    /// Look up a compressor by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Compressor>> {
        self.compressors.get(name)
    }

    /// The `Accept-Encoding` / `Connect-Accept-Encoding` header value.
    #[must_use]
    pub fn accept_encoding_header(&self) -> &str {
        &self.accept_encoding
    }

    /// Whether `name` is supported (`"identity"` is always accepted).
    #[must_use]
    pub fn supports(&self, name: &str) -> bool {
        name == "identity" || self.compressors.contains_key(name)
    }

    fn rebuild_header(&mut self) {
        self.accept_encoding = self.supported_names.join(", ");
    }
}

impl Default for CompressionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// The compressors negotiated for a call (Decision 06). `None` means identity.
#[derive(Clone, Default)]
pub struct NegotiatedCompression {
    /// Decompressor for incoming messages (`None` = identity).
    pub request_decompressor: Option<Arc<dyn Compressor>>,
    /// Compressor for outgoing messages (`None` = identity).
    pub response_compressor: Option<Arc<dyn Compressor>>,
}

/// Negotiate compression from the request's `Content-Encoding` and
/// `Accept-Encoding` (Decision 06 — the single normative definition, mirroring
/// connect-go's `negotiateCompression`).
///
/// # Errors
/// Returns [`Code::Unimplemented`] when the request declares an unsupported
/// non-identity `Content-Encoding`.
pub fn negotiate_compression(
    registry: &CompressionRegistry,
    request_encoding: Option<&str>,
    accept_encoding: Option<&str>,
) -> ConnectResult<NegotiatedCompression> {
    // 1. Validate the request's own encoding (how to DECODE the request body).
    let request_decompressor = match request_encoding {
        Some(enc) if enc != "identity" && !enc.is_empty() => match registry.get(enc) {
            Some(c) => Some(Arc::clone(c)),
            None => {
                return Err(CompressionError::Unsupported {
                    requested: enc.to_string(),
                    supported: registry.accept_encoding_header().to_string(),
                }
                .into());
            }
        },
        _ => None,
    };

    // 2. Pick a response compressor: first supported algorithm the client
    //    accepts; fall back to the request encoding; else identity.
    let response_compressor = accept_encoding
        .and_then(|accept| {
            accept
                .split(',')
                .map(|tok| tok.split(';').next().unwrap_or(tok).trim())
                .filter(|name| !name.is_empty() && *name != "identity")
                .find_map(|name| registry.get(name).map(Arc::clone))
        })
        .or_else(|| request_decompressor.clone());

    Ok(NegotiatedCompression {
        request_decompressor,
        response_compressor,
    })
}
