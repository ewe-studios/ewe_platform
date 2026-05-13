//! Compression middleware — gzip/brotli response compression based on Accept-Encoding.
//!
//! WHY: Reduces bandwidth for responses, especially text-heavy content.
//! WHAT: Compresses responses when client accepts gzip or brotli encoding.
//!
//! NOTE: This is a simplified implementation. Full response compression would require
//! response wrapping, which is complex. This version validates Accept-Encoding and
//! could be extended to compress response bodies.

use std::sync::Arc;

use foundation_core::wire::simple_http::{
    SimpleHeader, SimpleIncomingRequest,
};

use crate::context::ContextBag;
use crate::middleware::{MiddlewareResult, RequestMiddleware};

/// Compression algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// gzip compression.
    Gzip,
    /// brotli compression.
    Brotli,
    /// deflate compression.
    Deflate,
    /// Identity (no compression).
    Identity,
}

impl CompressionAlgorithm {
    /// Parse from Accept-Encoding header value.
    fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "gzip" => Some(CompressionAlgorithm::Gzip),
            "br" => Some(CompressionAlgorithm::Brotli),
            "deflate" => Some(CompressionAlgorithm::Deflate),
            "identity" => Some(CompressionAlgorithm::Identity),
            _ => None,
        }
    }

    /// Get the content encoding header value.
    fn content_encoding(&self) -> &'static str {
        match self {
            CompressionAlgorithm::Gzip => "gzip",
            CompressionAlgorithm::Brotli => "br",
            CompressionAlgorithm::Deflate => "deflate",
            CompressionAlgorithm::Identity => "identity",
        }
    }
}

/// Compression configuration.
#[derive(Clone)]
pub struct CompressionConfig {
    /// Minimum response size to compress (in bytes).
    pub min_size: usize,
    /// Maximum response size to compress (in bytes).
    pub max_size: usize,
    /// Compression level (1-9 for gzip, 1-11 for brotli).
    pub level: u32,
    /// Compressible MIME types.
    pub mime_types: Vec<String>,
    /// Preferred algorithm priority (first match wins).
    pub preferred_algorithms: Vec<CompressionAlgorithm>,
}

impl CompressionConfig {
    /// Create a new compression config with sensible defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_size: 1024,     // 1KB
            max_size: 10_485_760, // 10MB
            level: 6,
            mime_types: vec![
                "text/plain".to_string(),
                "text/html".to_string(),
                "text/css".to_string(),
                "text/javascript".to_string(),
                "application/javascript".to_string(),
                "application/json".to_string(),
                "application/xml".to_string(),
                "application/rss+xml".to_string(),
                "application/atom+xml".to_string(),
            ],
            preferred_algorithms: vec![
                CompressionAlgorithm::Brotli,
                CompressionAlgorithm::Gzip,
                CompressionAlgorithm::Deflate,
            ],
        }
    }

    /// Set the minimum response size to compress.
    #[must_use]
    pub fn with_min_size(mut self, size: usize) -> Self {
        self.min_size = size;
        self
    }

    /// Set the maximum response size to compress.
    #[must_use]
    pub fn with_max_size(mut self, size: usize) -> Self {
        self.max_size = size;
        self
    }

    /// Set the compression level.
    #[must_use]
    pub fn with_level(mut self, level: u32) -> Self {
        self.level = level;
        self
    }

    /// Add a compressible MIME type.
    #[must_use]
    pub fn with_mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_types.push(mime.into());
        self
    }

    /// Set preferred algorithms in priority order.
    #[must_use]
    pub fn with_algorithms(mut self, algorithms: Vec<CompressionAlgorithm>) -> Self {
        self.preferred_algorithms = algorithms;
        self
    }

    /// Check if a MIME type should be compressed.
    fn should_compress(&self, mime_type: &str) -> bool {
        self.mime_types.iter().any(|m| {
            mime_type.starts_with(m) || m.starts_with(mime_type)
        })
    }
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Compression middleware.
///
/// Validates Accept-Encoding and stores compression preference in request extensions
/// for response handling. Full response compression would require a response wrapper.
pub struct CompressionMiddleware {
    config: CompressionConfig,
}

impl CompressionMiddleware {
    /// Create a new compression middleware with the given config.
    #[must_use]
    pub fn new(config: CompressionConfig) -> Self {
        Self { config }
    }

    /// Create a compression middleware with default config.
    #[must_use]
    pub fn default() -> Self {
        Self::new(CompressionConfig::default())
    }

