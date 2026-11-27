//! Sealing: encrypt and decrypt a private key with a key encryption key (KEK).
//!
//! WHY: a private key must never sit in a store in clear text (owner decision
//! 3). Sealing is authenticated encryption (AEAD) with a KEK. The associated
//! data signs the record name, its kind and its revision, so a sealed key
//! cannot move to another record.
//!
//! WHAT: [`KeySealer`] (async), [`SealedBlob`], [`KekSealer`],
//! [`PassphraseSealer`], [`Argon2Params`] and [`record_aad`].
//!
//! HOW: the AEAD is ChaCha20-Poly1305 through `foundation_compact`. A nonce
//! comes from the injected entropy source, so a deterministic simulation run
//! with the same seed seals the same bytes. A passphrase first becomes a key
//! with Argon2id.

use std::future::Future;
use std::pin::Pin;

use base64::Engine as _;
use foundation_compact::crypto::chacha::{open_with, seal_with, ChaChaKey};
use foundation_compact::crypto::SecureBytes;
use foundation_context::env::{EntropySourceExt as _, Env};
use serde::{Deserialize, Serialize};
#[cfg(not(target_family = "wasm"))]
use zeroize::Zeroizing;

use super::{KeyId, KeyKind, KeyStoreError};

/// A boxed future that every [`KeySealer`] call returns.
pub type SealFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, KeyStoreError>> + Send + 'a>>;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Encrypts and decrypts private keys with one KEK.
pub trait KeySealer: Send + Sync + 'static {
    /// The name of the KEK. It is stored with each blob, so a blob names the
    /// KEK that can open it (needed for rotation).
    fn kek_id(&self) -> &str;

    /// Encrypt `plaintext`. `aad` is the associated data: the AEAD signs it
    /// but does not store it, so a blob opens only with the same `aad`.
    fn seal<'a>(&'a self, aad: &'a [u8], plaintext: &'a [u8]) -> SealFuture<'a, SealedBlob>;

    /// Decrypt a blob. An error if the KEK, the `aad` or the blob is wrong.
    fn open<'a>(&'a self, aad: &'a [u8], sealed: &'a SealedBlob) -> SealFuture<'a, SecureBytes>;
}

/// The stored form of a sealed key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBlob {
    /// Format version: 1.
    pub v: u32,
    /// "chacha20poly1305", "argon2id+chacha20poly1305", "aes256gcm", or
    /// "kms-envelope+chacha20poly1305".
    pub alg: String,
    /// The KEK that can open this blob.
    pub kek_id: String,
    /// The 12-byte nonce in base64url without padding.
    pub nonce_b64url: String,
    /// The ciphertext and its tag in base64url without padding.
    pub ciphertext_b64url: String,
    /// `argon2id+…` only: the 16-byte salt in base64url without padding.
    pub salt_b64url: Option<String>,
    /// `argon2id+…` only: the derivation cost.
    pub argon2: Option<Argon2Params>,
    /// `kms-envelope+…` only: the data encryption key (DEK), encrypted by the
    /// key management service (KMS).
    pub wrapped_dek_b64url: Option<String>,
}

impl SealedBlob {
    /// The nonce bytes.
    ///
    /// # Errors
    /// [`KeyStoreError::Seal`] when the stored nonce is not 12 bytes of
    /// base64url.
    fn nonce(&self) -> Result<[u8; 12], KeyStoreError> {
        let bytes = B64
            .decode(self.nonce_b64url.trim())
            .map_err(|e| KeyStoreError::Seal(format!("stored nonce is not base64url: {e}")))?;
        bytes
            .try_into()
            .map_err(|_| KeyStoreError::Seal("the stored nonce is not 12 bytes".to_string()))
    }

    /// The ciphertext bytes.
    ///
    /// # Errors
    /// [`KeyStoreError::Seal`] when the ciphertext is not base64url.
    fn ciphertext(&self) -> Result<Vec<u8>, KeyStoreError> {
        B64.decode(self.ciphertext_b64url.trim()).map_err(|e| {
            KeyStoreError::Seal(format!("the stored ciphertext is not base64url: {e}"))
        })
    }
}

