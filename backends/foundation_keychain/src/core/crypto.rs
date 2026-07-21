//! Cross-platform password crypto helpers (spec-57, F008 Stage 1).
//!
//! WHY: Per [decision 01](../../../specifications/57-foundation-social-and-keychain/decisions/01-crypto-backend.md)
//! the keychain owns no crypto — it derives password verifiers via
//! `foundation_auth::shared::password_hash`. But that PBKDF2 is **sync + infallible
//! on native** and **async + fallible (WebCrypto) on wasm**. These wrappers hide
//! that split behind a single always-`async` API so handlers have one call site.
//!
//! WHAT: `derive_password_hash` / `verify_password_hash` over PBKDF2-HMAC-SHA256,
//! `random_salt`, and a constant-time comparison.
//!
//! HOW: server-side verifier = PBKDF2 of the client-provided `master_password_hash`
//! with a per-user random salt at [`SERVER_ITERATIONS`]. On login the same
//! derivation is recomputed and compared in constant time.

use foundation_auth::shared::password_hash::pbkdf2_derive;

use crate::core::error::AppResult;

/// Derived-key length (bytes) for the server-side password verifier.
pub const KEY_LEN: u32 = 32;

/// Server-side re-hash iteration count. WebCrypto caps PBKDF2 at 100k, so this is
/// shared by both backends for identical output (`PBKDF2_SERVER_ITERATIONS`).
pub const SERVER_ITERATIONS: u32 =
    foundation_auth::shared::password_hash::PBKDF2_SERVER_ITERATIONS;

/// Derive the server-side password verifier from the client's master password hash.
#[cfg(not(target_family = "wasm"))]
pub async fn derive_password_hash(
    client_hash: &[u8],
    salt: &[u8],
    iterations: u32,
) -> AppResult<Vec<u8>> {
    Ok(pbkdf2_derive(client_hash, salt, iterations, KEY_LEN)
        .as_bytes()
        .to_vec())
}

/// Derive the server-side password verifier from the client's master password hash.
#[cfg(target_family = "wasm")]
pub async fn derive_password_hash(
    client_hash: &[u8],
    salt: &[u8],
    iterations: u32,
) -> AppResult<Vec<u8>> {
    let key = pbkdf2_derive(client_hash, salt, iterations, KEY_LEN)
        .await
        .map_err(|e| crate::core::error::AppError::Internal(format!("pbkdf2 derive failed: {e:?}")))?;
    Ok(key.as_bytes().to_vec())
}

/// Recompute the verifier and compare it to the stored one in constant time.
pub async fn verify_password_hash(
    client_hash: &[u8],
    salt: &[u8],
    expected: &[u8],
    iterations: u32,
) -> AppResult<bool> {
    let derived = derive_password_hash(client_hash, salt, iterations).await?;
    Ok(constant_time_eq(&derived, expected))
}

/// Constant-time byte comparison (avoids leaking match position via timing).
#[must_use]
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// A fresh 16-byte random salt (UUID v4 bytes — cross-platform `getrandom`).
#[must_use]
pub fn random_salt() -> Vec<u8> {
    uuid::Uuid::new_v4().as_bytes().to_vec()
}

/// Compute the Bitwarden client-side master password hash from a raw password.
///
/// `bw login` sends the raw master password; `localhost/register` sends the
/// pre-hashed value.  This function computes the hash chain `bw` uses so the
/// server can verify it against the stored verifier:
/// ```text
///   masterKey  = PBKDF2-SHA256(password, email_lower, client_iterations, 256)
///   localHash  = PBKDF2-SHA256(masterKey, password, 1, 256)
/// ```
/// Returns `Some(base64(localHash))` on success, `None` if the password is too
/// short to be a real master password.
pub async fn derive_client_hash(
    password: &str,
    email: &str,
    client_iterations: u32,
) -> Result<Option<String>, crate::core::error::AppError> {
    use base64::Engine;
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let email_lower = email.trim().to_lowercase();
    if password.len() < 8 {
        return Ok(None);
    }
    let mut mk = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(password.as_bytes(), email_lower.as_bytes(), client_iterations, &mut mk);
    let mut lh = vec![0u8; 32];
    pbkdf2_hmac::<Sha256>(&mk, password.as_bytes(), 1, &mut lh);
    Ok(Some(base64::engine::general_purpose::STANDARD.encode(&lh)))
}

/// A fresh 32-byte random key (base64-encoded string).
///
/// Used as a placeholder `akey` when no client-provided key is sent at
/// registration — ensures `Key` in the login response is always non-null
/// so the `bw` CLI doesn't refuse to proceed.
#[must_use]
pub fn random_key() -> String {
    use base64::Engine;
    // Two UUID v4 values give us 32 random bytes.
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let mut bytes = [0u8; 32];
    bytes[..16].copy_from_slice(a.as_bytes());
    bytes[16..].copy_from_slice(b.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(bytes)
}