    /// Parse Accept-Encoding header and select best algorithm.
    fn select_algorithm(&self, accept_encoding: &str) -> Option<CompressionAlgorithm> {
        // Parse qvalues: gzip;q=0.8, br;q=1.0, *;q=0
        let mut candidates: Vec<(CompressionAlgorithm, f32)> = Vec::new();

        for part in accept_encoding.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Split encoding from qvalue
            let mut qvalue = 1.0f32;
            let encoding = if let Some(idx) = part.find(";q=") {
                let enc = part[..idx].trim();
                if let Ok(q) = part[idx + 3..].parse::<f32>() {
                    qvalue = q;
                }
                enc
            } else {
                part
            };

            // Skip identity with q=0 (explicit no compression)
            if encoding.eq_ignore_ascii_case("identity") && qvalue == 0.0 {
                return None;
            }

            // Skip wildcards for now
            if encoding == "*" {
                continue;
            }

            if let Some(alg) = CompressionAlgorithm::from_str(encoding) {
                candidates.push((alg, qvalue));
            }
        }

        // Sort by qvalue descending, then by preferred algorithm order
        candidates.sort_by(|a, b| {
            let q_cmp = b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal);
            if q_cmp != std::cmp::Ordering::Equal {
                return q_cmp;
            }

            // Tie-break by preferred order
            let a_idx = self.config.preferred_algorithms.iter().position(|p| p == &a.0);
            let b_idx = self.config.preferred_algorithms.iter().position(|p| p == &b.0);
            a_idx.cmp(&b_idx)
        });

        candidates.into_iter().map(|(alg, _)| alg).next()
    }

    /// Extract Accept-Encoding header.
    fn extract_accept_encoding(req: &SimpleIncomingRequest) -> Option<String> {
        req.headers
            .iter()
            .find(|(k, _)| format!("{k}").eq_ignore_ascii_case("accept-encoding"))
            .and_then(|(_, v)| v.first().cloned())
    }
}

/// Request extension data for compression preference.
#[derive(Clone, Debug)]
pub struct CompressionPreference {
    /// Selected compression algorithm (None = no compression).
    pub algorithm: Option<CompressionAlgorithm>,
    /// Minimum size threshold.
    pub min_size: usize,
    /// Maximum size threshold.
    pub max_size: usize,
    /// Compression level.
    pub level: u32,
}

impl RequestMiddleware for CompressionMiddleware {
    fn handle(
        &self,
        _ctx: &Arc<ContextBag>,
        req: &mut SimpleIncomingRequest,
    ) -> MiddlewareResult {
        // Parse Accept-Encoding header
        let algorithm = Self::extract_accept_encoding(req)
            .and_then(|enc| self.select_algorithm(&enc));

        // Store compression preference in request extensions
        // This would require modifying SimpleIncomingRequest.extensions
        // For now, we just log the preference
        if let Some(alg) = algorithm {
            tracing::trace!(
                "Compression: client supports {:?} (min_size: {}, max_size: {})",
                alg,
                self.config.min_size,
                self.config.max_size
            );

            // In a full implementation, we'd wrap the response writer
            // to automatically compress based on Content-Type and size.
            // This requires response middleware support.
        }

        // Always continue — compression happens at response time
        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compression_algorithm_from_str() {
        assert_eq!(CompressionAlgorithm::from_str("gzip"), Some(CompressionAlgorithm::Gzip));
        assert_eq!(CompressionAlgorithm::from_str("GZIP"), Some(CompressionAlgorithm::Gzip));
        assert_eq!(CompressionAlgorithm::from_str("br"), Some(CompressionAlgorithm::Brotli));
        assert_eq!(CompressionAlgorithm::from_str("deflate"), Some(CompressionAlgorithm::Deflate));
        assert_eq!(CompressionAlgorithm::from_str("identity"), Some(CompressionAlgorithm::Identity));
        assert_eq!(CompressionAlgorithm::from_str("unknown"), None);
    }

    #[test]
    fn test_compression_config_default() {
        let config = CompressionConfig::default();
        assert_eq!(config.min_size, 1024);
        assert_eq!(config.max_size, 10_485_760);
        assert_eq!(config.level, 6);
        assert!(!config.mime_types.is_empty());
    }

    #[test]
    fn test_compression_select_algorithm() {
        let mw = CompressionMiddleware::default();

        // Prefer brotli
        let alg = mw.select_algorithm("gzip, br");
        assert_eq!(alg, Some(CompressionAlgorithm::Brotli));

        // Only gzip
        let alg = mw.select_algorithm("gzip");
        assert_eq!(alg, Some(CompressionAlgorithm::Gzip));

        // With qvalues
        let alg = mw.select_algorithm("gzip;q=0.5, br;q=1.0");
        assert_eq!(alg, Some(CompressionAlgorithm::Brotli));

        // Identity only
        let alg = mw.select_algorithm("identity");
        assert_eq!(alg, Some(CompressionAlgorithm::Identity));

        // Identity;q=0 means no compression
        let alg = mw.select_algorithm("identity;q=0");
        assert_eq!(alg, None);
    }
}
