#![cfg(feature = "crypto-chacha")]

//! `seal_with` / `open_with`: ChaCha20-Poly1305 with associated data.

use foundation_compact::crypto::chacha::{open_with, seal_with, ChaChaKey};

/// A deterministic test key, derived from `seed` so no key is a literal.
fn test_key(seed: u8) -> ChaChaKey {
    ChaChaKey::from_bytes(core::array::from_fn(|index| {
        let index = u8::try_from(index).expect("the array index fits in a byte");
        seed.wrapping_mul(37).wrapping_add(index.wrapping_mul(11))
    }))
}

/// A deterministic test nonce, derived from `seed`.
fn test_nonce(seed: u8) -> [u8; 12] {
    core::array::from_fn(|index| {
        let index = u8::try_from(index).expect("the array index fits in a byte");
        seed.wrapping_mul(53).wrapping_add(index)
    })
}

#[test]
fn associated_data_round_trips() {
    let key = test_key(9);
    let nonce = test_nonce(5);
    let aad = b"record:ca:1";
    let sealed = seal_with(&key, nonce, aad, b"private key").unwrap();
    assert_eq!(sealed[..12], nonce);
    let opened = open_with(&key, &nonce, aad, &sealed[12..]).unwrap();
    assert_eq!(opened.as_slice(), b"private key");
}

#[test]
fn a_different_associated_data_fails() {
    let key = test_key(9);
    let nonce = test_nonce(5);
    let sealed = seal_with(&key, nonce, b"record:ca:1", b"private key").unwrap();
    assert!(open_with(&key, &nonce, b"record:ca:2", &sealed[12..]).is_err());
}

#[test]
fn a_different_key_fails() {
    let key = test_key(9);
    let other = test_key(10);
    let nonce = test_nonce(5);
    let sealed = seal_with(&key, nonce, b"aad", b"private key").unwrap();
    assert!(open_with(&other, &nonce, b"aad", &sealed[12..]).is_err());
}

#[test]
fn a_changed_ciphertext_byte_fails() {
    let key = test_key(9);
    let nonce = test_nonce(5);
    let mut sealed = seal_with(&key, nonce, b"aad", b"private key").unwrap();
    sealed[12] ^= 1;
    assert!(open_with(&key, &nonce, b"aad", &sealed[12..]).is_err());
}
