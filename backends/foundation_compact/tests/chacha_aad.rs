#![cfg(feature = "crypto-chacha")]

//! `seal_with` / `open_with`: ChaCha20-Poly1305 with associated data.

use foundation_compact::crypto::chacha::{open_with, seal_with, ChaChaKey};

#[test]
fn associated_data_round_trips() {
    let key = ChaChaKey::from_bytes([9; 32]);
    let nonce = [5; 12];
    let aad = b"record:ca:1";
    let sealed = seal_with(&key, nonce, aad, b"private key").unwrap();
    assert_eq!(sealed[..12], nonce);
    let opened = open_with(&key, &nonce, aad, &sealed[12..]).unwrap();
    assert_eq!(opened.as_slice(), b"private key");
}

#[test]
fn a_different_associated_data_fails() {
    let key = ChaChaKey::from_bytes([9; 32]);
    let nonce = [5; 12];
    let sealed = seal_with(&key, nonce, b"record:ca:1", b"private key").unwrap();
    assert!(open_with(&key, &nonce, b"record:ca:2", &sealed[12..]).is_err());
}

#[test]
fn a_different_key_fails() {
    let key = ChaChaKey::from_bytes([9; 32]);
    let other = ChaChaKey::from_bytes([10; 32]);
    let nonce = [5; 12];
    let sealed = seal_with(&key, nonce, b"aad", b"private key").unwrap();
    assert!(open_with(&other, &nonce, b"aad", &sealed[12..]).is_err());
}

#[test]
fn a_changed_ciphertext_byte_fails() {
    let key = ChaChaKey::from_bytes([9; 32]);
    let nonce = [5; 12];
    let mut sealed = seal_with(&key, nonce, b"aad", b"private key").unwrap();
    sealed[12] ^= 1;
    assert!(open_with(&key, &nonce, b"aad", &sealed[12..]).is_err());
}
