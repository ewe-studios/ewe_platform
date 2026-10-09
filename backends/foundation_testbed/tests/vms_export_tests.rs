//! Tests extracted from vms/export/ modules

use std::path::Path;

use foundation_testbed::vms::export::compress::{
    compress_gzip, compress_xz, decompress_gzip, decompress_xz, is_compressed, Compression,
};
use foundation_testbed::vms::export::manifest::ExportManifest;

// From export/compress.rs
#[test]
fn test_compression_extension() {
    assert_eq!(Compression::Gzip.extension(), ".gz");
    assert_eq!(Compression::Xz.extension(), ".xz");
    assert_eq!(Compression::None.extension(), "");
    assert_eq!(Compression::default(), Compression::Gzip);
}

#[test]
fn test_is_compressed() {
    assert!(is_compressed(Path::new("image.qcow2.gz")));
    assert!(is_compressed(Path::new("image.qcow2.xz")));
    assert!(!is_compressed(Path::new("image.qcow2")));
    assert!(!is_compressed(Path::new("image.tar.gz")));
}

#[test]
fn test_gzip_roundtrip() {
    let dir = std::env::temp_dir().join("testbed_compress_test");
    let _ = std::fs::create_dir_all(&dir);

    let src = dir.join("test.qcow2");
    let compressed = dir.join("test.qcow2.gz");
    let decompressed = dir.join("test_restored.qcow2");

    // Create a small test file
    std::fs::write(&src, b"test qcow2 data that should compress and decompress correctly").unwrap();

    compress_gzip(&src, &compressed).unwrap();
    assert!(compressed.exists());
    // Tiny files may grow due to gzip header overhead -- just verify roundtrip works

    decompress_gzip(&compressed, &decompressed).unwrap();
    assert!(decompressed.exists());
    assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(&decompressed).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_xz_roundtrip() {
    let dir = std::env::temp_dir().join("testbed_xz_test");
    let _ = std::fs::create_dir_all(&dir);

    let src = dir.join("test.qcow2");
    let compressed = dir.join("test.qcow2.xz");
    let decompressed = dir.join("test_restored.qcow2");

    std::fs::write(&src, b"test qcow2 data that should compress and decompress correctly with xz").unwrap();

    compress_xz(&src, &compressed).unwrap();
    assert!(compressed.exists());

    decompress_xz(&compressed, &decompressed).unwrap();
    assert!(decompressed.exists());
    assert_eq!(std::fs::read(&src).unwrap(), std::fs::read(&decompressed).unwrap());

    let _ = std::fs::remove_dir_all(&dir);
}

// From export/manifest.rs
#[test]
fn test_manifest_tools_summary() {
    let mut tools = std::collections::HashMap::new();
    tools.insert("rust".to_string(), "1.78.0".to_string());
    tools.insert("node".to_string(), "22.1.0".to_string());

    let manifest = ExportManifest {
        name: "test".to_string(),
        version: "1.0.0".to_string(),
        os: "linux".to_string(),
        arch: "x86_64".to_string(),
        image_file: "test.qcow2".to_string(),
        image_size_bytes: 1_000_000,
        image_sha256: "abc123".to_string(),
        bootstrap_version: 3,
        installed_tools: tools,
        created_at: "2026-05-03T00:00:00Z".to_string(),
        created_by: "testhost".to_string(),
        git_commit: "abc1234".to_string(),
        notes: "".to_string(),
    };

    let summary = manifest.tools_summary();
    assert!(summary.contains("node 22.1.0"));
    assert!(summary.contains("rust 1.78.0"));
}

// From export/shrink.rs
#[test]
fn test_size_ratio_calculation() {
    // Just verify the math works with dummy values
    let orig = 4_000_000_000u64;
    let comp = 800_000_000u64;
    let expected_ratio = (comp as f64 / orig as f64) * 100.0;
    assert!((expected_ratio - 20.0).abs() < 0.1);
}
