//! The enrollment server proof of spec 81 `auth.md` section 9.1.1.
//!
//! WHY: Transport Layer Security (TLS) proves only that the client talks to
//! some holder of the server certificate. A proxy can end that TLS. Enrollment
//! therefore adds a proof that the server holds the private key of the
//! enrollment token.
//!
//! WHAT: the signed message, [`SecretIssuer::sign_enroll_proof`], and
//! [`VerifyingKey::verify_enroll_proof`]. The client checks the proof with the
//! public key in the token.
//!
//! HOW: the message starts with a domain and a zero byte. Four length-prefixed
//! fields follow, each with a big-endian `u32` length: the token secret ID,
//! the nonce, the client key or certificate signing request (CSR) hash, and
//! the client ID.

use zeroize::Zeroizing;

use super::{SecretError, VerifyingKey};

/// Separates this signature from every other use of the same key.
pub const ENROLL_PROOF_DOMAIN: &str = "ewe.agent_rpc.enroll_proof.v1";

/// The exact bytes that the server signs, for an audit or a golden test.
///
/// `client_key` is the client public key, or the SHA-256 of the client CSR.
#[must_use]
pub fn enroll_proof_message(
    secret_id: &str,
    nonce: &[u8],
    client_key: &[u8],
    client_id: &str,
) -> Vec<u8> {
    message(secret_id, nonce, client_key, client_id).to_vec()
}

/// The bytes that the server signs.
///
/// `client_key` is the client public key, or the SHA-256 of the client CSR.
pub(crate) fn message(
    secret_id: &str,
    nonce: &[u8],
    client_key: &[u8],
    client_id: &str,
) -> Zeroizing<Vec<u8>> {
    let parts = [
        secret_id.as_bytes(),
        nonce,
        client_key,
        client_id.as_bytes(),
    ];
    let capacity =
        ENROLL_PROOF_DOMAIN.len() + 1 + parts.iter().map(|part| 4 + part.len()).sum::<usize>();
    let mut message = Zeroizing::new(Vec::with_capacity(capacity));
    message.extend_from_slice(ENROLL_PROOF_DOMAIN.as_bytes());
    message.push(0);
    for part in parts {
        // A field longer than 4 GiB is outside every supported input. The
        // length still fits in the frame because it is cut to `u32::MAX`.
        let len = u32::try_from(part.len()).unwrap_or(u32::MAX);
        message.extend_from_slice(&len.to_be_bytes());
        message.extend_from_slice(part);
    }
    message
}

impl VerifyingKey {
    /// Check a server proof.
    ///
    /// Returns `Ok(true)` when the signature is good, `Ok(false)` when it is
    /// wrong, and an error when the key or the algorithm is unusable.
    ///
    /// # Errors
    /// [`SecretError::UnsupportedAlgorithm`] when this build cannot verify the
    /// key's algorithm.
    pub fn verify_enroll_proof(
        &self,
        secret_id: &str,
        nonce: &[u8],
        client_key: &[u8],
        client_id: &str,
        proof: &[u8],
    ) -> Result<bool, SecretError> {
        let message = message(secret_id, nonce, client_key, client_id);
        self.verify(&message, proof)
    }
}
