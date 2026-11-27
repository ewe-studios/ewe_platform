//! The public key of a client-key enrollment.
//!
//! WHY: a browser or a Worker enrolls with a public key whose private key
//! never leaves the client. The host stores the public key and checks the
//! client's refresh signatures with it.
//!
//! WHAT: [`ClientKeyAlgorithm`] (`ed25519` or `p256`) and [`ClientPublicKey`]
//! (a checked key with a verify method).
//!
//! HOW: `ed25519-compact` checks Ed25519 and `p256` checks ECDSA with SHA-256.
//! Both are pure Rust, so the code builds for wasm.

use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature as P256Signature, VerifyingKey as P256VerifyingKey};

use crate::fingerprint::public_fingerprint_hex;

use super::SecretError;

/// The algorithm of a client public key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKeyAlgorithm {
    /// Ed25519, raw 32 bytes (Request for Comments (RFC) 8032).
    Ed25519,
    /// ECDSA P-256, uncompressed SEC1, 65 bytes.
    P256,
}

impl ClientKeyAlgorithm {
    /// The name in the `alg` field.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::P256 => "p256",
        }
    }

    /// Read the name in the `alg` field.
    ///
    /// # Errors
    /// [`SecretError::BadKey`] for another name.
    pub fn parse(name: &str) -> Result<Self, SecretError> {
        match name {
            "ed25519" => Ok(Self::Ed25519),
            "p256" => Ok(Self::P256),
            other => Err(SecretError::BadKey(format!(
                "unknown client key algorithm {other:?}; use ed25519 or p256"
            ))),
        }
    }

    /// The exact byte length of a key of this algorithm.
    #[must_use]
    pub const fn key_len(self) -> usize {
        match self {
            Self::Ed25519 => 32,
            Self::P256 => 65,
        }
    }
}

/// A checked client public key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientPublicKey {
    alg: ClientKeyAlgorithm,
    bytes: Vec<u8>,
}

impl ClientPublicKey {
    /// Check a key from the wire.
    ///
    /// # Errors
    /// [`SecretError::BadKey`] for an unknown algorithm, a wrong length or a
    /// key that the curve refuses.
    pub fn parse(alg: &str, bytes: &[u8]) -> Result<Self, SecretError> {
        let alg = ClientKeyAlgorithm::parse(alg)?;
        if bytes.len() != alg.key_len() {
            return Err(SecretError::BadKey(format!(
                "a {} key has {} bytes; this one has {}",
                alg.as_str(),
                alg.key_len(),
                bytes.len()
            )));
        }
        match alg {
            ClientKeyAlgorithm::Ed25519 => {
                ed25519_compact::PublicKey::from_slice(bytes)
                    .map_err(|error| SecretError::BadKey(format!("ed25519: {error}")))?;
            }
            ClientKeyAlgorithm::P256 => {
                P256VerifyingKey::from_sec1_bytes(bytes)
                    .map_err(|error| SecretError::BadKey(format!("p256: {error}")))?;
            }
        }
        Ok(Self {
            alg,
            bytes: bytes.to_vec(),
        })
    }

    /// The algorithm.
    #[must_use]
    pub const fn algorithm(&self) -> ClientKeyAlgorithm {
        self.alg
    }

    /// The raw public key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The lowercase hexadecimal SHA-256 of the raw key. Safe to log.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        public_fingerprint_hex(&self.bytes)
    }

    /// Check one signature over `message`.
    ///
    /// A P-256 signature can be Distinguished Encoding Rules (DER) or the raw
    /// 64-byte `r || s` form that `WebCrypto` makes.
    ///
    /// # Errors
    /// [`SecretError::BadSignature`] when the signature does not match.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), SecretError> {
        match self.alg {
            ClientKeyAlgorithm::Ed25519 => {
                let key = ed25519_compact::PublicKey::from_slice(&self.bytes)
                    .map_err(|error| SecretError::BadKey(format!("ed25519: {error}")))?;
                let signature = ed25519_compact::Signature::from_slice(signature)
                    .map_err(|_| SecretError::BadSignature)?;
                key.verify(message, &signature)
                    .map_err(|_| SecretError::BadSignature)
            }
            ClientKeyAlgorithm::P256 => {
                let key = P256VerifyingKey::from_sec1_bytes(&self.bytes)
                    .map_err(|error| SecretError::BadKey(format!("p256: {error}")))?;
                let signature = p256_signature(signature)?;
                key.verify(message, &signature)
                    .map_err(|_| SecretError::BadSignature)
            }
        }
    }
}

/// Read a P-256 signature in DER or raw form.
fn p256_signature(signature: &[u8]) -> Result<P256Signature, SecretError> {
    if let Ok(der) = P256Signature::from_slice(signature) {
        return Ok(der);
    }
    if signature.len() != 64 {
        return Err(SecretError::BadSignature);
    }
    let raw = p256::elliptic_curve::generic_array::GenericArray::clone_from_slice(signature);
    P256Signature::from_bytes(&raw).map_err(|_| SecretError::BadSignature)
}
