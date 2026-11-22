//! Operator key files: where a private key comes from, how it is parsed, and
//! how it becomes a signing key.
//!
//! WHY: the proof of concept lets the operator make each private key with a
//! standard tool (`ssh-keygen` or `openssl`). The host loads the key and uses
//! it. The host does not make, seal, store or rotate private keys.
//!
//! WHAT: [`KeySource`] (a file over the host file system, or an inline value),
//! [`PrivateKey`] (OpenSSH, PKCS#8 and SEC1 PEM, all turned into PKCS#8 DER),
//! and [`KeyLoadError`].
//!
//! HOW: every input format becomes one PKCS#8 DER form, so the signing and
//! certificate code does not know the input format. Key bytes are zeroed on
//! drop and never printed.

mod parse;
mod source;

pub use source::KeySource;

use foundation_compact::crypto::SecureBytes;
use foundation_nativeapis::shared::vfs::VfsFileType;
use foundation_nativeapis::{AsyncVfsFileSystem, VfsError};

use crate::secret::{SigningKey, VerifyingKey};

use parse::ParsedKey;

/// The algorithm of a loaded private key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateKeyAlgorithm {
    /// Ed25519 (RFC 8032).
    Ed25519,
    /// ECDSA over NIST P-256.
    EcdsaP256,
    /// RSA. Native with feature `secret-rsa` only.
    Rsa,
}

impl PrivateKeyAlgorithm {
    /// The name of the algorithm, for a log line or an error message.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::EcdsaP256 => "ecdsa-p256",
            Self::Rsa => "rsa",
        }
    }
}

/// A private key that the host loaded, with its public key.
///
/// `Debug` prints the algorithm and the key ID, never key bytes. The key bytes
/// are zeroed on drop.
pub struct PrivateKey {
    algorithm: PrivateKeyAlgorithm,
    kid: String,
    /// The key in PKCS#8 DER form, whatever the input format was.
    pkcs8_der: SecureBytes,
    public_key: Vec<u8>,
}

impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateKey")
            .field("algorithm", &self.algorithm)
            .field("kid", &self.kid)
            .field(
                "public_key_sha256",
                &crate::fingerprint::sha256_hex(&self.public_key),
            )
            .finish_non_exhaustive()
    }
}

impl PrivateKey {
    /// Reads and parses the key. The read is async and runs over `files`, the
    /// file system that the host installs. It never uses `std::fs` and never
    /// blocks the executor.
    ///
    /// `files` is `NativeFs` (feature `vfs-native`) on a native host,
    /// `MemoryFs` in a test or a deterministic simulation, and whatever store
    /// a wasm host installs. An `Inline` source ignores `files`.
    ///
    /// # Errors
    /// [`KeyLoadError::NotFound`] when the file system has no such path,
    /// [`KeyLoadError::Io`] for any other file system error,
    /// [`KeyLoadError::LoosePermissions`] on Unix, and the parse errors of
    /// [`PrivateKey::parse`].
    pub async fn load<F: AsyncVfsFileSystem>(
        source: &KeySource,
        files: &F,
    ) -> Result<Self, KeyLoadError> {
        match source {
            KeySource::Inline(text) => Self::parse(text.as_slice()),
            KeySource::File(path) => {
                let metadata = files
                    .stat_async(path.clone())
                    .await
                    .map_err(|error| match error.current_context() {
                        VfsError::NotFound { .. } => KeyLoadError::NotFound(path.clone()),
                        other => KeyLoadError::Io(format!("{path}: {other}")),
                    })?;
                if metadata.file_type != VfsFileType::Regular {
                    return Err(KeyLoadError::UnsupportedFormat(format!(
                        "{path} is not a regular file"
                    )));
                }
                check_permissions(path, metadata.permissions)?;
                let text =
                    files.read_file_async(path.clone()).await.map_err(|error| {
                        match error.current_context() {
                            VfsError::NotFound { .. } => KeyLoadError::NotFound(path.clone()),
                            other => KeyLoadError::Io(format!("{path}: {other}")),
                        }
                    })?;
                Self::parse(&text)
            }
        }
    }

