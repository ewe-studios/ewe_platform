//! SHA-256 fingerprints of public material, in lowercase hexadecimal.
//!
//! WHY: a fingerprint lets a log line or a database row name a public key, a
//! certificate or a signature without holding the bytes. The same text form is
//! used on every target, so a native host and a Worker compare the same string.
//!
//! WHAT: [`public_fingerprint`] and [`public_fingerprint_hex`].
//!
//! WHEN NOT TO USE IT: never hash a password, a passphrase or a user-chosen
//! secret with these helpers. A fast hash is weak for such input, and the
//! crate uses Argon2id for a password ([`crate::keystore::PassphraseSealer`]).
//! Every input here is public: a public key, a certificate, a signature or a
//! protocol nonce that travels on the wire. Do not add a caller that hashes a
//! secret with this module; add an Argon2id step instead.
//!
//! HOW: `sha2` builds the digest. It is pure Rust, so it builds for wasm.

use sha2::{Digest, Sha256};

/// The SHA-256 digest of public `bytes`.
#[must_use]
pub fn public_fingerprint(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Lowercase hexadecimal SHA-256 of public `bytes`.
#[must_use]
pub fn public_fingerprint_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    public_fingerprint(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}
