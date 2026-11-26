//! A key store over the file system of a native host.
//!
//! Records live in `<state-dir>/keys/<id>.json`. A create writes a temporary
//! file and then hard-links it to the final name: `hard_link` fails when the
//! name exists, so two processes cannot both create the same record.
//!
//! The store also reads the clear-text key files of the first version
//! (`ca-key.pem`, `bootstrap-signing-key.pem`, `server-key.pem`) as read-only
//! [`StoredPrivateKey::Plaintext`] records. Task K3 seals them and removes
//! them at the first start with a key encryption key (KEK).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use base64::Engine as _;

use crate::keys::{PrivateKey, PrivateKeyAlgorithm};

use super::{
    ensure_sealed, CreateOutcome, KeyFuture, KeyId, KeyKind, KeyRecord, KeyStore, KeyStoreError,
    KeyStoreInfo, PublicMaterial, SecretText, StoredPrivateKey,
};

/// The subdirectory of the state directory that holds the records.
pub const KEYSTORE_DIR: &str = "keys";
/// The file name of the legacy certificate authority (CA) private key.
pub const LEGACY_CA_KEY: &str = "ca-key.pem";
/// The file name of the legacy CA certificate.
pub const LEGACY_CA_CERT: &str = "ca-cert.pem";
/// The file name of the legacy secret-signing key.
pub const LEGACY_SIGNING_KEY: &str = "bootstrap-signing-key.pem";
/// The file name of the legacy server private key.
pub const LEGACY_SERVER_KEY: &str = "server-key.pem";

/// A key store over a directory.
pub struct FsKeyStore {
    base: PathBuf,
}

impl std::fmt::Debug for FsKeyStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsKeyStore")
            .field("base", &self.base)
            .finish_non_exhaustive()
    }
}

impl FsKeyStore {
    /// A store over `state_dir`.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>) -> Self {
        Self {
            base: state_dir.into(),
        }
    }

    /// The directory that holds the record files.
    #[must_use]
    pub fn records_dir(&self) -> PathBuf {
        self.base.join(KEYSTORE_DIR)
    }

    /// The file of `id`.
    #[must_use]
    pub fn record_path(&self, id: &KeyId) -> PathBuf {
        self.records_dir().join(format!("{}.json", id.as_str()))
    }

    /// The path of a legacy file in the state directory.
    #[must_use]
    pub fn legacy_path(&self, name: &str) -> PathBuf {
        self.base.join(name)
    }

    /// Make the record directory, readable only by its owner.
    fn make_dir(&self) -> Result<(), KeyStoreError> {
        let dir = self.records_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| KeyStoreError::Storage(format!("creating {}: {e}", dir.display())))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).map_err(
                |e| KeyStoreError::Storage(format!("protecting {}: {e}", dir.display())),
            )?;
        }
        Ok(())
    }

    /// Read one file as text, or `None` when it does not exist.
    fn read_optional(path: &Path) -> Result<Option<String>, KeyStoreError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(KeyStoreError::Storage(format!(
                "reading {}: {error}",
                path.display()
            ))),
        }
    }

    /// Read a record from its file.
    fn read_record(path: &Path) -> Result<Option<KeyRecord>, KeyStoreError> {
        let Some(text) = Self::read_optional(path)? else {
            return Ok(None);
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| KeyStoreError::Corrupt(format!("{}: {e}", path.display())))
    }

    /// Write `record` to a temporary file and install it at `path` only if
    /// `path` is absent. Returns `false` when another writer installed one.
    fn create_file(path: &Path, record: &KeyRecord) -> Result<bool, KeyStoreError> {
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| KeyStoreError::Storage(format!("encoding a record: {e}")))?;
        let temp = Self::write_temp(path, &bytes)?;
        match std::fs::hard_link(&temp, path) {
            Ok(()) => {
                let _ = std::fs::remove_file(&temp);
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&temp);
                Ok(false)
            }
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(KeyStoreError::Storage(format!(
                    "installing {}: {error}",
                    path.display()
                )))
            }
        }
    }

    /// Write `record` over `path`, after the caller checked the revision.
    fn replace_file(path: &Path, record: &KeyRecord) -> Result<(), KeyStoreError> {
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|e| KeyStoreError::Storage(format!("encoding a record: {e}")))?;
        let temp = Self::write_temp(path, &bytes)?;
        // On Windows a rename cannot replace a file; the remove makes room.
        #[cfg(windows)]
        let _ = std::fs::remove_file(path);
        std::fs::rename(&temp, path).map_err(|error| {
            let _ = std::fs::remove_file(&temp);
            KeyStoreError::Storage(format!("installing {}: {error}", path.display()))
        })
    }

    /// Write `bytes` to a new temporary file next to `path`, mode 0600.
    fn write_temp(path: &Path, bytes: &[u8]) -> Result<PathBuf, KeyStoreError> {
        let mut temp = path.as_os_str().to_owned();
        temp.push(format!(".tmp-{}", std::process::id()));
        let temp = PathBuf::from(temp);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|e| KeyStoreError::Storage(format!("creating {}: {e}", temp.display())))?;
        file.write_all(bytes)
            .map_err(|e| KeyStoreError::Storage(format!("writing {}: {e}", temp.display())))?;
        file.sync_all()
            .map_err(|e| KeyStoreError::Storage(format!("syncing {}: {e}", temp.display())))?;
        drop(file);
        Ok(temp)
    }

    /// Build a read-only record from a legacy clear-text key file.
    fn legacy_record(&self, id: &KeyId) -> Result<Option<KeyRecord>, KeyStoreError> {
        match id.as_str() {
            KeyId::CA => {
                let key = Self::read_optional(&self.legacy_path(LEGACY_CA_KEY))?;
                let cert = Self::read_optional(&self.legacy_path(LEGACY_CA_CERT))?;
                match (key, cert) {
                    (None, None) => Ok(None),
                    (Some(_), None) | (None, Some(_)) => {
                        Err(KeyStoreError::Corrupt(id.to_string()))
                    }
                    (Some(key), Some(cert)) => Ok(Some(Self::plaintext_record(
                        id,
                        KeyKind::CertificateAuthority,
                        &key,
                        Some(cert),
                    )?)),
                }
            }
            KeyId::SECRET_SIGNING => {
                let Some(key) = Self::read_optional(&self.legacy_path(LEGACY_SIGNING_KEY))? else {
                    return Ok(None);
                };
                Ok(Some(Self::plaintext_record(
                    id,
                    KeyKind::SecretSigning,
                    &key,
                    None,
                )?))
            }
            KeyId::SERVER_IDENTITY => {
                let Some(key) = Self::read_optional(&self.legacy_path(LEGACY_SERVER_KEY))? else {
                    return Ok(None);
                };
                Ok(Some(Self::plaintext_record(
                    id,
                    KeyKind::ServerIdentity,
                    &key,
                    None,
                )?))
            }
            _ => Ok(None),
        }
    }

    /// A `Plaintext` record from legacy key text.
    fn plaintext_record(
        id: &KeyId,
        kind: KeyKind,
        key_text: &str,
        cert_pem: Option<String>,
    ) -> Result<KeyRecord, KeyStoreError> {
        let key = PrivateKey::parse(key_text.as_bytes())
            .map_err(|e| KeyStoreError::Corrupt(format!("legacy key {id}: {e}")))?;
        let pkcs8_pem = pem_from_der(key.pkcs8_der());
        Ok(KeyRecord {
            id: id.clone(),
            kind,
            revision: 1,
            created_at_unix: 0,
            not_after_unix: None,
            public: PublicMaterial {
                algorithm: algorithm_name(key.algorithm()).to_string(),
                public_key_b64url: base64::engine::general_purpose::URL_SAFE_NO_PAD
                    .encode(key.public_key()),
                cert_pem,
                subject_names: Vec::new(),
            },
            private: StoredPrivateKey::Plaintext {
                pkcs8_pem: SecretText::new(pkcs8_pem),
            },
        })
    }
}

