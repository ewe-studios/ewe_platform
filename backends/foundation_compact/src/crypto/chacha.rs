//! ChaCha20-Poly1305 authenticated encryption.
//!
//! Uses [`EncryptionKey<32>`] (256-bit) for the key and a 12-byte random nonce.
//! Output format: `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
//! Decrypted plaintext is returned as [`SecureBytes`] (zeroed on drop).

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};

use super::key::{EncryptionKey, SecureBytes, generate_nonce};

/// 256-bit key for ChaCha20-Poly1305.
pub type ChaChaKey = EncryptionKey<32>;

/// Encrypt `plaintext` with ChaCha20-Poly1305.
///
/// Returns `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
///
/// # Errors
///
/// Returns an error string if cipher initialization or encryption fails.
pub fn encrypt(key: &ChaChaKey, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| format!("cipher init: {e}"))?;

    let nonce_bytes: [u8; 12] = generate_nonce();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| format!("encrypt: {e}"))?;

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt data produced by [`encrypt`].
///
/// Expects `nonce (12 bytes) || ciphertext || tag (16 bytes)`.
/// Returns [`SecureBytes`] — the plaintext is zeroed when dropped.
///
/// # Errors
///
/// Returns an error string if the input is too short, cipher init fails,
/// or authentication/decryption fails (wrong key or tampered data).
pub fn decrypt(key: &ChaChaKey, encrypted: &[u8]) -> Result<SecureBytes, String> {
    if encrypted.len() < 12 {
        return Err("encrypted data too short (need at least 12-byte nonce)".into());
    }

    let cipher = ChaCha20Poly1305::new_from_slice(key.as_bytes())
        .map_err(|e| format!("cipher init: {e}"))?;

    let nonce = Nonce::from_slice(&encrypted[..12]);
    let ciphertext = &encrypted[12..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("decrypt: {e}"))?;

    Ok(SecureBytes::new(plaintext))
}
