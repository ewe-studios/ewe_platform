//! Load or make the key material of a host.
//!
//! WHY: a host needs a secret-signing key, a certificate authority (CA) key
//! and a server key. It reads them from its key store, or it makes them once
//! and keeps them sealed there.
//!
//! WHAT: [`KeyMaterial`], [`KeyProtection`] and the load-or-create steps for
//! each key.
//!
//! HOW: the store `create` is first-writer-wins, so two processes that start at
//! the same time agree on one key. A private key is sealed with the current
//! key encryption key (KEK). An older KEK can still open a blob that names it.

use std::sync::Arc;

use base64::Engine as _;
use foundation_compact::crypto::SecureBytes;
use foundation_context::Ctx;
use zeroize::Zeroizing;

use crate::secret::{SigningAlgorithm, SigningKey, VerifyingKey};

use super::{
    record_aad, CreateOutcome, KeyId, KeyKind, KeyRecord, KeySealer, KeyStore, KeyStoreError,
    PublicMaterial, SealedBlob, StoredPrivateKey,
};

/// The 16-byte prefix of an Ed25519 PKCS#8 version 1 key (RFC 8410).
const ED25519_PKCS8_HEADER: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
];

/// How private keys are kept. Owner decision 3: always sealed. There is no
/// clear-text variant.
#[derive(Clone)]
pub enum KeyProtection {
    /// The current sealer, and older sealers that can still open blobs sealed
    /// before a KEK rotation.
    Sealed {
        /// The sealer for new blobs.
        current: Arc<dyn KeySealer>,
        /// The sealers for blobs that name an older KEK.
        previous: Vec<Arc<dyn KeySealer>>,
    },
}

impl std::fmt::Debug for KeyProtection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sealed { current, previous } => f
                .debug_struct("Sealed")
                .field("current", &current.kek_id())
                .field(
                    "previous",
                    &previous.iter().map(|s| s.kek_id()).collect::<Vec<_>>(),
                )
                .finish(),
        }
    }
}

impl KeyProtection {
    /// A protection with one current sealer.
    #[must_use]
    pub fn new(current: Arc<dyn KeySealer>) -> Self {
        Self::Sealed {
            current,
            previous: Vec::new(),
        }
    }

    /// A protection with a current sealer and older sealers.
    #[must_use]
    pub fn with_previous(current: Arc<dyn KeySealer>, previous: Vec<Arc<dyn KeySealer>>) -> Self {
        Self::Sealed { current, previous }
    }

    fn current(&self) -> &Arc<dyn KeySealer> {
        match self {
            Self::Sealed { current, .. } => current,
        }
    }

    fn for_kek(&self, kek_id: &str) -> Option<&Arc<dyn KeySealer>> {
        match self {
            Self::Sealed { current, previous } => previous
                .iter()
                .chain(core::iter::once(current))
                .find(|sealer| sealer.kek_id() == kek_id),
        }
    }
}

/// Loads or makes the key material of a host.
pub struct KeyMaterial {
    store: Arc<dyn KeyStore>,
    /// `None` for a verify-only host: it never makes or opens a private key.
    protection: Option<KeyProtection>,
    ctx: Ctx,
}

impl std::fmt::Debug for KeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyMaterial")
            .field("store", &self.store.info().name)
            .field("protection", &self.protection)
            .finish_non_exhaustive()
    }
}

impl KeyMaterial {
    /// A host that makes and opens private keys.
    #[must_use]
    pub fn new(store: Arc<dyn KeyStore>, protection: KeyProtection, ctx: Ctx) -> Self {
        Self {
            store,
            protection: Some(protection),
            ctx,
        }
    }

    /// A host that only verifies (a Worker). Every call that needs a private
    /// key returns [`KeyStoreError::ReadOnly`].
    #[must_use]
    pub fn verify_only(store: Arc<dyn KeyStore>, ctx: Ctx) -> Self {
        Self {
            store,
            protection: None,
            ctx,
        }
    }

    /// The store under this material.
    #[must_use]
    pub fn store(&self) -> &Arc<dyn KeyStore> {
        &self.store
    }

