//! The key store: load and keep key material in any backend.
//!
//! WHY: a host must load its certificate authority (CA), secret-signing and
//! server keys, or make them and keep them. The files of the first version
//! hold the private key in clear text, which the owner refused (decision 3:
//! always encrypt at rest).
//!
//! WHAT: [`KeyStore`] and [`KeyRecord`], the sealed form in [`sealing`], the
//! four stores ([`MemoryKeyStore`], [`EnvKeyStore`], [`DbKeyStore`],
//! [`FsKeyStore`]) and [`KeyMaterial`], which loads or makes a key.
//!
//! HOW: a private key is always a [`sealing::SealedBlob`]. A store never
//! writes the clear text. The associated data binds a blob to its record, so a
//! blob cannot move to another record.

pub mod db;
pub mod env;
#[cfg(not(target_family = "wasm"))]
pub mod fs;
pub mod material;
pub mod memory;
pub mod sealing;

pub use db::DbKeyStore;
pub use env::EnvKeyStore;
#[cfg(not(target_family = "wasm"))]
pub use fs::FsKeyStore;
pub use material::{KeyMaterial, KeyProtection};
pub use memory::MemoryKeyStore;
#[cfg(not(target_family = "wasm"))]
pub use sealing::PassphraseSealer;
pub use sealing::{record_aad, Argon2Params, KekSealer, KeySealer, SealFuture, SealedBlob};

use core::future::Future;
use core::pin::Pin;

use serde::{Deserialize, Serialize};

/// The future that every [`KeyStore`] call returns.
pub type KeyFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, KeyStoreError>> + Send + 'a>>;

/// The name of one record: 1 to 64 characters of `[a-z0-9._-]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyId(String);

impl KeyId {
    /// The certificate authority record.
    pub const CA: &'static str = "ca";
    /// The secret-signing key record.
    pub const SECRET_SIGNING: &'static str = "secret-signing";
    /// The server identity record.
    pub const SERVER_IDENTITY: &'static str = "server-identity";

    /// A record name.
    ///
    /// # Errors
    /// [`KeyStoreError::BadId`] when the name is empty, longer than 64
    /// characters, or holds a character outside `[a-z0-9._-]`.
    pub fn new(id: &str) -> Result<Self, KeyStoreError> {
        let valid = !id.is_empty()
            && id.len() <= 64
            && id.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            });
        if !valid {
            return Err(KeyStoreError::BadId(id.to_string()));
        }
        Ok(Self(id.to_string()))
    }

    /// The record name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// A new record name from this one with `suffix` added after a `-`.
    ///
    /// It keeps the result within the length limit: the caller's suffix is
    /// truncated when the pair is too long.
    ///
    /// # Errors
    /// [`KeyStoreError::BadId`] when the result is not a valid name.
    pub fn with_suffix(&self, suffix: &str) -> Result<Self, KeyStoreError> {
        let mut base = self.0.clone();
        base.push('-');
        let room = 64_usize.saturating_sub(base.len());
        base.extend(suffix.chars().take(room));
        Self::new(&base)
    }
}

impl core::fmt::Display for KeyId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The kind of key material in a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyKind {
    /// The key of the certificate authority (CA).
    CertificateAuthority,
    /// The key that signs secrets.
    SecretSigning,
    /// The key of the server certificate.
    ServerIdentity,
}

impl KeyKind {
    /// The name of the kind, for the associated data and for a log line.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CertificateAuthority => "certificate-authority",
            Self::SecretSigning => "secret-signing",
            Self::ServerIdentity => "server-identity",
        }
    }
}

/// The public part of a key record. It is never secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicMaterial {
    /// "ed25519", "ecdsa-p256" or "rsa-pss-sha256".
    pub algorithm: String,
    /// The public key in base64url without padding.
    pub public_key_b64url: String,
    /// The certificate in Privacy-Enhanced Mail (PEM) form. CA and server
    /// identity records only.
    pub cert_pem: Option<String>,
    /// The Domain Name System (DNS) names and Internet Protocol (IP) literals
    /// that the certificate covers. Server identity records only.
    pub subject_names: Vec<String>,
}

/// A `String` that is zeroed on drop and never printed.
#[derive(Clone)]
pub struct SecretText(zeroize::Zeroizing<String>);

impl SecretText {
    /// Wrap a secret text.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(zeroize::Zeroizing::new(text.into()))
    }

    /// The text. Treat the result as a secret.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Debug for SecretText {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Serialize for SecretText {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SecretText {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(Self::new(text))
    }
}

/// How the private key of a record is kept.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "protection", rename_all = "kebab-case")]
pub enum StoredPrivateKey {
    /// Encrypted with a key encryption key (KEK). The normal case.
    Sealed(SealedBlob),
    /// PKCS#8 Privacy-Enhanced Mail (PEM) in clear text. **Read only**, for the
    /// legacy files of the first version. No store writes it. The host seals
    /// it at the first start with a KEK, then removes the file.
    Plaintext {
        /// The key text.
        pkcs8_pem: SecretText,
    },
    /// The key lives outside the store (a Worker secret, an environment
    /// variable). The record keeps only the name of that place.
    External {
        /// The name of the place that holds the key.
        reference: String,
    },
    /// No private key: a verify-only record.
    Absent,
}

