//! Password hashing — PBKDF2-HMAC-SHA256 and Argon2id.
//!
//! WHY: Bitwarden clients support two KDF types: PBKDF2 (type 0) and Argon2id (type 1).
//! The server must verify passwords hashed by either KDF. WebCrypto (WASM) only supports PBKDF2.
//!
//! WHAT: `PasswordHasher` trait with `pbkdf2` and `argon2id` implementations,
//! plus a `PasswordVerifier` that dispatches based on KDF type.
//!
//! HOW: Native uses `pbkdf2` + `hmac` + `sha2` crates and `argon2` crate.
//! WASM uses Web Crypto API for PBKDF2 only (Argon2id not available).

use zeroize::Zeroizing;

// ===========================================================================
// KDF type constants (matching Bitwarden wire format)
// ===========================================================================

/// PBKDF2-SHA256 KDF type.
pub const KDF_TYPE_PBKDF2: i32 = 0;
/// Argon2id KDF type.
pub const KDF_TYPE_ARGON2ID: i32 = 1;

// ===========================================================================
// PBKDF2 (both native and WASM)
// ===========================================================================

/// Default PBKDF2 iterations for server-side re-hash of client-derived hash.
/// Bitwarden's upstream default is 600,000, but WebCrypto rejects counts
/// above 100,000. The client-side KDF runs at whatever iteration count the
/// client chose — this value is only for the server-side re-hash.
pub const PBKDF2_SERVER_ITERATIONS: u32 = 100_000;

/// Derive a key using PBKDF2-HMAC-SHA256.
/// Native: sync via `pbkdf2` crate. WASM: async via Web Crypto.
#[cfg(not(target_family = "wasm"))]
pub fn pbkdf2_derive(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Pbkdf2Key {
    use hmac::Hmac;
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let mut output = vec![0u8; key_len as usize];
    pbkdf2_hmac::<Hmac<Sha256>>(password, salt, iterations, &mut output);
    Pbkdf2Key(Zeroizing::new(output))
}

#[cfg(not(target_family = "wasm"))]
pub fn pbkdf2_verify(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> bool {
    let computed = pbkdf2_derive(password, salt, iterations, key_len);
    constant_time_eq(&computed.0, expected)
}

#[cfg(target_family = "wasm")]
pub async fn pbkdf2_derive(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Result<Pbkdf2Key, CryptoError> {
    use js_sys::{Object, Uint8Array};
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Crypto;

    fn subtle() -> Result<web_sys::SubtleCrypto, CryptoError> {
        let global = js_sys::global();
        let crypto: Crypto = js_sys::Reflect::get(&global, &"crypto".into())
            .map_err(|_| CryptoError::CryptoUnavailable)?
            .into();
        Ok(crypto.subtle())
    }

    fn js_set(obj: &Object, key: &str, val: &JsValue) -> Result<(), CryptoError> {
        js_sys::Reflect::set(obj, &JsValue::from_str(key), val)
            .map_err(|e| CryptoError::JsError(format!("set {key}: {e:?}")))
    }

    let subtle = subtle()?;
    let password_arr = Uint8Array::from(password);

    let import_promise = subtle.import_key_with_str(
        "raw", &password_arr, "PBKDF2", false,
        &js_sys::Array::from_iter(["deriveBits".into()]),
    ).map_err(|e| CryptoError::JsError(format!("import_key: {e:?}")))?;

    let key = JsFuture::from(import_promise).await
        .map_err(|e| CryptoError::JsError(format!("import_key await: {e:?}")))?;

    let algo = Object::new();
    js_set(&algo, "name", &"PBKDF2".into())?;
    js_set(&algo, "hash", &"SHA-256".into())?;
    js_set(&algo, "salt", &Uint8Array::from(salt))?;
    js_set(&algo, "iterations", &JsValue::from(iterations))?;

    let derive_promise = subtle.derive_bits_with_object(&algo, &key.into(), key_len * 8)
        .map_err(|e| CryptoError::JsError(format!("derive_bits: {e:?}")))?;

    let result = JsFuture::from(derive_promise).await
        .map_err(|e| CryptoError::JsError(format!("derive_bits await: {e:?}")))?;

    let buf = Uint8Array::new(&result);
    Ok(Pbkdf2Key(Zeroizing::new(buf.to_vec())))
}

#[cfg(target_family = "wasm")]
pub async fn pbkdf2_verify(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> Result<bool, CryptoError> {
    let computed = pbkdf2_derive(password, salt, iterations, key_len).await?;
    Ok(constant_time_eq(&computed.0, expected))
}

// ===========================================================================
// Argon2id (native only — not available in WebCrypto)
// ===========================================================================

/// Default Argon2id parameters (matching Bitwarden defaults).
pub const ARGON2ID_MEMORY_KB: u32 = 65536;     // 64 MB
pub const ARGON2ID_PARALLELISM: u32 = 4;
pub const ARGON2ID_ITERATIONS: u32 = 3;

/// Argon2id derived key.
#[cfg(not(target_family = "wasm"))]
pub struct Argon2idKey(Zeroizing<Vec<u8>>);

#[cfg(not(target_family = "wasm"))]
impl Argon2idKey {
    pub fn as_bytes(&self) -> &[u8] { &self.0 }
    pub fn into_bytes(self) -> Vec<u8> { self.0.into() }
}

/// Derive a key using Argon2id (native only).
#[cfg(not(target_family = "wasm"))]
pub fn argon2id_derive(
    password: &[u8],
    salt: &[u8],
    iterations: u32,
    memory_kb: u32,
    parallelism: u32,
    key_len: u32,
) -> Argon2idKey {
    use argon2::{Argon2, Params, Version};

    let params = Params::new(memory_kb, iterations, parallelism, Some(key_len as usize))
        .expect("argon2 params");
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    let mut output = vec![0u8; key_len as usize];
    argon2.hash_password_into(password, salt, &mut output)
        .expect("argon2 derive");
    Argon2idKey(Zeroizing::new(output))
}

/// Verify a password against an Argon2id hash (native only).
#[cfg(not(target_family = "wasm"))]
pub fn argon2id_verify(
    password: &[u8],
    salt: &[u8],
    expected: &[u8],
    iterations: u32,
    memory_kb: u32,
    parallelism: u32,
    key_len: u32,
) -> bool {
    let computed = argon2id_derive(password, salt, iterations, memory_kb, parallelism, key_len);
    constant_time_eq(&computed.0, expected)
}

// ===========================================================================
// Password verifier — dispatches based on KDF type
// ===========================================================================

/// Result of a password derivation — zeroized key bytes.
pub struct PasswordKey(Zeroizing<Vec<u8>>);

impl PasswordKey {
    pub fn as_bytes(&self) -> &[u8] { &self.0 }
    pub fn into_bytes(self) -> Vec<u8> { self.0.into() }
}

/// PBKDF2 key (wraps the zeroized bytes).
pub struct Pbkdf2Key(Zeroizing<Vec<u8>>);

impl Pbkdf2Key {
    pub fn as_bytes(&self) -> &[u8] { &self.0 }
    pub fn into_bytes(self) -> Vec<u8> { self.0.into() }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) { diff |= x ^ y; }
    diff == 0
}

/// Crypto errors (WASM only — native operations are infallible).
#[cfg(target_family = "wasm")]
#[derive(Debug)]
pub enum CryptoError {
    JsError(String),
    CryptoUnavailable,
}

#[cfg(target_family = "wasm")]
impl core::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CryptoError::JsError(s) => write!(f, "Crypto JS error: {s}"),
            CryptoError::CryptoUnavailable => write!(f, "Web Crypto unavailable"),
        }
    }
}

