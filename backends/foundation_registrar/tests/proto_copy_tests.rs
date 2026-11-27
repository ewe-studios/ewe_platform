//! The vendored registration proto is byte-identical to the proxy's copy.
//!
//! WHY: the contract is defined once, in `foundation_proxy/proto`. A package
//! tarball carries only its own files, so this crate keeps a copy to build
//! from; a drift between the two copies must fail a test, not reach a release
//! (issue #877).
//! WHAT: compare the two files byte for byte.
//! HOW: read both from the workspace; skip with a clear message when this
//! crate is built from its own tarball, where the canonical file is absent.

use std::path::Path;

const PROTO: &str = "proto/ewe/proxy/registration/v1/registration.proto";

#[test]
fn the_vendored_proto_matches_the_proxy_copy() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mine = manifest.join(PROTO);
    let canonical = manifest.join("../foundation_proxy").join(PROTO);
    if !canonical.is_file() {
        println!(
            "skipped: {canonical:?} is absent; this crate is built from its tarball"
        );
        return;
    }
    let mine_bytes = std::fs::read(&mine).expect("read the vendored proto");
    let canonical_bytes = std::fs::read(&canonical).expect("read the canonical proto");
    assert_eq!(
        mine_bytes, canonical_bytes,
        "the vendored proto differs from foundation_proxy's copy; copy it again"
    );
}
