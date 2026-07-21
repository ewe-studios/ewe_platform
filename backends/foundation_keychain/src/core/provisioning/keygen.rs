//! SSH key generation + at-rest `age` encryption (feature 009, native).

use std::io::{Read, Write};

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use secrecy::SecretString;
use ssh_key::{Algorithm, LineEnding, PrivateKey};

use crate::core::error::{AppError, AppResult};

/// A freshly generated OpenSSH key pair.
pub struct GeneratedKey {
    pub key_type: String,
    pub public_openssh: String,
    pub private_openssh: String,
}

/// Generate an Ed25519 (default) or RSA key pair with an OpenSSH comment.
pub fn generate(key_type: &str, comment: &str) -> AppResult<GeneratedKey> {
    let (algo, canonical) = match key_type {
        "rsa" | "rsa4096" => (Algorithm::Rsa { hash: None }, "rsa4096"),
        "ed25519" | "" => (Algorithm::Ed25519, "ed25519"),
        other => return Err(AppError::BadRequest(format!("unsupported key_type: {other}"))),
    };
    let mut key = PrivateKey::random(&mut rand::thread_rng(), algo)
        .map_err(|e| AppError::Internal(format!("ssh key generation failed: {e}")))?;
    if !comment.is_empty() {
        key.set_comment(comment);
    }
    let private_openssh = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| AppError::Internal(format!("encode private key: {e}")))?
        .to_string();
    let public_openssh = key
        .public_key()
        .to_openssh()
        .map_err(|e| AppError::Internal(format!("encode public key: {e}")))?;
    Ok(GeneratedKey {
        key_type: canonical.to_string(),
        public_openssh,
        private_openssh,
    })
}

/// Encrypt `plaintext` with the master passphrase → base64(age ciphertext).
pub fn encrypt(master_key: &str, plaintext: &[u8]) -> AppResult<String> {
    let encryptor = age::Encryptor::with_user_passphrase(SecretString::new(master_key.to_string()));
    let mut encrypted = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut encrypted)
        .map_err(|e| AppError::Internal(format!("age wrap: {e}")))?;
    writer
        .write_all(plaintext)
        .map_err(|e| AppError::Internal(format!("age write: {e}")))?;
    writer
        .finish()
        .map_err(|e| AppError::Internal(format!("age finish: {e}")))?;
    Ok(STANDARD.encode(&encrypted))
}

/// Decrypt base64(age ciphertext) with the master passphrase.
pub fn decrypt(master_key: &str, ciphertext_b64: &str) -> AppResult<Vec<u8>> {
    let ciphertext = STANDARD
        .decode(ciphertext_b64)
        .map_err(|e| AppError::Internal(format!("age b64 decode: {e}")))?;
    let decryptor = match age::Decryptor::new(&ciphertext[..])
        .map_err(|e| AppError::Internal(format!("age decryptor: {e}")))?
    {
        age::Decryptor::Passphrase(d) => d,
        age::Decryptor::Recipients(_) => {
            return Err(AppError::Internal("age: expected passphrase ciphertext".into()))
        }
    };
    let mut reader = decryptor
        .decrypt(&SecretString::new(master_key.to_string()), None)
        .map_err(|_| AppError::Internal("age decrypt failed (wrong master key?)".into()))?;
    let mut out = Vec::new();
    reader
        .read_to_end(&mut out)
        .map_err(|e| AppError::Internal(format!("age read: {e}")))?;
    Ok(out)
}
