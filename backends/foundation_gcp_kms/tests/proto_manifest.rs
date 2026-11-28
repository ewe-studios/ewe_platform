//! The vendored Cloud KMS protos are the recorded files.
//!
//! `proto/SHA256SUMS` holds the sha256 of every vendored file, and
//! `proto/README.md` names the source commit. This test recomputes the hashes,
//! so a changed or replaced proto fails until the record is updated.

use std::path::Path;

use sha2::{Digest, Sha256};

/// Lowercase hexadecimal text.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            use std::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[test]
fn proto_manifest_matches() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("proto");
    let manifest = std::fs::read_to_string(root.join("SHA256SUMS")).expect("read proto/SHA256SUMS");
    let mut checked = 0_usize;
    for line in manifest.lines().filter(|line| !line.trim().is_empty()) {
        let (expected, path) = line
            .split_once("  ")
            .unwrap_or_else(|| panic!("bad SHA256SUMS line: {line:?}"));
        let bytes = std::fs::read(root.join(path))
            .unwrap_or_else(|error| panic!("read proto/{path}: {error}"));
        let actual = hex(&Sha256::digest(&bytes));
        assert_eq!(
            expected, actual,
            "proto/{path} changed; update proto/SHA256SUMS and proto/README.md"
        );
        checked += 1;
    }
    assert!(checked > 0, "proto/SHA256SUMS lists no file");
}
