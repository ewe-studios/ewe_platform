#![cfg(feature = "crypto-chacha")]

use foundation_compact::crypto::chacha::{decrypt, encrypt_with_nonce, ChaChaKey};

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
fn supplied_nonce_preserves_the_wire_format() {
    let key = test_key(7);
    let nonce = test_nonce(3);
    let plaintext = b"test message";
    let ciphertext = encrypt_with_nonce(&key, nonce, plaintext).unwrap();
    assert_eq!(ciphertext[..12], nonce);
    assert_eq!(decrypt(&key, &ciphertext).unwrap().as_slice(), plaintext);
    assert_eq!(
        encrypt_with_nonce(&key, nonce, plaintext).unwrap(),
        ciphertext
    );
}

#[test]
fn changed_tag_fails_to_decrypt() {
    let key = test_key(7);
    let mut ciphertext = encrypt_with_nonce(&key, test_nonce(4), b"counter").unwrap();
    let tag_byte = ciphertext.last_mut().expect("ciphertext includes a tag");
    *tag_byte ^= 1;
    assert!(decrypt(&key, &ciphertext).is_err());
}
