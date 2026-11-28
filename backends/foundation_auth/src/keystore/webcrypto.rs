//! The Worker key encryption key (KEK): WebCrypto and the Worker secret
//! bindings.
//!
//! WHY: a Worker gets the raw KEK bytes from a Secrets Store secret or a Worker
//! secret. Cloudflare has no key management service that keeps the key in a
//! hardware module. The Worker therefore imports the bytes one time into a
//! non-extractable WebCrypto `CryptoKey` and drops the bytes, so a bug that
//! logs memory cannot log the key.
//!
//! WHAT: [`WebCryptoSealer`], [`SecretsStoreKek`], [`WorkerSecretKek`] and the
//! [`WorkerKekReader`] seam that the Worker host fills.
//!
//! HOW: `crypto.subtle.encrypt` and `decrypt` with AES-256-GCM. The code does
//! not use `wrapKey`: the Workers documentation lists that only for AES-CTR,
//! AES-CBC and RSA-OAEP.
//!
//! This module builds only for wasm with the `wasm-kek` feature.

#![cfg(all(target_family = "wasm", feature = "wasm-kek"))]

use std::sync::Arc;

use base64::Engine as _;
use foundation_compact::crypto::SecureBytes;
use foundation_compact::SendWrapper;
use foundation_context::env::Env;
use wasm_bindgen::JsCast as _;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CryptoKey, SubtleCrypto};

use super::kek::{KekSource, KekSourceInfo};
use super::sealing::{SealFuture, SealedBlob};
use super::{KeySealer, KeyStoreError};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// A sealer whose key lives inside a non-extractable WebCrypto `CryptoKey`.
pub struct WebCryptoSealer {
    kek_id: String,
    key: CryptoKey,
    env: Env,
}

impl std::fmt::Debug for WebCryptoSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebCryptoSealer")
            .field("kek_id", &self.kek_id)
            .finish_non_exhaustive()
    }
}

impl WebCryptoSealer {
    /// Import 32 raw bytes one time. The bytes are zeroed after the import.
    ///
    /// # Errors
    /// [`KeyStoreError::Seal`] when the import fails.
    pub async fn import(kek_id: String, raw: SecureBytes, env: Env) -> Result<Self, KeyStoreError> {
        if raw.len() != 32 {
            return Err(KeyStoreError::Seal(
                "a WebCrypto KEK must be 32 bytes".to_string(),
            ));
        }
        let subtle = subtle()?;
        let algorithm = aes_gcm_algorithm(None, &[])?;
        let usages = js_sys::Array::new();
        usages.push(&wasm_bindgen::JsValue::from_str("encrypt"));
        usages.push(&wasm_bindgen::JsValue::from_str("decrypt"));
        let data = js_sys::Uint8Array::from(raw.as_slice());
        let promise = subtle
            .import_key_with_object(
                "raw",
                data.unchecked_ref::<js_sys::Object>(),
                &algorithm,
                false,
                &usages,
            )
            .map_err(|e| KeyStoreError::Seal(format!("importing the KEK: {e:?}")))?;
        let key = JsFuture::from(promise)
            .await
            .map_err(|e| KeyStoreError::Seal(format!("importing the KEK: {e:?}")))?
            .dyn_into::<CryptoKey>()
            .map_err(|_| {
                KeyStoreError::Seal("the imported value is not a CryptoKey".to_string())
            })?;
        // The JavaScript array holds a copy; dropping the Rust buffer zeroes
        // it.
        drop(raw);
        Ok(Self { kek_id, key, env })
    }

    /// The non-extractable `CryptoKey` that holds the key encryption key (KEK).
    ///
    /// A caller can pass it to another `crypto.subtle` operation. The browser
    /// refuses `exportKey` on this key, because the import marked it
    /// non-extractable.
    #[must_use]
    pub fn crypto_key(&self) -> &CryptoKey {
        &self.key
    }

    /// A 12-byte nonce from the injected entropy source.
    fn nonce(&self) -> Result<[u8; 12], KeyStoreError> {
        let mut nonce = [0u8; 12];
        self.env
            .entropy()
            .fill(&mut nonce)
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))?;
        Ok(nonce)
    }
}

impl KeySealer for WebCryptoSealer {
    fn kek_id(&self) -> &str {
        &self.kek_id
    }

