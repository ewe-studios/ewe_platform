//! Seed-derivation and key-parsing tests (spec-55, feature 01; decision 03).
//!
//! WHY: The whole "share the secret, share the network" property rests on *deterministic*
//! derivation — the same seed must yield the same bootstrap keypair and PSKs in any
//! process — and on rejecting malformed seeds/keys (no silent defaults).

use foundation_wireguard::{IdentityKeypair, NetworkId, PeerPublicKey, SeedBits, WgSeed};
use tracing_test::traced_test;

#[test]
#[traced_test]
fn seed_derivation_is_deterministic_and_network_bound() {
    let seed = WgSeed::from_bytes(&[0x42u8; 32]).expect("32-byte seed");
    let network = seed.derive_network_id();

    let first = seed.derive_bootstrap(&network);

    // A second, independently reconstructed seed derives identical material.
    let reconstructed = WgSeed::from_base64url(&seed.to_base64url()).expect("round-trip base64url");
    let second = reconstructed.derive_bootstrap(&network);

    assert_eq!(
        first.public.as_bytes(),
        second.public.as_bytes(),
        "same seed + network => same bootstrap public key"
    );
    assert_eq!(first.psk, second.psk, "same seed => same WG PSK");
    assert_eq!(
        first.channel_psk, second.channel_psk,
        "same seed => same bootstrap channel PSK"
    );

    // Domain separation: the three derived values are mutually distinct.
    assert_ne!(
        first.psk, first.channel_psk,
        "WG tunnel PSK differs from bootstrap channel PSK"
    );
    assert_ne!(
        &first.psk[..],
        first.public.as_bytes(),
        "PSK differs from the public key"
    );

    // Network binding: a different network id yields a different keypair.
    let other_network = NetworkId::from_bytes([0x01u8; 16]);
    let other = seed.derive_bootstrap(&other_network);
    assert_ne!(
        first.public.as_bytes(),
        other.public.as_bytes(),
        "different network => different bootstrap key"
    );
}

#[test]
#[traced_test]
fn seed_128_and_256_supported_but_short_seeds_rejected() {
    let s128 = WgSeed::generate(SeedBits::Bits128).expect("gen 128");
    assert_eq!(s128.as_bytes().len(), 16);
    assert_eq!(s128.bits(), SeedBits::Bits128);

    let s256 = WgSeed::generate(SeedBits::Bits256).expect("gen 256");
    assert_eq!(s256.as_bytes().len(), 32);

    // No silent defaults: anything but 16/32 bytes is a hard error.
    assert!(WgSeed::from_bytes(&[0u8; 8]).is_err(), "8-byte seed rejected");
    assert!(WgSeed::from_bytes(&[0u8; 24]).is_err(), "24-byte seed rejected");
}

#[test]
#[traced_test]
fn peer_public_key_parses_hex_and_base64_round_trip() {
    let identity = IdentityKeypair::generate().expect("gen identity");
    let expected = *identity.public().as_bytes();

    // base64 (WireGuard canonical) round-trips.
    let from_b64 = PeerPublicKey::parse(&identity.public_base64()).expect("parse base64");
    assert_eq!(from_b64.as_bytes(), expected);

    // hex round-trips.
    let from_hex = PeerPublicKey::parse(&from_b64.to_hex()).expect("parse hex");
    assert_eq!(from_hex.as_bytes(), expected);

    // Garbage is rejected.
    assert!(PeerPublicKey::parse("not-a-key").is_err());
    assert!(PeerPublicKey::parse("").is_err());
}
