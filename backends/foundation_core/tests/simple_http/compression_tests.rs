//! Unit tests for `client::compression` moved into the canonical units test tree.
//!
//! These tests are non-destructive copies of the original in-crate `#[cfg(test)]`
//! module. They exercise compression encoding parsing, configuration, and decompression
//! functionality in a fast, deterministic manner suitable for unit test execution
//! under `tests/backends/foundation_core/units/simple_http/`.

use foundation_core::wire::simple_http::client::{
    CompressionConfig, ContentEncoding, DecompressingReader,
};
use foundation_core::wire::simple_http::HttpClientError;
use std::io;
use std::io::Read;

/// WHY: Content-Encoding parsing must be case-insensitive per HTTP spec
/// WHAT: Verify `from_header()` correctly parses all standard encodings
#[test]
fn test_content_encoding_from_header_gzip() {
    assert_eq!(ContentEncoding::from_header("gzip"), ContentEncoding::Gzip);
    assert_eq!(ContentEncoding::from_header("GZIP"), ContentEncoding::Gzip);
    assert_eq!(ContentEncoding::from_header("GzIp"), ContentEncoding::Gzip);
}

/// WHY: Brotli uses "br" encoding name per RFC 7932
/// WHAT: Verify "br" maps to Brotli variant
#[test]
fn test_content_encoding_from_header_brotli() {
    assert_eq!(ContentEncoding::from_header("br"), ContentEncoding::Brotli);
    assert_eq!(ContentEncoding::from_header("BR"), ContentEncoding::Brotli);
}

/// WHY: Deflate is standard HTTP compression algorithm
/// WHAT: Verify "deflate" maps to Deflate variant
#[test]
fn test_content_encoding_from_header_deflate() {
    assert_eq!(
        ContentEncoding::from_header("deflate"),
        ContentEncoding::Deflate
    );
    assert_eq!(
        ContentEncoding::from_header("DEFLATE"),
        ContentEncoding::Deflate
    );
}

/// WHY: Identity means no compression (default/fallback)
/// WHAT: Verify "identity" maps to Identity variant
#[test]
fn test_content_encoding_from_header_identity() {
    assert_eq!(
        ContentEncoding::from_header("identity"),
        ContentEncoding::Identity
    );
    assert_eq!(
        ContentEncoding::from_header("IDENTITY"),
        ContentEncoding::Identity
    );
}

/// WHY: Unknown encodings should be captured, not cause errors
/// WHAT: Verify unknown encoding names return Unknown variant with original value
#[test]
fn test_content_encoding_from_header_unknown() {
    match ContentEncoding::from_header("compress") {
        ContentEncoding::Unknown(s) => assert_eq!(s, "compress"),
        _ => panic!("Expected Unknown variant"),
    }

    match ContentEncoding::from_header("custom-encoding") {
        ContentEncoding::Unknown(s) => assert_eq!(s, "custom-encoding"),
        _ => panic!("Expected Unknown variant"),
    }
}

/// WHY: Default config should enable compression with all encodings
/// WHAT: Verify `Default::default()` returns compression-enabled config
#[test]
fn test_compression_config_default() {
    let config = CompressionConfig::default();
    assert!(config.add_accept_encoding);
    assert!(config.auto_decompress);
    assert_eq!(config.supported_encodings.len(), 3);
    assert_eq!(config.supported_encodings[0], ContentEncoding::Brotli);
    assert_eq!(config.supported_encodings[1], ContentEncoding::Gzip);
    assert_eq!(config.supported_encodings[2], ContentEncoding::Deflate);
}

/// WHY: Must be able to disable compression entirely
/// WHAT: Verify `disabled()` returns config with compression off
#[test]
fn test_compression_config_disabled() {
    let config = CompressionConfig::disabled();
    assert!(!config.add_accept_encoding);
    assert!(!config.auto_decompress);
    assert_eq!(config.supported_encodings.len(), 0);
}

/// WHY: Accept-Encoding header must be properly formatted comma-separated list
/// WHAT: Verify `accept_encoding_value()` generates correct header value
#[test]
fn test_compression_config_accept_encoding_value() {
    let config = CompressionConfig::default();
    let value = config.accept_encoding_value();

    // Should contain all three encodings
    assert!(value.contains("br"));
    assert!(value.contains("gzip"));
    assert!(value.contains("deflate"));

    // Should be comma-separated
    assert!(value.contains(", "));
}

/// WHY: Empty supported encodings should produce empty header value
/// WHAT: Verify disabled config produces empty accept-encoding value
#[test]
fn test_compression_config_empty_accept_encoding() {
    let config = CompressionConfig::disabled();
    let value = config.accept_encoding_value();
    assert_eq!(value, "");
}