    /// The wall-clock time in seconds since the UNIX epoch.
    #[must_use]
    pub fn now_unix(&self) -> u64 {
        u64::try_from(foundation_compact::time::unix_seconds(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or_default()
    }

    /// The sealer for new blobs.
    fn current_sealer(&self) -> Result<&Arc<dyn KeySealer>, KeyStoreError> {
        match &self.protection {
            Some(protection) => Ok(protection.current()),
            None => Err(KeyStoreError::ReadOnly),
        }
    }

    /// Seal `plaintext` with the current KEK.
    async fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<SealedBlob, KeyStoreError> {
        self.current_sealer()?.seal(aad, plaintext).await
    }

    /// Open a blob with the sealer that names its KEK.
    async fn open(&self, aad: &[u8], sealed: &SealedBlob) -> Result<SecureBytes, KeyStoreError> {
        let protection = self.protection.as_ref().ok_or(KeyStoreError::ReadOnly)?;
        let opener = protection.for_kek(&sealed.kek_id).ok_or_else(|| {
            KeyStoreError::KekUnavailable(format!(
                "no sealer for KEK {}; add it to the previous KEKs",
                sealed.kek_id
            ))
        })?;
        opener.open(aad, sealed).await
    }

    /// Open the private part of `record`.
    ///
    /// # Errors
    /// [`KeyStoreError::ReadOnly`], [`KeyStoreError::PlaintextRefused`],
    /// [`KeyStoreError::KekUnavailable`] or [`KeyStoreError::Seal`].
    pub async fn open_private(&self, record: &KeyRecord) -> Result<SecureBytes, KeyStoreError> {
        match &record.private {
            StoredPrivateKey::Sealed(sealed) => self.open(&record.aad(), sealed).await,
            StoredPrivateKey::Plaintext { .. } => Err(KeyStoreError::PlaintextRefused("key store")),
            StoredPrivateKey::External { .. } => Err(KeyStoreError::Unsupported(format!(
                "the private key of {} lives outside the store",
                record.id
            ))),
            StoredPrivateKey::Absent => Err(KeyStoreError::NotFound(record.id.to_string())),
        }
    }

    /// Seal `plaintext` under `aad` with the current KEK.
    ///
    /// The migration of the clear-text key files uses it.
    ///
    /// # Errors
    /// [`KeyStoreError::ReadOnly`] or [`KeyStoreError::Seal`].
    pub async fn seal_bytes(
        &self,
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<SealedBlob, KeyStoreError> {
        self.seal(aad, plaintext).await
    }

    /// Reseal every sealed record with the current KEK.
    ///
    /// A record that already names the current KEK is skipped. Each record
    /// moves to the next revision, and the record's current revision guards
    /// the write, so a concurrent change is a conflict and is not lost.
    ///
    /// # Returns
    /// The number of records that this call resealed.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants.
    pub async fn rotate_kek(&self) -> Result<usize, KeyStoreError> {
        let protection = self.protection.as_ref().ok_or(KeyStoreError::ReadOnly)?;
        let current_id = protection.current().kek_id().to_string();
        let mut rotated = 0;
        for kind in [
            KeyKind::CertificateAuthority,
            KeyKind::SecretSigning,
            KeyKind::ServerIdentity,
        ] {
            for record in self.store.list(kind).await? {
                let StoredPrivateKey::Sealed(sealed) = &record.private else {
                    continue;
                };
                if sealed.kek_id == current_id {
                    continue;
                }
                let plaintext = self.open(&record.aad(), sealed).await?;
                let revision = record.revision + 1;
                let aad = record_aad(&record.id, record.kind, revision);
                let resealed = protection.current().seal(&aad, &plaintext).await?;
                let mut updated = record.clone();
                updated.revision = revision;
                updated.private = StoredPrivateKey::Sealed(resealed);
                self.store.replace(&updated, record.revision).await?;
                rotated += 1;
            }
        }
        Ok(rotated)
    }

    /// Load a record, or make it with `make` and store it.
    ///
    /// `make` gives the public part and the clear private bytes; this method
    /// seals the bytes before it stores them, so a store never sees clear text.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants; a corrupt record is an error and is not
    /// replaced.
    pub async fn load_or_create<F>(
        &self,
        id: &KeyId,
        kind: KeyKind,
        make: F,
    ) -> Result<KeyRecord, KeyStoreError>
    where
        F: FnOnce() -> Result<(PublicMaterial, SecureBytes), KeyStoreError>,
    {
        if let Some(found) = self.store.get(id).await? {
            return Ok(found);
        }
        let info = self.store.info();
        if !info.writable {
            return Err(KeyStoreError::NotFound(id.to_string()));
        }
        if info.shared && !info.atomic_create {
            return Err(KeyStoreError::NotAtomic);
        }
        // A verify-only host never makes a private key.
        if self.protection.is_none() {
            return Err(KeyStoreError::ReadOnly);
        }
        let (public, plaintext) = make()?;
        let revision = 1;
        let aad = record_aad(id, kind, revision);
        let sealed = self.seal(&aad, &plaintext).await?;
        let fresh = KeyRecord {
            id: id.clone(),
            kind,
            revision,
            created_at_unix: self.now_unix(),
            not_after_unix: None,
            public,
            private: StoredPrivateKey::Sealed(sealed),
        };
        match self.store.create(&fresh).await? {
            CreateOutcome::Created => Ok(fresh),
            CreateOutcome::AlreadyExists(winner) => {
                tracing::info!(
                    key = id.as_str(),
                    "another process created this key first; using it"
                );
                Ok(winner)
            }
        }
    }

    /// The secret-signing key: loaded, or made (Ed25519) and stored.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants.
    pub async fn signing_key(&self, id: &KeyId) -> Result<SigningKey, KeyStoreError> {
        let record = self
            .load_or_create(id, KeyKind::SecretSigning, || self.make_ed25519())
            .await?;
        self.open_signing_key(&record).await
    }

    /// The signing key of a record that this material loaded.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants.
    pub async fn open_signing_key(&self, record: &KeyRecord) -> Result<SigningKey, KeyStoreError> {
        let bytes = self.open_private(record).await?;
        SigningKey::from_pkcs8_der(&bytes, Some(record.kid()))
            .map_err(|e| KeyStoreError::Corrupt(format!("{}: {e}", record.id)))
    }

    /// A new secret-signing record, sealed but not stored.
    ///
    /// Rotation uses it to replace the current record at the next revision.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants.
    pub async fn fresh_signing_record(
        &self,
        id: &KeyId,
        revision: u64,
    ) -> Result<KeyRecord, KeyStoreError> {
        let (public, plaintext) = self.make_ed25519()?;
        let aad = record_aad(id, KeyKind::SecretSigning, revision);
        let sealed = self.seal(&aad, &plaintext).await?;
        Ok(KeyRecord {
            id: id.clone(),
            kind: KeyKind::SecretSigning,
            revision,
            created_at_unix: self.now_unix(),
            not_after_unix: None,
            public,
            private: StoredPrivateKey::Sealed(sealed),
        })
    }

    /// The public keys for a verify-only host (a Worker).
    ///
    /// # Errors
    /// [`KeyStoreError::NotFound`] when a key is missing.
    pub async fn verifying_keys(&self, ids: &[KeyId]) -> Result<Vec<VerifyingKey>, KeyStoreError> {
        let mut keys = Vec::with_capacity(ids.len());
        for id in ids {
            let record = self
                .store
                .get(id)
                .await?
                .ok_or_else(|| KeyStoreError::NotFound(id.to_string()))?;
            keys.push(verifying_key(&record)?);
        }
        Ok(keys)
    }

    /// Every verifying key of `kind` in the store. Rotation uses it: the
    /// verifier holds the old public keys until their secrets expire.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants.
    pub async fn verifying_keys_of_kind(
        &self,
        kind: KeyKind,
    ) -> Result<Vec<VerifyingKey>, KeyStoreError> {
        let records = self.store.list(kind).await?;
        records.iter().map(verifying_key).collect()
    }

    /// The certificate authority: loaded, or made and stored.
    ///
    /// The CA certificate in the record is reused when it matches the key.
    /// Otherwise a new self-signed certificate is made and written back to the
    /// record. A certificate is public, so the record can hold it.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants and the certificate errors of
    /// [`CertificateAuthority`](crate::pki::CertificateAuthority).
    #[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
    pub async fn certificate_authority(
        &self,
        id: &KeyId,
        config: &crate::pki::CaConfig,
    ) -> Result<crate::pki::CertificateAuthority, KeyStoreError> {
        let ctx = self.ctx.clone();
        let algorithm = config.key_algorithm;
        let record = self
            .load_or_create(id, KeyKind::CertificateAuthority, || {
                let key = crate::pki::CaKey::generate(algorithm, &ctx)
                    .map_err(|e| KeyStoreError::Seal(e.to_string()))?;
                let public = PublicMaterial {
                    algorithm: key_algorithm_name(algorithm).to_string(),
                    public_key_b64url: base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .encode(key.public_key_der()),
                    cert_pem: None,
                    subject_names: Vec::new(),
                };
                Ok((public, SecureBytes::new(key.to_pem().as_bytes().to_vec())))
            })
            .await?;
        let pem = self.open_private(&record).await?;
        let pem = std::str::from_utf8(&pem)
            .map_err(|_| KeyStoreError::Corrupt(id.to_string()))?
            .to_string();
        let key = crate::pki::CaKey::from_pem(&pem)
            .map_err(|e| KeyStoreError::Corrupt(format!("{id}: {e}")))?;
        let (ca, fresh) = crate::pki::CertificateAuthority::from_key(
            key,
            record.public.cert_pem.as_deref(),
            config,
            &self.ctx,
        )
        .map_err(|e| KeyStoreError::Seal(e.to_string()))?;
        if let Some(cert_pem) = fresh {
            let mut updated = record.clone();
            updated.public.cert_pem = Some(cert_pem);
            self.store.replace(&updated, record.revision).await?;
        }
        Ok(ca)
    }

    /// The server certificate: loaded, or made and stored.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants and the certificate errors of the CA.
    #[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
    pub async fn server_identity(
        &self,
        ca: &crate::pki::CertificateAuthority,
        id: &KeyId,
        names: &[String],
        lifetime: foundation_compact::Duration,
    ) -> Result<crate::pki::ServerIdentity, KeyStoreError> {
        let record = self
            .load_or_create(id, KeyKind::ServerIdentity, || {
                self.make_p256(&[]).map(|(public, private)| {
                    let mut public = public;
                    public.subject_names = names.to_vec();
                    (public, private)
                })
            })
            .await?;
        let pem = self.open_private(&record).await?;
        let pem = std::str::from_utf8(&pem)
            .map_err(|_| KeyStoreError::Corrupt(id.to_string()))?
            .to_string();
        let key = crate::pki::CaKey::from_pem(&pem)
            .map_err(|e| KeyStoreError::Corrupt(format!("{id}: {e}")))?;
        ca.issue_server_identity(&key, names, lifetime)
            .map_err(|e| KeyStoreError::Seal(e.to_string()))
    }

    /// A new Ed25519 secret-signing key as (public part, PKCS#8 DER).
    fn make_ed25519(&self) -> Result<(PublicMaterial, SecureBytes), KeyStoreError> {
        let env = self
            .ctx
            .require_env()
            .map_err(|e| KeyStoreError::Seal(e.to_string()))?;
        let mut seed = Zeroizing::new([0u8; 32]);
        env.entropy()
            .fill(&mut *seed)
            .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))?;
        let pair = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(*seed));
        let mut pkcs8 = Vec::with_capacity(48);
        pkcs8.extend_from_slice(&ED25519_PKCS8_HEADER);
        pkcs8.extend_from_slice(&*seed);
        let public = PublicMaterial {
            algorithm: "ed25519".to_string(),
            public_key_b64url: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(pair.pk.as_slice()),
            cert_pem: None,
            subject_names: Vec::new(),
        };
        Ok((public, SecureBytes::new(pkcs8)))
    }

