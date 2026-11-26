//! The registry reference tests (feature 15, section 7).
//!
//! The rows are the table of section 6.1 and the properties of a parsed
//! reference. They need no registry and no network.

use foundation_sandbox::native::images::registry::{
    OciReference, TagOrDigest, DOCKER_HUB_REGISTRY,
};
use foundation_sandbox::shared::error::SandboxError;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

/// One fixed property configuration.
fn config() -> ProptestConfig {
    ProptestConfig {
        cases: 64,
        rng_seed: RngSeed::Fixed(0x0c1),
        ..ProptestConfig::default()
    }
}

/// Parses one reference and returns its three parts as text.
fn parts(text: &str) -> (String, String, String) {
    let reference = OciReference::parse(text).expect("the reference parses");
    (
        reference.registry,
        reference.repository,
        reference.reference.as_str().to_string(),
    )
}

/// The `InvalidSpec { field: "reference" }` error of a bad reference.
fn bad(text: &str) {
    match OciReference::parse(text) {
        Ok(reference) => panic!("the reference {text:?} parsed as {reference}"),
        Err(error) => match error.current_context() {
            SandboxError::InvalidSpec { field, .. } => {
                assert_eq!(field, "reference", "the error names the reference field");
            }
            other => panic!("the error of {text:?} is InvalidSpec: {other:?}"),
        },
    }
}

#[test]
fn reference_table() {
    let hub = DOCKER_HUB_REGISTRY.to_string();

    // Each row of section 6.1.
    let digest = format!("sha256:{}", "a".repeat(64));
    for (text, registry, repository, reference) in [
        (
            "alpine".to_string(),
            hub.clone(),
            "library/alpine".to_string(),
            "latest".to_string(),
        ),
        (
            "alpine:3.22".to_string(),
            hub.clone(),
            "library/alpine".to_string(),
            "3.22".to_string(),
        ),
        (
            "user/app".to_string(),
            hub.clone(),
            "user/app".to_string(),
            "latest".to_string(),
        ),
        (
            "docker.io/library/alpine".to_string(),
            hub.clone(),
            "library/alpine".to_string(),
            "latest".to_string(),
        ),
        (
            "ghcr.io/org/app:1".to_string(),
            "ghcr.io".to_string(),
            "org/app".to_string(),
            "1".to_string(),
        ),
        (
            format!("localhost:5000/test/a@{digest}"),
            "localhost:5000".to_string(),
            "test/a".to_string(),
            digest.clone(),
        ),
    ] {
        assert_eq!(
            parts(&text),
            (registry, repository, reference),
            "input {text}"
        );
    }

    // The digest form keeps its digest.
    let reference = OciReference::parse(&format!("localhost:5000/test/a@{digest}"))
        .expect("the digest reference parses");
    assert_eq!(reference.digest(), Some(digest.as_str()));
    assert!(matches!(reference.reference, TagOrDigest::Digest(_)));

    // A tag is not a digest.
    let reference = OciReference::parse("alpine:3.22").expect("the tag reference parses");
    assert_eq!(reference.digest(), None);
    assert!(matches!(reference.reference, TagOrDigest::Tag(_)));
}

#[test]
fn reference_display_roundtrip() {
    for text in [
        "alpine",
        "alpine:3.22",
        "user/app:1",
        "docker.io/library/alpine:latest",
        "ghcr.io/org/app:1",
        "localhost:5000/test/a:dev",
    ] {
        let reference = OciReference::parse(text).expect("the reference parses");
        let printed = reference.to_string();
        assert_eq!(
            OciReference::parse(&printed).expect("the printed reference parses"),
            reference,
            "the printed form {printed} parses to the same reference"
        );
    }
}

#[test]
fn reference_bad_inputs() {
    for text in [
        "",
        "@",
        "alpine@",
        "alpine:",
        "alpine@sha256:abc",
        "alpine@sha256:zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        "Alpine",
        "alpine:tag:extra",
        "alpine@sha256:\
         0000000000000000000000000000000000000000000000000000000000000000\
         @sha256:0000000000000000000000000000000000000000000000000000000000000000",
        "a//b",
        "/alpine",
        "alpine/",
        "alpine:.hidden",
        "alpine:-hidden",
        "user/Upper:1",
        "user//app:1",
        "user/app..x:1",
        "user/app_-x:1",
    ] {
        bad(text);
    }

    // The tag length limit is 128 characters.
    let long = "a".repeat(129);
    bad(&format!("alpine:{long}"));
    let longest = "a".repeat(128);
    assert!(OciReference::parse(&format!("alpine:{longest}")).is_ok());

    // A digest with an upper-case hex digit is not a canonical sha256 digest.
    bad(&format!("alpine@sha256:{}", "A".repeat(64)));
}

#[test]
fn reference_docker_hub_aliases() {
    for registry in ["docker.io", "index.docker.io", "registry-1.docker.io"] {
        let (found, repository, tag) = parts(&format!("{registry}/library/alpine:3.22"));
        assert_eq!(found, DOCKER_HUB_REGISTRY);
        assert_eq!(repository, "library/alpine");
        assert_eq!(tag, "3.22");
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn reference_roundtrip(
        host in prop::collection::vec("[a-z0-9]{1,6}", 2..4),
        repository in prop::collection::vec("[a-z0-9]{1,8}([._-][a-z0-9]{1,8})*", 1..4),
        tag in "[A-Za-z0-9_][A-Za-z0-9._-]{0,20}",
    ) {
        let registry = host.join(".");
        let repo = repository.join("/");
        let text = format!("{registry}/{repo}:{tag}");
        let parsed = OciReference::parse(&text).expect("the generated reference parses");
        prop_assert_eq!(parsed.registry.as_str(), registry.as_str());
        prop_assert_eq!(parsed.repository.as_str(), repo.as_str());
        prop_assert_eq!(parsed.reference_text(), tag.as_str());
        let printed = parsed.to_string();
        let again = OciReference::parse(&printed).expect("the printed reference parses");
        prop_assert_eq!(again, parsed);
    }

    #[test]
    fn reference_digest_roundtrip(
        host in prop::collection::vec("[a-z0-9]{1,6}", 2..4),
        repository in prop::collection::vec("[a-z0-9]{1,8}([._-][a-z0-9]{1,8})*", 1..4),
        digest in "[0-9a-f]{64}",
    ) {
        let registry = host.join(".");
        let repo = repository.join("/");
        let text = format!("{registry}/{repo}@sha256:{digest}");
        let parsed = OciReference::parse(&text).expect("the generated digest reference parses");
        let expected = format!("sha256:{digest}");
        prop_assert_eq!(parsed.digest(), Some(expected.as_str()));
        let printed = parsed.to_string();
        let again = OciReference::parse(&printed).expect("the printed reference parses");
        prop_assert_eq!(again, parsed);
    }
}