/// A 32-byte KEK from a file, an environment variable or a Worker secret.
pub struct KekSealer {
    kek_id: String,
    key: ChaChaKey,
    env: Env,
}

impl std::fmt::Debug for KekSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KekSealer")
            .field("kek_id", &self.kek_id)
            .finish_non_exhaustive()
    }
}

impl KekSealer {
    /// A sealer over a 32-byte key.
    #[must_use]
    pub fn new(kek_id: impl Into<String>, key: [u8; 32], env: Env) -> Self {
        Self {
            kek_id: kek_id.into(),
            key: ChaChaKey::from_bytes(key),
            env,
        }
    }

    /// A random nonce from the injected entropy source.
    fn nonce(&self) -> Result<[u8; 12], KeyStoreError> {
        self.env
            .entropy()
            .fill_new::<12>()
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))
    }
}

impl KeySealer for KekSealer {
    fn kek_id(&self) -> &str {
        &self.kek_id
    }

    fn seal<'a>(&'a self, aad: &'a [u8], plaintext: &'a [u8]) -> SealFuture<'a, SealedBlob> {
        Box::pin(async move {
            let nonce = self.nonce()?;
            let sealed =
                seal_with(&self.key, nonce, aad, plaintext).map_err(KeyStoreError::Seal)?;
            Ok(SealedBlob {
                v: 1,
                alg: "chacha20poly1305".to_string(),
                kek_id: self.kek_id.clone(),
                nonce_b64url: B64.encode(nonce),
                ciphertext_b64url: B64.encode(&sealed[12..]),
                salt_b64url: None,
                argon2: None,
                wrapped_dek_b64url: None,
            })
        })
    }

    fn open<'a>(&'a self, aad: &'a [u8], sealed: &'a SealedBlob) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            require_alg(sealed, "chacha20poly1305")?;
            if sealed.kek_id != self.kek_id {
                return Err(KeyStoreError::Seal(format!(
                    "the blob names KEK {} but this sealer is {}",
                    sealed.kek_id, self.kek_id
                )));
            }
            let nonce = sealed.nonce()?;
            let ciphertext = sealed.ciphertext()?;
            open_with(&self.key, &nonce, aad, &ciphertext).map_err(KeyStoreError::Seal)
        })
    }
}

/// The Argon2id cost. Default: 19 MiB of memory, 2 passes and 1 lane, the
/// minimum of the OWASP Password Storage Cheat Sheet for Argon2id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2Params {
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Pass count.
    pub iterations: u32,
    /// Parallelism, in lanes.
    pub lanes: u32,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_kib: 19 * 1024,
            iterations: 2,
            lanes: 1,
        }
    }
}

/// A KEK derived from a passphrase with Argon2id, one salt per blob.
#[cfg(not(target_family = "wasm"))]
pub struct PassphraseSealer {
    kek_id: String,
    passphrase: Zeroizing<String>,
    params: Argon2Params,
    env: Env,
}

#[cfg(not(target_family = "wasm"))]
impl std::fmt::Debug for PassphraseSealer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PassphraseSealer")
            .field("kek_id", &self.kek_id)
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_family = "wasm"))]
impl PassphraseSealer {
    /// A sealer over a passphrase.
    #[must_use]
    pub fn new(
        kek_id: impl Into<String>,
        passphrase: Zeroizing<String>,
        params: Argon2Params,
        env: Env,
    ) -> Self {
        Self {
            kek_id: kek_id.into(),
            passphrase,
            params,
            env,
        }
    }

    /// A random salt of 16 bytes.
    fn salt(&self) -> Result<[u8; 16], KeyStoreError> {
        self.env
            .entropy()
            .fill_new::<16>()
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))
    }

    fn nonce(&self) -> Result<[u8; 12], KeyStoreError> {
        self.env
            .entropy()
            .fill_new::<12>()
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))
    }
}

#[cfg(not(target_family = "wasm"))]
impl KeySealer for PassphraseSealer {
    fn kek_id(&self) -> &str {
        &self.kek_id
    }

