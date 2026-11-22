//! The store that holds the certificate authority (CA) certificate and key.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use foundation_context::Ctx;
use zeroize::Zeroizing;

use super::error::PkiError;
use super::{CaConfig, CertificateAuthority};

/// File name of the CA certificate inside a state directory.
pub const CA_CERT_FILE: &str = "ca-cert.pem";

/// File name of the CA private key inside a state directory.
pub const CA_KEY_FILE: &str = "ca-key.pem";

/// A stored CA: its certificate and its private key, in Privacy-Enhanced Mail
/// (PEM) form.
struct StoredCa {
    cert_pem: String,
    key_pem: Zeroizing<String>,
}

#[derive(Clone)]
enum CaStoreKind {
    File(PathBuf),
    Memory(Arc<Mutex<Option<StoredCa>>>),
}

/// Where the certificate authority (CA) lives: in a directory of files or in
/// memory.
///
/// A file store keeps the CA across restarts, because every client certificate
/// chains to it. A memory store is for a test and for a short-lived process.
#[derive(Clone)]
pub struct CaStore {
    kind: CaStoreKind,
}

impl std::fmt::Debug for CaStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            CaStoreKind::File(dir) => f.debug_tuple("CaStore::File").field(dir).finish(),
            CaStoreKind::Memory(_) => f.write_str("CaStore::Memory"),
        }
    }
}

impl CaStore {
    /// A store in `dir`. It reads `ca-cert.pem` and `ca-key.pem` from the
    /// directory.
    #[must_use]
    pub fn file(dir: impl Into<PathBuf>) -> Self {
        Self {
            kind: CaStoreKind::File(dir.into()),
        }
    }

    /// A store in memory. It holds the CA for the life of this value.
    #[must_use]
    pub fn memory() -> Self {
        Self {
            kind: CaStoreKind::Memory(Arc::new(Mutex::new(None))),
        }
    }

    /// Load the CA from the store, or generate one and save it when the store
    /// is empty.
    ///
    /// The method returns the CA and the new certificate in Privacy-Enhanced
    /// Mail (PEM) form. The second value is `Some` only when the method
    /// generated a new CA; the host then gives the new certificate to clients.
    ///
    /// # Errors
    /// [`PkiError::Store`] when the files cannot be read or written, when only
    /// one of the two files exists, or when the PEM text is invalid.
    pub fn load_or_generate(
        &self,
        config: &CaConfig,
        ctx: &Ctx,
    ) -> Result<(CertificateAuthority, Option<String>), PkiError> {
        match &self.kind {
            CaStoreKind::File(dir) => load_or_generate_file(dir, config, ctx),
            CaStoreKind::Memory(state) => {
                let stored = state.lock().unwrap_or_else(PoisonError::into_inner).take();
                if let Some(mut stored) = stored {
                    let key = super::CaKey::from_pem(&stored.key_pem)?;
                    let (ca, fresh) =
                        CertificateAuthority::from_key(key, Some(&stored.cert_pem), config, ctx)?;
                    // A certificate that does not match the key is replaced, so
                    // the next load is consistent.
                    if let Some(fresh_pem) = &fresh {
                        stored.cert_pem.clone_from(fresh_pem);
                    }
                    *state.lock().unwrap_or_else(PoisonError::into_inner) = Some(stored);
                    Ok((ca, fresh))
                } else {
                    let key = super::CaKey::generate(config.key_algorithm, ctx)?;
                    let key_pem = key.to_pem();
                    let (ca, fresh) = CertificateAuthority::from_key(key, None, config, ctx)?;
                    let cert_pem = fresh.clone().unwrap_or_default();
                    *state.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some(StoredCa { cert_pem, key_pem });
                    Ok((ca, fresh))
                }
            }
        }
    }
}

fn load_or_generate_file(
    dir: &Path,
    config: &CaConfig,
    ctx: &Ctx,
) -> Result<(CertificateAuthority, Option<String>), PkiError> {
    let (cert_path, key_path) = (dir.join(CA_CERT_FILE), dir.join(CA_KEY_FILE));
    let read = |path: &Path| {
        std::fs::read_to_string(path)
            .map_err(|e| PkiError::Store(format!("reading {}: {e}", path.display())))
    };
    match (cert_path.exists(), key_path.exists()) {
        (true, true) => {
            let key_pem = Zeroizing::new(read(&key_path)?);
            let cert_pem = read(&cert_path)?;
            let key = super::CaKey::from_pem(&key_pem)?;
            CertificateAuthority::from_key(key, Some(&cert_pem), config, ctx)
        }
        (false, false) => {
            std::fs::create_dir_all(dir)
                .map_err(|e| PkiError::Store(format!("creating {}: {e}", dir.display())))?;
            let key = super::CaKey::generate(config.key_algorithm, ctx)?;
            let key_pem = key.to_pem();
            let (ca, fresh) = CertificateAuthority::from_key(key, None, config, ctx)?;
            write_secret_file(&key_path, key_pem.as_bytes())?;
            write_secret_file(&cert_path, ca.ca_cert_pem().as_bytes())?;
            Ok((ca, fresh))
        }
        _ => Err(PkiError::Store(format!(
            "{} holds only one of {CA_CERT_FILE} and {CA_KEY_FILE}",
            dir.display()
        ))),
    }
}

/// Write a file that only the owner may read.
fn write_secret_file(path: &Path, bytes: &[u8]) -> Result<(), PkiError> {
    use std::io::Write as _;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| PkiError::Store(format!("opening {}: {e}", path.display())))?;
    file.write_all(bytes)
        .map_err(|e| PkiError::Store(format!("writing {}: {e}", path.display())))
}