    fn seal<'a>(&'a self, aad: &'a [u8], plaintext: &'a [u8]) -> SealFuture<'a, SealedBlob> {
        Box::pin(SendWrapper::new(async move {
            let nonce = self.nonce()?;
            let subtle = subtle()?;
            let algorithm = aes_gcm_algorithm(Some(&nonce), aad)?;
            let data = js_sys::Uint8Array::from(plaintext);
            let promise = subtle
                .encrypt_with_object_and_buffer_source(
                    &algorithm,
                    &self.key,
                    data.unchecked_ref::<js_sys::Object>(),
                )
                .map_err(|e| KeyStoreError::Seal(format!("encrypt: {e:?}")))?;
            let ciphertext = JsFuture::from(promise)
                .await
                .map_err(|e| KeyStoreError::Seal(format!("encrypt: {e:?}")))?;
            let ciphertext = js_sys::Uint8Array::new(&ciphertext).to_vec();
            Ok(SealedBlob {
                v: 1,
                alg: "aes256gcm".to_string(),
                kek_id: self.kek_id.clone(),
                nonce_b64url: B64.encode(nonce),
                ciphertext_b64url: B64.encode(ciphertext),
                salt_b64url: None,
                argon2: None,
                wrapped_dek_b64url: None,
            })
        }))
    }

    fn open<'a>(&'a self, aad: &'a [u8], sealed: &'a SealedBlob) -> SealFuture<'a, SecureBytes> {
        Box::pin(SendWrapper::new(async move {
            if sealed.alg != "aes256gcm" || sealed.kek_id != self.kek_id {
                return Err(KeyStoreError::Seal(
                    "the blob does not belong to this KEK".to_string(),
                ));
            }
            let nonce = B64
                .decode(sealed.nonce_b64url.trim())
                .map_err(|e| KeyStoreError::Seal(format!("nonce: {e}")))?;
            let ciphertext = B64
                .decode(sealed.ciphertext_b64url.trim())
                .map_err(|e| KeyStoreError::Seal(format!("ciphertext: {e}")))?;
            let subtle = subtle()?;
            let algorithm = aes_gcm_algorithm(Some(&nonce), aad)?;
            let data = js_sys::Uint8Array::from(ciphertext.as_slice());
            let promise = subtle
                .decrypt_with_object_and_buffer_source(
                    &algorithm,
                    &self.key,
                    data.unchecked_ref::<js_sys::Object>(),
                )
                .map_err(|e| KeyStoreError::Seal(format!("decrypt: {e:?}")))?;
            let plaintext = JsFuture::from(promise)
                .await
                .map_err(|e| KeyStoreError::Seal(format!("decrypt: {e:?}")))?;
            Ok(SecureBytes::new(
                js_sys::Uint8Array::new(&plaintext).to_vec(),
            ))
        }))
    }
}

/// The AES-256-GCM parameters, with the nonce as the initialization vector and
/// the record bytes as the additional authenticated data.
fn aes_gcm_algorithm(nonce: Option<&[u8]>, aad: &[u8]) -> Result<js_sys::Object, KeyStoreError> {
    let algorithm = js_sys::Object::new();
    js_sys::Reflect::set(
        &algorithm,
        &wasm_bindgen::JsValue::from_str("name"),
        &wasm_bindgen::JsValue::from_str("AES-GCM"),
    )
    .map_err(|e| KeyStoreError::Seal(format!("AES-GCM parameters: {e:?}")))?;
    if let Some(nonce) = nonce {
        js_sys::Reflect::set(
            &algorithm,
            &wasm_bindgen::JsValue::from_str("iv"),
            &js_sys::Uint8Array::from(nonce),
        )
        .map_err(|e| KeyStoreError::Seal(format!("AES-GCM nonce: {e:?}")))?;
        js_sys::Reflect::set(
            &algorithm,
            &wasm_bindgen::JsValue::from_str("additionalData"),
            &js_sys::Uint8Array::from(aad),
        )
        .map_err(|e| KeyStoreError::Seal(format!("AES-GCM associated data: {e:?}")))?;
    }
    Ok(algorithm)
}

/// The `crypto.subtle` handle of the current Worker or page.
fn subtle() -> Result<SubtleCrypto, KeyStoreError> {
    let global = js_sys::global();
    if let Ok(worker) = global.clone().dyn_into::<web_sys::WorkerGlobalScope>() {
        return Ok(worker.crypto().map_err(js_error)?.subtle());
    }
    if let Ok(window) = global.dyn_into::<web_sys::Window>() {
        return Ok(window.crypto().map_err(js_error)?.subtle());
    }
    Err(KeyStoreError::KekUnavailable(
        "no WebCrypto in this JavaScript environment".to_string(),
    ))
}