    /// A new P-256 key as (public part, PKCS#8 PEM).
    #[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
    fn make_p256(
        &self,
        subject_names: &[String],
    ) -> Result<(PublicMaterial, SecureBytes), KeyStoreError> {
        use p256::elliptic_curve::sec1::ToEncodedPoint as _;
        use p256::pkcs8::EncodePrivateKey as _;

        let env = self
            .ctx
            .require_env()
            .map_err(|e| KeyStoreError::Seal(e.to_string()))?;
        let mut secret = None;
        while secret.is_none() {
            let mut scalar = Zeroizing::new([0u8; 32]);
            env.entropy()
                .fill(&mut *scalar)
                .map_err(|e| KeyStoreError::Seal(format!("entropy source: {e}")))?;
            secret = p256::SecretKey::from_slice(&*scalar).ok();
        }
        let secret = secret.ok_or_else(|| KeyStoreError::Seal("P-256 key".to_string()))?;
        let der = secret
            .to_pkcs8_der()
            .map_err(|e| KeyStoreError::Seal(format!("P-256 PKCS#8: {e}")))?;
        let pem = super::fs::pem_from_der(der.as_bytes());
        let public = PublicMaterial {
            algorithm: "ecdsa-p256".to_string(),
            public_key_b64url: base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(secret.public_key().to_encoded_point(false).as_bytes()),
            cert_pem: None,
            subject_names: subject_names.to_vec(),
        };
        Ok((public, SecureBytes::new(pem.into_bytes())))
    }
}

/// The design name of a CA key algorithm.
#[cfg(all(not(target_family = "wasm"), feature = "mtls"))]
fn key_algorithm_name(algorithm: crate::pki::KeyAlgorithm) -> &'static str {
    match algorithm {
        crate::pki::KeyAlgorithm::EcdsaP256 => "ecdsa-p256",
        crate::pki::KeyAlgorithm::Ed25519 => "ed25519",
    }
}

/// The verifying key of a record.
///
/// # Errors
/// [`KeyStoreError::Corrupt`] for an unknown algorithm or a bad public key.
fn verifying_key(record: &KeyRecord) -> Result<VerifyingKey, KeyStoreError> {
    let algorithm = SigningAlgorithm::parse(&record.public.algorithm)
        .ok_or_else(|| KeyStoreError::Corrupt(format!("{}: unknown algorithm", record.id)))?;
    VerifyingKey::from_base64url(
        algorithm,
        &record.public.public_key_b64url,
        Some(record.kid()),
    )
    .map_err(|e| KeyStoreError::Corrupt(format!("{}: {e}", record.id)))
}
