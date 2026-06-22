//! ChaCha20-Poly1305 encryption — delegates to `foundation_compact::crypto::chacha`.

use crate::core::errors::{StorageError, StorageResult};

pub use foundation_compact::crypto::chacha::ChaChaKey as EncryptionKey;
pub use foundation_compact::crypto::{SecureBytes, SecureString};

/// Encrypt data using ChaCha20-Poly1305.
///
/// Returns: `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
///
/// # Errors
///
/// Returns `StorageError::Encryption` on cipher or entropy failure.
pub fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> StorageResult<Vec<u8>> {
    foundation_compact::crypto::chacha::encrypt(key, plaintext)
        .map_err(StorageError::Encryption)
}

/// Decrypt data produced by [`encrypt`].
///
/// Returns [`SecureBytes`] — the plaintext is zeroed when dropped.
///
/// # Errors
///
/// Returns `StorageError::Encryption` if data is too short, key is wrong,
/// or data was tampered with.
pub fn decrypt(key: &EncryptionKey, encrypted: &[u8]) -> StorageResult<SecureBytes> {
    foundation_compact::crypto::chacha::decrypt(key, encrypted)
        .map_err(StorageError::Encryption)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