/// The design name of a key algorithm.
#[must_use]
pub fn algorithm_name(algorithm: PrivateKeyAlgorithm) -> &'static str {
    match algorithm {
        PrivateKeyAlgorithm::Ed25519 => "ed25519",
        PrivateKeyAlgorithm::EcdsaP256 => "ecdsa-p256",
        PrivateKeyAlgorithm::Rsa => "rsa-pss-sha256",
    }
}

/// Encode PKCS#8 DER bytes as a `PRIVATE KEY` Privacy-Enhanced Mail (PEM)
/// block.
#[must_use]
pub fn pem_from_der(der: &[u8]) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(der);
    let mut pem = String::with_capacity(encoded.len() + 64);
    pem.push_str("-----BEGIN PRIVATE KEY-----\n");
    for line in encoded.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(line).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END PRIVATE KEY-----\n");
    pem
}

impl KeyStore for FsKeyStore {
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>> {
        Box::pin(async move {
            if let Some(record) = Self::read_record(&self.record_path(id))? {
                return Ok(Some(record));
            }
            self.legacy_record(id)
        })
    }

    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome> {
        Box::pin(async move {
            ensure_sealed(record, "file")?;
            self.make_dir()?;
            let path = self.record_path(&record.id);
            if Self::create_file(&path, record)? {
                Ok(CreateOutcome::Created)
            } else {
                match Self::read_record(&path)? {
                    Some(existing) => Ok(CreateOutcome::AlreadyExists(existing)),
                    None => Err(KeyStoreError::Corrupt(record.id.to_string())),
                }
            }
        })
    }

    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()> {
        Box::pin(async move {
            ensure_sealed(record, "file")?;
            self.make_dir()?;
            let path = self.record_path(&record.id);
            let existing = Self::read_record(&path)?
                .ok_or_else(|| KeyStoreError::NotFound(record.id.to_string()))?;
            if existing.revision != expected_revision {
                return Err(KeyStoreError::Conflict(record.id.to_string()));
            }
            Self::replace_file(&path, record)
        })
    }

    fn list(&self, kind: KeyKind) -> KeyFuture<'_, Vec<KeyRecord>> {
        Box::pin(async move {
            let dir = self.records_dir();
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
                Err(error) => {
                    return Err(KeyStoreError::Storage(format!(
                        "listing {}: {error}",
                        dir.display()
                    )))
                }
            };
            let mut records = Vec::new();
            for entry in entries {
                let entry = entry.map_err(|e| {
                    KeyStoreError::Storage(format!("listing {}: {e}", dir.display()))
                })?;
                if entry.path().extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                if let Some(record) = Self::read_record(&entry.path())? {
                    if record.kind == kind {
                        records.push(record);
                    }
                }
            }
            Ok(records)
        })
    }

    fn info(&self) -> KeyStoreInfo {
        KeyStoreInfo {
            name: "file",
            atomic_create: true,
            shared: false,
            writable: true,
        }
    }
}
