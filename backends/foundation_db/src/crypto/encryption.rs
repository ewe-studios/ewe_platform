//! Encryption utilities using ChaCha20-Poly1305.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use rand::RngCore;
use zeroize::Zeroizing;

use crate::errors::{StorageError, StorageResult};

/// 256-bit encryption key for ChaCha20-Poly1305.
#[derive(Clone)]
pub struct EncryptionKey([u8; 32]);

impl EncryptionKey {
    /// Generate a new random encryption key.
    #[must_use]
    pub fn generate() -> Self {
        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        Self(key_bytes)
    }

    /// Create a key from raw bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for EncryptionKey {
    fn drop(&mut self) {
        // Securely zero out the key
        Zeroizing::new(self.0);
    }
}

/// Encrypt data using ChaCha20-Poly1305.
///
/// Returns: nonce (12 bytes) || ciphertext || tag (16 bytes)
///
/// # Errors
///
/// Returns an error if cipher initialization fails or encryption fails.
pub fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> StorageResult<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| StorageError::Encryption(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| StorageError::Encryption(e.to_string()))?;

    // Prepend nonce to ciphertext
    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);

    Ok(result)
}

/// Decrypt data using ChaCha20-Poly1305.
///
/// Expects: nonce (12 bytes) || ciphertext || tag (16 bytes)
///
/// # Errors
///
/// Returns an error if the data is too short, cipher initialization fails, or decryption fails.
pub fn decrypt(key: &EncryptionKey, encrypted: &[u8]) -> StorageResult<Vec<u8>> {
    if encrypted.len() < 12 {
        return Err(StorageError::Encryption(
            "Encrypted data too short".to_string(),
        ));
    }

    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| StorageError::Encryption(e.to_string()))?;

    // Extract nonce and ciphertext
    let nonce_bytes = &encrypted[0..12];
    let ciphertext = &encrypted[12..];

    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| StorageError::Encryption(e.to_string()))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = EncryptionKey::generate();
        let plaintext = b"Hello, World!";

        let encrypted = encrypt(&key, plaintext).unwrap();
        assert_ne!(encrypted.as_slice(), plaintext.as_slice());

        let decrypted = decrypt(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext.as_slice());
    }

    #[test]
    fn test_decrypt_with_wrong_key() {
        let key1 = EncryptionKey::generate();
        let key2 = EncryptionKey::generate();
        let plaintext = b"Secret message";

        let encrypted = encrypt(&key1, plaintext).unwrap();

        // Decrypting with wrong key should fail
        let result = decrypt(&key2, &encrypted);
        assert!(matches!(result, Err(StorageError::Encryption(_))));
    }
}