    /// Parses key text in one of the formats of section 4.3 of the design:
    /// OpenSSH (no passphrase), PKCS#8 (version 1 and 2), and SEC1 P-256.
    ///
    /// # Errors
    /// [`KeyLoadError::Encrypted`], [`KeyLoadError::UnsupportedFormat`].
    pub fn parse(text: &[u8]) -> Result<Self, KeyLoadError> {
        let parsed = parse::parse(text)?;
        Ok(Self::from_parsed(parsed))
    }

    fn from_parsed(parsed: ParsedKey) -> Self {
        let kid = crate::secret::keys::derived_kid(&parsed.public_key);
        Self {
            algorithm: parsed.algorithm,
            kid,
            pkcs8_der: SecureBytes::new(parsed.pkcs8_der),
            public_key: parsed.public_key,
        }
    }

    /// Sets a `kid` from configuration in place of the derived one.
    #[must_use]
    pub fn with_kid(mut self, kid: impl Into<String>) -> Self {
        self.kid = kid.into();
        self
    }

    /// The algorithm of the key.
    #[must_use]
    pub fn algorithm(&self) -> PrivateKeyAlgorithm {
        self.algorithm
    }

    /// The key ID: the first 16 hexadecimal characters of the SHA-256 of the
    /// public key, unless configuration set another one.
    #[must_use]
    pub fn kid(&self) -> &str {
        &self.kid
    }

    /// The raw public key bytes.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// The key as a secret-signing key.
    ///
    /// # Errors
    /// [`KeyLoadError::WrongAlgorithm`] for a P-256 key.
    pub fn into_signing_key(self) -> Result<SigningKey, KeyLoadError> {
        if self.algorithm != PrivateKeyAlgorithm::Ed25519 {
            return Err(KeyLoadError::WrongAlgorithm {
                expected: PrivateKeyAlgorithm::Ed25519,
                found: self.algorithm,
            });
        }
        SigningKey::from_pkcs8_der(&self.pkcs8_der, Some(self.kid))
            .map_err(|error| KeyLoadError::UnsupportedFormat(error.to_string()))
    }

    /// The PKCS#8 DER bytes, for the CA and the server identity.
    #[must_use]
    pub fn pkcs8_der(&self) -> &[u8] {
        &self.pkcs8_der
    }
}

/// Check that only the owner can read a key file.
#[cfg(unix)]
fn check_permissions(path: &str, permissions: u32) -> Result<(), KeyLoadError> {
    if permissions & 0o077 != 0 {
        return Err(KeyLoadError::LoosePermissions(path.to_string()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_permissions(_path: &str, _permissions: u32) -> Result<(), KeyLoadError> {
    Ok(())
}

impl VerifyingKey {
    /// Parses the JSON list of `AGENT_RPC_SECRET_PUBLIC_KEYS`: an array of
    /// `{"kid": "...", "alg": "ed25519", "public_key": "<base64url>"}`.
    ///
    /// # Errors
    /// [`KeyLoadError::UnsupportedFormat`] for bad JSON or an unknown `alg`.
    pub fn list_from_json(text: &str) -> Result<Vec<Self>, KeyLoadError> {
        crate::secret::keys::parse_verifying_keys(text).map_err(KeyLoadError::UnsupportedFormat)
    }
}

/// Every way a key can fail to load.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum KeyLoadError {
    /// There is no key at the path or in the variable.
    #[display("no key at {_0}; make one with: ssh-keygen -t ed25519 -N \"\" -f {_0}")]
    NotFound(#[error(not(source))] String),
    /// The file system failed.
    #[display("cannot read key {_0}")]
    Io(#[error(not(source))] String),
    /// The file is readable by the group or by others.
    #[display("key file {_0} can be read by other users; run: chmod 600 {_0}")]
    LoosePermissions(#[error(not(source))] String),
    /// The key has a passphrase.
    #[display("the key has a passphrase; the proof of concept reads only keys made with -N \"\"")]
    Encrypted,
    /// The format is not one that this build reads.
    #[display("unsupported key format: {_0}")]
    UnsupportedFormat(#[error(not(source))] String),
    /// The key has the wrong algorithm for its role.
    #[display("this role needs a {expected:?} key, found {found:?}")]
    WrongAlgorithm {
        /// The algorithm that the role needs.
        expected: PrivateKeyAlgorithm,
        /// The algorithm that the key has.
        found: PrivateKeyAlgorithm,
    },
}
