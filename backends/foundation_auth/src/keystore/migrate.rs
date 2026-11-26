//! Migrate the clear-text key files of the first version to sealed records.
//!
//! WHY: the first version keeps each private key in a file with mode 0600.
//! Owner decision 3 says a private key is always encrypted at rest, so the host
//! must seal each legacy key and then remove the clear-text file.
//!
//! WHAT: [`migrate_legacy_keys`] and [`MigrationReport`].
//!
//! HOW: for each known key, the file store returns a read-only `Plaintext`
//! record. The migration seals it, stores the sealed record, opens it again to
//! prove that the key is usable, and only then removes the clear-text file. A
//! failure at any step stops the migration and keeps the file. A CA
//! certificate is public, so the migration keeps it.

use std::path::PathBuf;

use super::fs::FsKeyStore;
use super::{
    CreateOutcome, KeyId, KeyKind, KeyMaterial, KeyStore, KeyStoreError, StoredPrivateKey,
};

/// What a migration changed.
#[derive(Debug, Clone, Default)]
pub struct MigrationReport {
    /// The records that this call sealed.
    pub migrated: Vec<KeyId>,
    /// The clear-text files that this call removed.
    pub removed: Vec<PathBuf>,
}

impl MigrationReport {
    /// True when the migration changed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.migrated.is_empty()
    }
}

/// Seal every legacy clear-text key in `store` and remove its file.
///
/// # Errors
/// [`KeyStoreError`] variants. A sealed record is not written back as clear
/// text, and a clear-text file stays when the sealed record does not open.
pub async fn migrate_legacy_keys(
    store: &FsKeyStore,
    material: &KeyMaterial,
) -> Result<MigrationReport, KeyStoreError> {
    let mut report = MigrationReport::default();
    for (name, kind) in [
        (KeyId::CA, KeyKind::CertificateAuthority),
        (KeyId::SECRET_SIGNING, KeyKind::SecretSigning),
        (KeyId::SERVER_IDENTITY, KeyKind::ServerIdentity),
    ] {
        let id = KeyId::new(name)?;
        let Some(record) = store.get(&id).await? else {
            continue;
        };
        let StoredPrivateKey::Plaintext { pkcs8_pem } = &record.private else {
            continue;
        };
        // A signing key seals its PKCS#8 DER form, which is what
        // `open_signing_key` parses. A CA or server key seals its PEM text,
        // which is what `CaKey::from_pem` parses.
        let plaintext = match kind {
            KeyKind::SecretSigning => {
                let key = crate::keys::PrivateKey::parse(pkcs8_pem.expose().as_bytes())
                    .map_err(|e| KeyStoreError::Corrupt(format!("{id}: {e}")))?;
                foundation_compact::crypto::SecureBytes::new(key.pkcs8_der().to_vec())
            }
            KeyKind::CertificateAuthority | KeyKind::ServerIdentity => {
                foundation_compact::crypto::SecureBytes::new(pkcs8_pem.expose().as_bytes().to_vec())
            }
        };
        let sealed = material
            .seal_bytes(&record.aad(), plaintext.as_slice())
            .await?;
        let mut sealed_record = record.clone();
        sealed_record.private = StoredPrivateKey::Sealed(sealed);
        // The legacy record is not a file yet, but another process may have
        // migrated it since the read.
        let sealed_record = match store.create(&sealed_record).await? {
            CreateOutcome::Created => sealed_record,
            CreateOutcome::AlreadyExists(winner) => winner,
        };
        verify(&sealed_record, material).await?;
        let legacy = FsKeyStore::legacy_key_file(&id).map(|name| store.legacy_path(name));
        if store.remove_legacy(&id)? {
            if let Some(path) = legacy {
                report.removed.push(path);
            }
        }
        report.migrated.push(id);
    }
    Ok(report)
}

/// Prove that the sealed record opens and holds a valid key.
async fn verify(record: &super::KeyRecord, material: &KeyMaterial) -> Result<(), KeyStoreError> {
    match record.kind {
        KeyKind::SecretSigning => {
            material.open_signing_key(record).await?;
        }
        KeyKind::CertificateAuthority | KeyKind::ServerIdentity => {
            let pem = material.open_private(record).await?;
            #[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
            {
                let text = std::str::from_utf8(&pem)
                    .map_err(|_| KeyStoreError::Corrupt(record.id.to_string()))?;
                crate::pki::CaKey::from_pem(text)
                    .map_err(|e| KeyStoreError::Corrupt(format!("{}: {e}", record.id)))?;
            }
            #[cfg(not(all(not(target_family = "wasm"), feature = "mtls")))]
            {
                let _ = pem;
            }
        }
    }
    Ok(())
}
