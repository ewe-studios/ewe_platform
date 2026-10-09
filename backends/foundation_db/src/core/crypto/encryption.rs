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