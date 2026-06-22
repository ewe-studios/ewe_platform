use foundation_db::{decrypt, encrypt, EncryptionKey, StorageError};

#[test]
fn encrypt_decrypt_roundtrip() {
    let key = EncryptionKey::generate();
    let plaintext = b"Hello, World!";

    let encrypted = encrypt(&key, plaintext).unwrap();
    assert_ne!(encrypted.as_slice(), plaintext.as_slice());

    let decrypted = decrypt(&key, &encrypted).unwrap();
    assert_eq!(decrypted.as_slice(), plaintext.as_slice());
}

#[test]
fn wrong_key_fails() {
    let key1 = EncryptionKey::generate();
    let key2 = EncryptionKey::generate();
    let plaintext = b"Secret message";

    let encrypted = encrypt(&key1, plaintext).unwrap();
    let result = decrypt(&key2, &encrypted);
    assert!(matches!(result, Err(StorageError::Encryption(_))));
}
