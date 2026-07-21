//! Request-scoped context passed to every portable handler (spec-57, F008 Stage 1).
//!
//! WHY: The `core/api` handlers must be cross-platform and testable — they can't
//! depend on `worker::RouteContext` (wasm) or a `foundation_http` request (native).
//! `KeychainContext` is the dependency-injection seam: it carries the
//! `foundation_db` storage handle and the `foundation_auth` JWT signing key +
//! verifier used to mint and check Bitwarden access/refresh tokens.
//!
//! WHAT: `KeychainContext { db, signing_key, verifier }`. It is `Clone` (all
//! fields are `Arc`), so it is cheap to move into async tasks.
//!
//! HOW: `db` resolves to Turso (native) or D1 (wasm) by target, per decision 03.
//! `signing_key` is a `foundation_auth::JwtSigningKey` (decision 01); the default
//! constructor generates an ephemeral Ed25519 key (fine for a single process and
//! for tests), while `with_signing_key` injects a persistent one loaded by the
//! server bootstrap so tokens survive restarts. `verifier` is derived from the
//! signing key's public key.

use std::sync::Arc;

use foundation_auth::shared::jwt::{
    JwtAlgorithm, JwtSigningKey, JwtVerifier, JwtVerifierConfig, PublicKeySource,
};
use foundation_db::core::storage_provider::AsyncQueryStore;

use crate::core::auth::ISSUER;

/// Dependencies the portable vault handlers operate over.
#[derive(Clone)]
pub struct KeychainContext {
    /// Relational store (Turso on native, D1 on wasm) — the vault's system of record.
    db: Arc<dyn AsyncQueryStore>,
    /// Server JWT signing key — mints access/refresh tokens.
    signing_key: Arc<JwtSigningKey>,
    /// Verifier derived from the signing key's public key — checks incoming tokens.
    verifier: Arc<JwtVerifier>,
}

impl KeychainContext {
    /// Build a context over the given async SQL store with a fresh ephemeral
    /// Ed25519 signing key.
    #[must_use]
    pub fn new(db: Arc<dyn AsyncQueryStore>) -> Self {
        Self::with_signing_key(db, JwtSigningKey::generate_ed25519())
    }

    /// Build a context with an explicit (persistent) signing key.
    ///
    /// # Panics
    ///
    /// Panics if a verifier cannot be derived from the signing key's public key —
    /// this only happens on an internally-inconsistent key and is not reachable
    /// with `JwtSigningKey::generate_ed25519()` or a valid loaded key.
    #[must_use]
    pub fn with_signing_key(db: Arc<dyn AsyncQueryStore>, signing_key: JwtSigningKey) -> Self {
        let verifier = build_verifier(&signing_key).expect("derive verifier from signing key");
        Self {
            db,
            signing_key: Arc::new(signing_key),
            verifier: Arc::new(verifier),
        }
    }

    /// The relational store handlers query.
    #[must_use]
    pub fn db(&self) -> &dyn AsyncQueryStore {
        self.db.as_ref()
    }

    /// The JWT signing key used to mint tokens.
    #[must_use]
    pub fn signing_key(&self) -> &JwtSigningKey {
        &self.signing_key
    }

    /// The verifier used to authenticate incoming tokens.
    #[must_use]
    pub fn verifier(&self) -> &JwtVerifier {
        &self.verifier
    }
}

fn build_verifier(signing_key: &JwtSigningKey) -> Result<JwtVerifier, foundation_auth::shared::jwt::JwtError> {
    let pem = signing_key.public_key_pem()?;
    JwtVerifier::from_config(JwtVerifierConfig {
        allowed_algorithms: vec![JwtAlgorithm::EdDSA, JwtAlgorithm::RS256, JwtAlgorithm::ES256],
        issuer: Some(ISSUER.to_string()),
        audience: None,
        key_id: None,
        public_key: PublicKeySource::Pem(pem),
    })
}
