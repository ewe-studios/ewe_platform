//! Compression middleware tests (moved from `src/shared/middleware/compression.rs`).

use foundation_http::shared::middleware::{
    CompressionAlgorithm, CompressionConfig, CompressionMiddleware,
};

#[test]
fn test_compression_algorithm_from_str() {
    assert_eq!(
        CompressionAlgorithm::from_str("gzip"),
        Some(CompressionAlgorithm::Gzip)
    );
    assert_eq!(
        CompressionAlgorithm::from_str("GZIP"),
        Some(CompressionAlgorithm::Gzip)
    );
    assert_eq!(
        CompressionAlgorithm::from_str("br"),
        Some(CompressionAlgorithm::Brotli)
    );
    assert_eq!(
        CompressionAlgorithm::from_str("deflate"),
        Some(CompressionAlgorithm::Deflate)
    );
    assert_eq!(
        CompressionAlgorithm::from_str("identity"),
        Some(CompressionAlgorithm::Identity)
    );
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

    // Prefer brotli.
    let alg = mw.select_algorithm("gzip, br");
    assert_eq!(alg, Some(CompressionAlgorithm::Brotli));

    // Only gzip.
    let alg = mw.select_algorithm("gzip");
    assert_eq!(alg, Some(CompressionAlgorithm::Gzip));

    // With qvalues.
    let alg = mw.select_algorithm("gzip;q=0.5, br;q=1.0");
    assert_eq!(alg, Some(CompressionAlgorithm::Brotli));

    // Identity only.
    let alg = mw.select_algorithm("identity");
    assert_eq!(alg, Some(CompressionAlgorithm::Identity));

    // Identity;q=0 means no compression.
    let alg = mw.select_algorithm("identity;q=0");
    assert_eq!(alg, None);
}