#[cfg(target_family = "wasm")]
impl core::error::Error for CryptoError {}

/// Verify a password against a stored hash, dispatching by KDF type.
///
/// KDF type 0 = PBKDF2 (both native and WASM).
/// KDF type 1 = Argon2id (native only — returns error on WASM).
///
/// The Bitwarden model: client derives a master key with the chosen KDF,
/// then hashes that master key with PBKDF2 (1 iteration for server-side).
/// This function verifies the server-side hash.
#[cfg(not(target_family = "wasm"))]
pub fn verify_password(
    kdf_type: i32,
    password_hash: &[u8],
    salt: &[u8],
    stored_hash: &[u8],
    iterations: u32,
    memory_kb: Option<u32>,
    parallelism: Option<u32>,
) -> bool {
    match kdf_type {
        KDF_TYPE_PBKDF2 => {
            pbkdf2_verify(password_hash, salt, stored_hash, iterations, 32)
        }
        KDF_TYPE_ARGON2ID => {
            argon2id_verify(
                password_hash, salt, stored_hash,
                iterations,
                memory_kb.unwrap_or(ARGON2ID_MEMORY_KB),
                parallelism.unwrap_or(ARGON2ID_PARALLELISM),
                32,
            )
        }
        _ => false,
    }
}

#[cfg(target_family = "wasm")]
pub async fn verify_password(
    kdf_type: i32,
    password_hash: &[u8],
    salt: &[u8],
    stored_hash: &[u8],
    iterations: u32,
) -> Result<bool, CryptoError> {
    match kdf_type {
        KDF_TYPE_PBKDF2 => {
            pbkdf2_verify(password_hash, salt, stored_hash, iterations, 32).await
        }
        KDF_TYPE_ARGON2ID => {
            // Argon2id not available in WebCrypto — reject.
            Err(CryptoError::CryptoUnavailable)
        }
        _ => Ok(false),
    }
}