/// WHY: Should be able to customize supported encodings
/// WHAT: Verify custom encoding list generates correct header value
#[test]
fn test_compression_config_custom_encodings() {
    let config = CompressionConfig::new(
        true,
        true,
        vec![ContentEncoding::Gzip, ContentEncoding::Deflate],
    );
    let value = config.accept_encoding_value();

    assert!(value.contains("gzip"));
    assert!(value.contains("deflate"));
    assert!(!value.contains("br"));
}

/// WHY: Identity encoding (no compression) should pass through data unchanged
/// WHAT: Verify `DecompressingReader` with Identity works as pass-through
#[test]
fn test_decompressing_reader_identity() {
    let data = b"Hello, World! This is uncompressed data.";
    let cursor = io::Cursor::new(data.as_ref());

    let mut reader = DecompressingReader::new(cursor, &ContentEncoding::Identity)
        .expect("Failed to create identity reader");

    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .expect("Failed to read identity data");

    assert_eq!(output, data);
}

/// WHY: Gzip is common HTTP compression format
/// WHAT: Verify DecompressingReader correctly decompresses gzip data
#[test]
#[cfg(feature = "gzip")]
fn test_decompressing_reader_gzip() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    // Create gzip compressed data
    let original = b"Hello, World! This is test data for gzip compression.";
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original).expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to finish compression");

    // Decompress using DecompressingReader
    let cursor = io::Cursor::new(compressed);
    let mut reader =
        DecompressingReader::new(cursor, &ContentEncoding::Gzip).expect("Failed to create reader");

    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .expect("Failed to read gzip data");

    assert_eq!(output, original);
}

/// WHY: Deflate is another common HTTP compression format
/// WHAT: Verify DecompressingReader correctly decompresses deflate data
#[test]
#[cfg(any(feature = "gzip", feature = "deflate"))]
fn test_decompressing_reader_deflate() {
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    // Create deflate compressed data
    let original = b"Hello, World! This is test data for deflate compression.";
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(original).expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to finish compression");

    // Decompress using DecompressingReader
    let cursor = io::Cursor::new(compressed);
    let mut reader = DecompressingReader::new(cursor, &ContentEncoding::Deflate)
        .expect("Failed to create reader");

    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .expect("Failed to read deflate data");

    assert_eq!(output, original);
}

/// WHY: Brotli offers better compression than gzip
/// WHAT: Verify `DecompressingReader` correctly decompresses brotli data
#[test]
#[cfg(feature = "brotli")]
fn test_decompressing_reader_brotli() {
    use brotli::enc::BrotliEncoderParams;
    use std::io::Write;

    // Create brotli compressed data
    let original = b"Hello, World! This is test data for brotli compression.";
    let mut compressed = Vec::new();
    {
        let params = BrotliEncoderParams::default();
        let mut encoder = brotli::CompressorWriter::with_params(&mut compressed, 4096, &params);
        encoder.write_all(original).expect("Failed to write");
        encoder.flush().expect("Failed to flush");
    }

    // Decompress using DecompressingReader
    let cursor = io::Cursor::new(compressed);
    let mut reader = DecompressingReader::new(cursor, &ContentEncoding::Brotli)
        .expect("Failed to create reader");

    let mut output = Vec::new();
    reader
        .read_to_end(&mut output)
        .expect("Failed to read brotli data");

    assert_eq!(output, original);
}

/// WHY: Unknown encodings should be rejected gracefully
/// WHAT: Verify `DecompressingReader` returns error for unknown encodings
#[test]
fn test_decompressing_reader_unknown_encoding() {
    let data = b"Some data";
    let cursor = io::Cursor::new(data.as_ref());

    let result = DecompressingReader::new(cursor, &ContentEncoding::Unknown("custom".to_string()));

    match result {
        Err(HttpClientError::UnsupportedEncoding(enc)) => {
            assert_eq!(enc, "custom");
        }
        _ => panic!("Expected UnsupportedEncoding error"),
    }
}

/// WHY: Streaming decompression must work incrementally (not buffer entire response)
/// WHAT: Verify DecompressingReader works with small buffer reads (streaming)
#[test]
#[cfg(feature = "gzip")]
fn test_decompressing_reader_streaming() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Write;

    // Create large gzip compressed data
    let original = "x".repeat(10000);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(original.as_bytes())
        .expect("Failed to write");
    let compressed = encoder.finish().expect("Failed to finish compression");

    // Read in small chunks to verify streaming behavior
    let cursor = io::Cursor::new(compressed);
    let mut reader =
        DecompressingReader::new(cursor, &ContentEncoding::Gzip).expect("Failed to create reader");

    let mut output = Vec::new();
    let mut buffer = [0u8; 128]; // Small buffer

    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => output.extend_from_slice(&buffer[..n]),
            Err(e) => panic!("Read error: {}", e),
        }
    }

    assert_eq!(output, original.as_bytes());
}
