//! PBKDF2-HMAC-SHA256 key derivation — shared between native and WASM backends.
//!
//! WHY: password verification in Bitwarden's model (client sends a
//! client-KDF-derived hash; server re-hashes it with PBKDF2 and compares).
//! Also used for send password hashing. This is a general auth primitive, not
//! Bitwarden-specific.
//!
//! WHAT: `pbkdf2_derive` (key derivation) and `pbkdf2_verify` (constant-time
//! comparison).
//!
//! HOW: Native uses the `pbkdf2` + `hmac` + `sha2` crates. WASM uses the Web
//! Crypto API via `web-sys` (gated on `wasm-pbkdf2` feature).

use zeroize::Zeroizing;

// ===========================================================================
// Native: pbkdf2 + hmac + sha2 crates
// ===========================================================================

#[cfg(not(target_family = "wasm"))]
fn derive_bytes_native(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Vec<u8> {
    use hmac::Hmac;
    use pbkdf2::pbkdf2_hmac;
    use sha2::Sha256;

    let mut output = vec![0u8; key_len as usize];
    pbkdf2_hmac::<Hmac<Sha256>>(password, salt, iterations, &mut output);
    output
}

#[cfg(not(target_family = "wasm"))]
fn verify_native(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> bool {
    let computed = derive_bytes_native(password, salt, iterations, key_len);
    constant_time_eq(&computed, expected)
}

// ===========================================================================
// WASM: Web Crypto API via web-sys (gated on wasm-pbkdf2 feature)
// ===========================================================================

#[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
async fn derive_bytes_wasm(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Result<Vec<u8>, Pbkdf2Error> {
    use js_sys::{Object, Uint8Array};
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Crypto;

    fn subtle() -> Result<web_sys::SubtleCrypto, Pbkdf2Error> {
        let global = js_sys::global();
        let crypto: Crypto = js_sys::Reflect::get(&global, &"crypto".into())
            .map_err(|_| Pbkdf2Error::CryptoUnavailable)?
            .into();
        Ok(crypto.subtle())
    }

    fn js_set(obj: &Object, key: &str, val: &JsValue) -> Result<(), Pbkdf2Error> {
        js_sys::Reflect::set(obj, &JsValue::from_str(key), val)
            .map_err(|e| Pbkdf2Error::JsError(format!("set {key} failed: {e:?}")))
    }

    let subtle = subtle()?;
    let password_arr = Uint8Array::from(password);

    let import_promise = subtle.import_key_with_str(
        "raw",
        &password_arr,
        "PBKDF2",
        false,
        &js_sys::Array::from_iter(["deriveBits".into()]),
    ).map_err(|e| Pbkdf2Error::JsError(format!("import_key: {e:?}")))?;

    let key = JsFuture::from(import_promise).await
        .map_err(|e| Pbkdf2Error::JsError(format!("import_key await: {e:?}")))?;

    let algo = Object::new();
    js_set(&algo, "name", &"PBKDF2".into())?;
    js_set(&algo, "hash", &"SHA-256".into())?;
    js_set(&algo, "salt", &Uint8Array::from(salt))?;
    js_set(&algo, "iterations", &JsValue::from(iterations))?;

    let derive_promise = subtle.derive_bits_with_object(&algo, &key.into(), key_len * 8)
        .map_err(|e| Pbkdf2Error::JsError(format!("derive_bits: {e:?}")))?;

    let result = JsFuture::from(derive_promise).await
        .map_err(|e| Pbkdf2Error::JsError(format!("derive_bits await: {e:?}")))?;

    let buf = Uint8Array::new(&result);
    Ok(buf.to_vec())
}

#[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
async fn verify_wasm(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> Result<bool, Pbkdf2Error> {
    let computed = derive_bytes_wasm(password, salt, iterations, key_len).await?;
    Ok(constant_time_eq(&computed, expected))
}

// ===========================================================================
// Shared
// ===========================================================================

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// PBKDF2-HMAC-SHA256 key derivation result, zeroized on drop.
pub struct Pbkdf2Key(Zeroizing<Vec<u8>>);

impl Pbkdf2Key {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into()
    }
}

/// PBKDF2 errors.
#[derive(Debug)]
pub enum Pbkdf2Error {
    #[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
    JsError(String),
    #[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
    CryptoUnavailable,
}

impl core::fmt::Display for Pbkdf2Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            #[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
            Pbkdf2Error::JsError(s) => write!(f, "PBKDF2 JS error: {s}"),
            #[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
            Pbkdf2Error::CryptoUnavailable => write!(f, "Web Crypto unavailable"),
        }
    }
}

impl core::error::Error for Pbkdf2Error {}

/// Default iteration count for server-side password verification.
/// Bitwarden's upstream default is 600,000, but WebCrypto rejects counts
/// above 100,000. The client-side KDF (which derives the actual vault key)
/// runs at whatever iteration count the client chose — this value is only
/// for the server-side re-hash of the client-derived hash.
pub const SERVER_PASSWORD_ITERATIONS: u32 = 100_000;

// ===========================================================================
// Public API — native (sync)
// ===========================================================================

/// Derive a key using PBKDF2-HMAC-SHA256.
#[cfg(not(target_family = "wasm"))]
pub fn pbkdf2_derive(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Pbkdf2Key {
    Pbkdf2Key(Zeroizing::new(derive_bytes_native(password, salt, iterations, key_len)))
}

/// Verify a password against a stored hash using PBKDF2-HMAC-SHA256.
/// Uses constant-time comparison.
#[cfg(not(target_family = "wasm"))]
pub fn pbkdf2_verify(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> bool {
    verify_native(password, salt, expected, iterations, key_len)
}

// ===========================================================================
// Public API — WASM (async, requires wasm-pbkdf2 feature)
// ===========================================================================

/// Derive a key using PBKDF2-HMAC-SHA256 (WASM, async via Web Crypto).
#[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
pub async fn pbkdf2_derive(password: &[u8], salt: &[u8], iterations: u32, key_len: u32) -> Result<Pbkdf2Key, Pbkdf2Error> {
    derive_bytes_wasm(password, salt, iterations, key_len).await.map(Pbkdf2Key)
}

/// Verify a password against a stored hash using PBKDF2-HMAC-SHA256 (WASM, async).
/// Uses constant-time comparison.
#[cfg(all(target_family = "wasm", feature = "wasm-pbkdf2"))]
pub async fn pbkdf2_verify(password: &[u8], salt: &[u8], expected: &[u8], iterations: u32, key_len: u32) -> Result<bool, Pbkdf2Error> {
    verify_wasm(password, salt, expected, iterations, key_len).await
}
