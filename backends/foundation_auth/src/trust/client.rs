//! The trust store types: one enrolled client, one secret use and the trait
//! that every backend implements.

use core::future::Future;
use core::pin::Pin;

use futures_core::Stream;
use serde::{Deserialize, Serialize};

/// The future that a trust-store method returns.
pub type TrustFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, TrustError>> + Send + 'a>>;

/// A stream of enrolled clients, one at a time.
pub type TrustedClientStream =
    Pin<Box<dyn Stream<Item = Result<TrustedClient, TrustError>> + Send>>;

/// One enrolled client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedClient {
    /// Lowercase hexadecimal SHA-256 of the client certificate.
    pub fingerprint_sha256: String,
    /// The `client_id` that enrollment gave. It is the `sub` of an access
    /// secret that the client uses, so the store can find the client by ID.
    /// A client certificate of the proof of concept has no ID.
    #[serde(default)]
    pub client_id: Option<String>,
    /// The common name of the certificate.
    pub client_name: String,
    /// When the certificate was issued (Unix seconds).
    pub enrolled_at: u64,
    /// When the certificate expires (Unix seconds).
    pub expires_at: u64,
    /// The last successful call (Unix seconds).
    pub last_seen_at: Option<u64>,
    /// When the client was revoked (Unix seconds).
    pub revoked_at: Option<u64>,
}

impl TrustedClient {
    /// Not revoked and not expired at `now` (Unix seconds).
    #[must_use]
    pub fn is_active_at(&self, now: u64) -> bool {
        self.revoked_at.is_none() && now < self.expires_at
    }
}

/// The public key of a client-key enrollment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientKeyRecord {
    /// The algorithm name: `ed25519` or `p256`.
    pub alg: String,
    /// The raw public key bytes.
    pub public_key: Vec<u8>,
}

/// One use of a secret to record.
#[derive(Debug, Clone, Copy)]
pub struct SecretUse<'a> {
    /// The identifier of the secret.
    pub secret_id: &'a str,
    /// The end of the secret's validity window (Unix seconds).
    pub valid_until: u64,
    /// The maximum number of uses.
    pub max_uses: u32,
    /// The current time (Unix seconds).
    pub now: u64,
}

/// The result of one use of a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseOutcome {
    /// The use was counted; this is the total number of uses.
    Counted {
        /// The number of uses after this one.
        uses: u32,
    },
    /// The secret has no use left.
    Spent,
}

/// The facts that a trust store reports about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustStoreInfo {
    /// The name of the backend, for a log line.
    pub name: &'static str,
    /// Whether [`TrustStore::consume_secret_use`] is atomic across processes.
    pub atomic_across_processes: bool,
}

/// A durable record of enrolled clients, secret uses and revoked secrets.
pub trait TrustStore: Send + Sync + 'static {
    /// Create the tables (Structured Query Language, SQL) or do nothing (a
    /// key-value store).
    fn init(&self) -> TrustFuture<'_, ()>;

    /// Every client, one at a time.
    fn clients(&self) -> TrustFuture<'_, TrustedClientStream>;

    /// The client with this certificate fingerprint.
    fn client<'a>(&'a self, fingerprint: &'a str) -> TrustFuture<'a, Option<TrustedClient>>;

    /// The client with this `client_id`, the `sub` of an access secret.
    fn client_by_id<'a>(&'a self, client_id: &'a str) -> TrustFuture<'a, Option<TrustedClient>>;

    /// Record a newly issued client certificate.
    fn record_enrollment<'a>(
        &'a self,
        client: &'a TrustedClient,
        cert_pem: &'a str,
    ) -> TrustFuture<'a, ()>;

    /// Record the public key of a client-key enrollment.
    fn record_client_key<'a>(
        &'a self,
        client_id: &'a str,
        key: &'a ClientKeyRecord,
    ) -> TrustFuture<'a, ()>;

    /// The public key of a client-key enrollment.
    fn client_key<'a>(&'a self, client_id: &'a str) -> TrustFuture<'a, Option<ClientKeyRecord>>;

    /// Record one refresh nonce. The result is `true` for the first use of the
    /// nonce, and `false` for a replay.
    fn consume_refresh_nonce<'a>(
        &'a self,
        client_id: &'a str,
        nonce: &'a [u8],
        at: u64,
    ) -> TrustFuture<'a, bool>;

    /// Revoke a client at `at` (Unix seconds). The result says whether a client
    /// changed from active to revoked.
    fn revoke_client<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, bool>;

    /// Record a successful call at `at` (Unix seconds).
    fn mark_seen<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, ()>;

    /// Count one use of a secret. The count must be atomic: two calls with the
    /// last free use cannot both get [`UseOutcome::Counted`].
    fn consume_secret_use<'a>(&'a self, one_use: SecretUse<'a>) -> TrustFuture<'a, UseOutcome>;

    /// Revoke a secret at `at` (Unix seconds). The result says whether the
    /// secret changed from not revoked to revoked.
    fn revoke_secret<'a>(
        &'a self,
        secret_id: &'a str,
        valid_until: u64,
        at: u64,
    ) -> TrustFuture<'a, bool>;

    /// Whether the secret is revoked.
    fn is_secret_revoked(&self, secret_id: &str) -> TrustFuture<'_, bool>;

    /// Delete use and revocation rows whose secret expired before `cutoff`
    /// (Unix seconds). Returns the number of deleted rows.
    fn prune(&self, cutoff: u64) -> TrustFuture<'_, u64>;

    /// The facts about this store.
    fn info(&self) -> TrustStoreInfo;
}

/// Why a trust-store operation failed.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum TrustError {
    /// The backend failed.
    #[display("storage error: {_0}")]
    Storage(#[error(not(source))] String),
    /// The client already exists.
    #[display("the client already exists: {_0}")]
    Duplicate(#[error(not(source))] String),
    /// The secret has no use left.
    #[display("the secret is spent")]
    Spent,
    /// The secret is revoked.
    #[display("the secret is revoked")]
    SecretRevoked,
    /// The client that an access secret names (`sub`) is revoked.
    #[display("the client is revoked")]
    ClientRevoked,
    /// The client that an access secret names (`sub`) is not enrolled.
    #[display("the client is not enrolled: {_0}")]
    UnknownClient(#[error(not(source))] String),
    /// The store cannot count uses safely across processes.
    #[display("this store cannot count uses safely across processes")]
    NotAtomic,
}