    fn seal<'a>(&'a self, aad: &'a [u8], plaintext: &'a [u8]) -> SealFuture<'a, SealedBlob> {
        Box::pin(async move {
            let salt = self.salt()?;
            let key = derive_key(&self.passphrase, &salt, self.params)?;
            let nonce = self.nonce()?;
            let sealed = seal_with(&key, nonce, aad, plaintext).map_err(KeyStoreError::Seal)?;
            Ok(SealedBlob {
                v: 1,
                alg: "argon2id+chacha20poly1305".to_string(),
                kek_id: self.kek_id.clone(),
                nonce_b64url: B64.encode(nonce),
                ciphertext_b64url: B64.encode(&sealed[12..]),
                salt_b64url: Some(B64.encode(salt)),
                argon2: Some(self.params),
                wrapped_dek_b64url: None,
            })
        })
    }

    fn open<'a>(&'a self, aad: &'a [u8], sealed: &'a SealedBlob) -> SealFuture<'a, SecureBytes> {
        Box::pin(async move {
            require_alg(sealed, "argon2id+chacha20poly1305")?;
            if sealed.kek_id != self.kek_id {
                return Err(KeyStoreError::Seal(format!(
                    "the blob names KEK {} but this sealer is {}",
                    sealed.kek_id, self.kek_id
                )));
            }
            let salt = sealed
                .salt_b64url
                .as_deref()
                .ok_or_else(|| KeyStoreError::Seal("the blob has no Argon2 salt".to_string()))?;
            let salt: [u8; 16] = B64
                .decode(salt.trim())
                .map_err(|e| KeyStoreError::Seal(format!("stored salt is not base64url: {e}")))?
                .try_into()
                .map_err(|_| KeyStoreError::Seal("the stored salt is not 16 bytes".to_string()))?;
            let params = sealed
                .argon2
                .ok_or_else(|| KeyStoreError::Seal("the blob has no Argon2 cost".to_string()))?;
            let key = derive_key(&self.passphrase, &salt, params)?;
            let nonce = sealed.nonce()?;
            let ciphertext = sealed.ciphertext()?;
            open_with(&key, &nonce, aad, &ciphertext).map_err(KeyStoreError::Seal)
        })
    }
}

/// Derive a 32-byte key from a passphrase with Argon2id.
///
/// The call is central processing unit (CPU) and memory heavy, so it runs only
/// at a host start or an operator command, never on a request path.
#[cfg(not(target_family = "wasm"))]
fn derive_key(
    passphrase: &str,
    salt: &[u8; 16],
    params: Argon2Params,
) -> Result<ChaChaKey, KeyStoreError> {
    use argon2::{Algorithm, Argon2, Params, Version};

    let params = Params::new(params.memory_kib, params.iterations, params.lanes, Some(32))
        .map_err(|e| KeyStoreError::Seal(format!("Argon2 cost: {e}")))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut *key)
        .map_err(|e| KeyStoreError::Seal(format!("Argon2id: {e}")))?;
    Ok(ChaChaKey::from_bytes(*key))
}

/// Check that a blob names `alg`.
fn require_alg(sealed: &SealedBlob, alg: &str) -> Result<(), KeyStoreError> {
    if sealed.alg != alg {
        return Err(KeyStoreError::Seal(format!(
            "this sealer opens {alg} blobs, not {}",
            sealed.alg
        )));
    }
    if sealed.v != 1 {
        return Err(KeyStoreError::Seal(format!(
            "unknown sealed blob version {}",
            sealed.v
        )));
    }
    Ok(())
}

/// The associated data for one record. It binds the blob to the record ID,
/// kind and revision.
#[must_use]
pub fn record_aad(id: &KeyId, kind: KeyKind, revision: u64) -> Vec<u8> {
    let mut aad = b"ewe.keystore.v1\0".to_vec();
    aad.extend_from_slice(id.as_str().as_bytes());
    aad.push(0);
    aad.extend_from_slice(kind.as_str().as_bytes());
    aad.push(0);
    aad.extend_from_slice(&revision.to_be_bytes());
    aad
}