/// One stored item of key material.
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyRecord {
    /// The record name.
    pub id: KeyId,
    /// The kind of key.
    pub kind: KeyKind,
    /// Starts at 1. Each replacement adds 1.
    pub revision: u64,
    /// The wall-clock time when the record was made, in seconds since the
    /// UNIX epoch.
    pub created_at_unix: u64,
    /// The certificate expiry in seconds since the UNIX epoch. Certificates
    /// only.
    pub not_after_unix: Option<u64>,
    /// The public part.
    pub public: PublicMaterial,
    /// The private part.
    pub private: StoredPrivateKey,
}

impl KeyRecord {
    /// The key ID (`kid`) of the public key: the first 16 hexadecimal
    /// characters of its SHA-256.
    #[must_use]
    pub fn kid(&self) -> String {
        let digest = crate::fingerprint::sha256_hex(&self.public_key_bytes());
        digest.chars().take(16).collect()
    }

    /// The public key bytes, decoded from base64url.
    ///
    /// A record that a store read has a public key in base64url, so this
    /// decode fails only for a corrupt record. It returns empty bytes then.
    #[must_use]
    pub fn public_key_bytes(&self) -> Vec<u8> {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(self.public.public_key_b64url.trim())
            .unwrap_or_default()
    }

    /// The associated data of the sealed private part.
    #[must_use]
    pub fn aad(&self) -> Vec<u8> {
        record_aad(&self.id, self.kind, self.revision)
    }
}

/// Refuse a write of a clear-text private key (owner decision 3).
///
/// Every writable store calls this before it writes a record. A legacy record
/// in clear text is read-only input; it is never written again.
///
/// # Errors
/// [`KeyStoreError::PlaintextRefused`] for a [`StoredPrivateKey::Plaintext`]
/// private part.
pub fn ensure_sealed(record: &KeyRecord, store: &'static str) -> Result<(), KeyStoreError> {
    match &record.private {
        StoredPrivateKey::Plaintext { .. } => Err(KeyStoreError::PlaintextRefused(store)),
        StoredPrivateKey::Sealed(_)
        | StoredPrivateKey::External { .. }
        | StoredPrivateKey::Absent => Ok(()),
    }
}

/// The result of [`KeyStore::create`].
///
/// The large variant is the winner record that a losing writer must use, so it
/// travels whole rather than behind another allocation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum CreateOutcome {
    /// This call made the record.
    Created,
    /// Another writer made the record first. Use theirs.
    AlreadyExists(KeyRecord),
}

impl core::fmt::Debug for CreateOutcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Created => f.write_str("Created"),
            Self::AlreadyExists(record) => {
                f.debug_tuple("AlreadyExists").field(&record.id).finish()
            }
        }
    }
}

/// Facts about a store, so a caller can refuse an unsafe setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStoreInfo {
    /// The store name, for a log line.
    pub name: &'static str,
    /// `create` is atomic across processes ("first writer wins").
    pub atomic_create: bool,
    /// Other processes can write the same records.
    pub shared: bool,
    /// The store accepts writes at all.
    pub writable: bool,
}

/// Loads and keeps key material in any backend.
pub trait KeyStore: Send + Sync + 'static {
    /// The record, or `None`.
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>>;

    /// Store `record` only if no record with its ID exists. The check and the
    /// write are one atomic step when [`KeyStoreInfo::atomic_create`] is true.
    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome>;

    /// Replace the record when the stored revision is `expected_revision`.
    /// Rotation is an operator action; it does not race with `create`.
    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()>;

    /// Every record of `kind`. Rotation uses it to keep the old public keys.
    ///
    /// The default returns [`KeyStoreError::Unsupported`]; a store that can
    /// list returns its records.
    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        let _ = kind;
        Box::pin(async {
            Err(KeyStoreError::Unsupported(
                "the store cannot list records".to_string(),
            ))
        })
    }

    /// What this store can do.
    fn info(&self) -> KeyStoreInfo;
}

/// Every way a key store call can fail.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum KeyStoreError {
    /// The record name is not valid.
    #[display("bad key id {_0:?}")]
    BadId(#[error(not(source))] String),
    /// There is no record with this name.
    #[display("no key material named {_0}")]
    NotFound(#[error(not(source))] String),
    /// The stored record is not valid. It is never overwritten.
    #[display("the stored record {_0} is corrupt; it is not overwritten")]
    Corrupt(#[error(not(source))] String),
    /// The store accepts no writes.
    #[display("the store is read-only")]
    ReadOnly,
    /// The store cannot create a record atomically, and it is shared.
    #[display("this backend cannot create records atomically")]
    NotAtomic,
    /// The stored revision is not the expected one.
    #[display("revision conflict for {_0}")]
    Conflict(#[error(not(source))] String),
    /// A clear-text private key is not allowed.
    #[display("plain-text private keys are not allowed in {_0}")]
    PlaintextRefused(#[error(not(source))] &'static str),
    /// Sealing or opening a private key failed.
    #[display("sealing failed: {_0}")]
    Seal(#[error(not(source))] String),
    /// The storage backend failed.
    #[display("storage error: {_0}")]
    Storage(#[error(not(source))] String),
    /// The operation is not possible on this store.
    #[display("unsupported key store operation: {_0}")]
    Unsupported(#[error(not(source))] String),
    /// The key encryption key (KEK) is not available.
    #[display("the key encryption key (KEK) is unavailable: {_0}")]
    KekUnavailable(#[error(not(source))] String),
}