fn js_error(error: wasm_bindgen::JsValue) -> KeyStoreError {
    KeyStoreError::Seal(format!("WebCrypto: {error:?}"))
}

/// The Worker host's read of a Secrets Store secret or a Worker secret.
///
/// The Worker build fills it with `await env.<binding>.get()` or with the value
/// of a Worker secret. This crate does not depend on the Worker runtime, so the
/// host passes the seam in.
pub trait WorkerKekReader: Send + Sync + 'static {
    /// The secret value, or `None` when the binding is absent or empty.
    fn get<'a>(&'a self, name: &'a str) -> SealFuture<'a, Option<SecureBytes>>;
}

/// A KEK in a Cloudflare Secrets Store secret.
pub struct SecretsStoreKek<R: WorkerKekReader> {
    /// The binding name, for example `AGENT_RPC_KEK`.
    pub binding: String,
    /// The secret name inside the store.
    pub secret_name: String,
    /// The KEK ID.
    pub kek_id: String,
    reader: Arc<R>,
}

impl<R: WorkerKekReader> std::fmt::Debug for SecretsStoreKek<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretsStoreKek")
            .field("binding", &self.binding)
            .finish_non_exhaustive()
    }
}

impl<R: WorkerKekReader> SecretsStoreKek<R> {
    /// A Secrets Store secret `secret_name` behind `binding`.
    #[must_use]
    pub fn new(binding: impl Into<String>, secret_name: impl Into<String>, reader: Arc<R>) -> Self {
        Self {
            binding: binding.into(),
            secret_name: secret_name.into(),
            kek_id: "secrets-store".to_string(),
            reader,
        }
    }
}

impl<R: WorkerKekReader> KekSource for SecretsStoreKek<R> {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "secrets-store",
            location: self.binding.clone(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        // `WebCryptoSealer::import` waits on a JavaScript promise, which is not
        // `Send`. The wrapper asserts `Send` on single-threaded wasm.
        Box::pin(SendWrapper::new(async move {
            let value = self.reader.get(&self.secret_name).await?.ok_or_else(|| {
                KeyStoreError::KekUnavailable(format!(
                    "Secrets Store binding {} has no secret {}",
                    self.binding, self.secret_name
                ))
            })?;
            let raw = decode_kek(value.as_slice())?;
            let sealer = WebCryptoSealer::import(self.kek_id.clone(), raw, env.clone()).await?;
            Ok(Arc::new(sealer) as Arc<dyn KeySealer>)
        }))
    }
}

/// A KEK in a Worker secret (`wrangler secret put <name>`).
pub struct WorkerSecretKek<R: WorkerKekReader> {
    /// The secret name.
    pub name: String,
    /// The KEK ID.
    pub kek_id: String,
    reader: Arc<R>,
}

impl<R: WorkerKekReader> std::fmt::Debug for WorkerSecretKek<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerSecretKek")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl<R: WorkerKekReader> WorkerSecretKek<R> {
    /// A Worker secret `name`.
    #[must_use]
    pub fn new(name: impl Into<String>, reader: Arc<R>) -> Self {
        Self {
            name: name.into(),
            kek_id: "worker-secret".to_string(),
            reader,
        }
    }
}

impl<R: WorkerKekReader> KekSource for WorkerSecretKek<R> {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "worker-secret",
            location: self.name.clone(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(SendWrapper::new(async move {
            let value = self.reader.get(&self.name).await?.ok_or_else(|| {
                KeyStoreError::KekUnavailable(format!("Worker secret {} is not bound", self.name))
            })?;
            let raw = decode_kek(value.as_slice())?;
            let sealer = WebCryptoSealer::import(self.kek_id.clone(), raw, env.clone()).await?;
            Ok(Arc::new(sealer) as Arc<dyn KeySealer>)
        }))
    }
}

/// The 32 bytes of a base64url secret.
fn decode_kek(value: &[u8]) -> Result<SecureBytes, KeyStoreError> {
    let text = core::str::from_utf8(value)
        .map_err(|_| KeyStoreError::KekUnavailable("the secret is not UTF-8".to_string()))?;
    let bytes = B64
        .decode(text.trim())
        .map_err(|e| KeyStoreError::KekUnavailable(format!("the secret is not base64url: {e}")))?;
    if bytes.len() != 32 {
        return Err(KeyStoreError::KekUnavailable(
            "the secret is not 32 bytes".to_string(),
        ));
    }
    Ok(SecureBytes::new(bytes))
}
