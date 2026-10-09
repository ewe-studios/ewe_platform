//! Bootstrap parser + token codec tests (spec-55, feature 02; decision 04).
//!
//! WHY: The token must round-trip losslessly, reject corruption/truncation, and the
//! single parser must correctly distinguish a `wg1_` token from a bare seed — with no
//! silent defaults.

use std::net::SocketAddr;

use foundation_wireguard::shared::bootstrap::{flag, BootstrapFlags, TokenSecret};
use foundation_wireguard::{BootstrapToken, SeedBits, WgBootstrap, WgSeed};
use tracing_test::traced_test;

fn sample_token() -> BootstrapToken {
    let seed = WgSeed::from_bytes(&[0x33u8; 32]).unwrap();
    let network_id = seed.derive_network_id();
    let endpoints: Vec<SocketAddr> = vec![
        "203.0.113.7:51820".parse().unwrap(),
        "[2001:db8::1]:51820".parse().unwrap(),
    ];
    BootstrapToken::new(network_id, TokenSecret::Seed(seed), endpoints)
        .with_flags(BootstrapFlags(flag::RELAY_CAPABLE_HINT | flag::EPHEMERAL))
        .with_expiry(1_900_000_000)
}

#[test]
#[traced_test]
fn token_round_trips_all_fields() {
    let token = sample_token();
    let encoded = token.to_token();
    assert!(encoded.starts_with("wg1_"), "has version prefix");

    let parsed = BootstrapToken::parse(&encoded).expect("parse own token");
    assert_eq!(parsed.network_id.as_bytes(), token.network_id.as_bytes());
    assert_eq!(parsed.endpoints, token.endpoints, "both endpoints preserved");
    assert!(parsed.flags.relay_capable_hint());
    assert!(parsed.flags.ephemeral());
    assert_eq!(parsed.expires_at, Some(1_900_000_000));
    match parsed.secret {
        TokenSecret::Seed(seed) => assert_eq!(seed.as_bytes(), &[0x33u8; 32]),
        other => panic!("expected seed secret, got {other:?}"),
    }
}

#[test]
#[traced_test]
fn token_rejects_corruption_and_truncation() {
    let encoded = sample_token().to_token();

    // Flip a byte in the middle → CRC mismatch.
    let mut corrupt: Vec<u8> = encoded.clone().into_bytes();
    let mid = corrupt.len() / 2;
    corrupt[mid] ^= 0x01;
    let corrupt = String::from_utf8(corrupt).unwrap();
    assert!(
        BootstrapToken::parse(&corrupt).is_err(),
        "corrupted token rejected"
    );

    // Truncate the body.
    let truncated = &encoded[..encoded.len() - 6];
    assert!(
        BootstrapToken::parse(truncated).is_err(),
        "truncated token rejected"
    );

    // Missing prefix.
    assert!(BootstrapToken::parse("nope_abc").is_err());
}

#[test]
#[traced_test]
fn parse_dispatches_token_vs_bare_seed() {
    // Token form.
    let encoded = sample_token().to_token();
    match WgBootstrap::parse(&encoded).expect("parse token") {
        WgBootstrap::Token(t) => assert_eq!(t.endpoints.len(), 2),
        other => panic!("expected token, got {other:?}"),
    }

    // Bare seed — base64url form.
    let seed = WgSeed::generate(SeedBits::Bits256).unwrap();
    let b64 = seed.to_base64url();
    match WgBootstrap::parse(&b64).expect("parse base64url seed") {
        WgBootstrap::Seed(s) => assert_eq!(s.as_bytes(), seed.as_bytes()),
        other => panic!("expected seed, got {other:?}"),
    }

    // Bare seed — hex form (64 hex chars = 32 bytes).
    let hex_seed = hex::encode([0xABu8; 32]);
    match WgBootstrap::parse(&hex_seed).expect("parse hex seed") {
        WgBootstrap::Seed(s) => assert_eq!(s.as_bytes(), &[0xABu8; 32]),
        other => panic!("expected seed, got {other:?}"),
    }

    // A seed carried either way is retrievable.
    assert!(WgBootstrap::parse(&encoded).unwrap().seed().is_some());
    assert!(WgBootstrap::parse(&b64).unwrap().seed().is_some());
}

#[test]
#[traced_test]
fn token_requires_endpoints_and_honours_expiry() {
    let seed = WgSeed::from_bytes(&[0x11u8; 16]).unwrap();
    let network_id = seed.derive_network_id();

    // No endpoints → minting is fine, but parsing rejects it (>=1 required).
    let no_endpoints = BootstrapToken::new(network_id, TokenSecret::Seed(seed.clone()), vec![]);
    let encoded = no_endpoints.to_token();
    assert!(
        BootstrapToken::parse(&encoded).is_err(),
        "token with no endpoints rejected"
    );

    // Expiry check.
    let token = BootstrapToken::new(
        network_id,
        TokenSecret::Seed(seed),
        vec!["10.0.0.1:51820".parse().unwrap()],
    )
    .with_expiry(1000);
    assert!(!token.is_expired(999), "not yet expired");
    assert!(token.is_expired(1001), "expired past TTL");
}
