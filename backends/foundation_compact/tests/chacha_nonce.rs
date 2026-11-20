#![cfg(feature = "crypto-chacha")]

use foundation_compact::crypto::chacha::{decrypt, encrypt_with_nonce, ChaChaKey};

#[test]
fn supplied_nonce_preserves_the_wire_format() {
    let key = ChaChaKey::from_bytes([7; 32]);
    let nonce = [3; 12];
    let plaintext = b"test message";
    let ciphertext = encrypt_with_nonce(&key, nonce, plaintext).unwrap();
    assert_eq!(ciphertext[..12], nonce);
    assert_eq!(decrypt(&key, &ciphertext).unwrap().as_slice(), plaintext);
    assert_eq!(
        encrypt_with_nonce(&key, nonce, plaintext).unwrap(),
        ciphertext
    );
}
