# The CA, the trust store and the TLS verifier

_Part of [spec 87](./requirements.md). Written in ASD-STE100 Simplified Technical English
(STE). Code and identifiers stay as they are._

**Revision 2 (2026-11-15).** The owner moved the mTLS code out of spec 81 into this
specification. [requirements.md](./requirements.md) is the authority. Where this file and
requirements.md disagree, requirements.md wins. The changes are:

| This file says | Read now |
|---|---|
| Spec 81 owns the CA, the trust store and the verifier | Spec 87 owns them |
| `foundation_core::env::Env` for entropy | `Ctx::require_env()?.entropy()` (spec 84) |
| `foundation_core::env::Env` for the time | `ctx.clock()` (spec 76); the `Env` of spec 84 has no clock |
| `foundation_compact::clock::Clock` and `unix_secs` | `foundation_context::Clock` and `foundation_compact::unix_seconds` |
| The section numbers 5, 6 and 7 of `auth.md` | The sections 1, 2 and 3 of this file |
| The mTLS provider is one of the spec 81 providers | The mTLS provider implements `ClientAuth` of spec 86 |
| The task numbers T8, T9 and T10 of spec 81 | The tasks T1, T2 and T3 of this specification |

The rest of the text is unchanged from `auth.md` sections 5 to 7.

## 1. The CA and certificates (`foundation_auth::pki`, native)

### 1.1 The CA

```rust
// foundation_auth/src/pki/mod.rs
use std::sync::Arc;

use foundation_context::{Ctx, Clock};
use foundation_compact::Duration;

/// The key algorithm of the CA and of the certificates it makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAlgorithm {
    /// ECDSA P-256 (default). Every TLS client supports it.
    EcdsaP256,
    /// Ed25519. Deterministic signatures (DST). Browsers do not accept
    /// Ed25519 server certificates, so use it only for tests and DST.
    Ed25519,
}

#[derive(Debug, Clone)]
pub struct CaConfig {
    pub common_name: String,
    /// The algorithm that the operator's key must have (spec 81 feature 00, section 4.1).
    pub key_algorithm: KeyAlgorithm,
    /// Default 10 years: the mkcert root lifetime (as today).
    pub ca_lifetime: Duration,
    /// How far back `not_before` goes, for clients whose clock is behind.
    /// Default 5 minutes (as today; MIT Kerberos `clockskew`).
    pub backdate: Duration,
}

/// Issues the server certificate and signs client CSRs.
pub struct CertificateAuthority {
    cert: rcgen::Certificate,
    cert_pem: String,
    key: rcgen::KeyPair,     // a remote key pair over DeterministicSigner
    /// The time source (spec 84). Not the system clock: a DST run must cross
    /// a validity window at a known point.
    clock: Arc<dyn Clock>,
    config: CaConfig,
}

impl CertificateAuthority {
    /// The CA over the operator's CA key (spec 81 feature 00, section 4.5). Reuses
    /// `existing_cert_pem` (today's `ca-cert.pem`) when its public key is
    /// the key's public key. Otherwise it makes a new self-signed
    /// certificate, with dates from `ctx.clock()`, and returns its PEM so that
    /// the host writes it.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn from_key(key: &PrivateKey, existing_cert_pem: Option<&str>, config: &CaConfig, ctx: &Ctx) -> Result<(Self, Option<String>), PkiError> { todo!("T1") }

    /// A server certificate for `names` over the operator's server key.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn issue_server_identity(&self, server_key: &PrivateKey, names: &[String], lifetime: Duration) -> Result<ServerIdentity, PkiError> { todo!("T1") }

    /// Sign a client CSR. Only the public key of the CSR is used. The
    /// subject is `client_name`, the certificate is a non-CA leaf with the
    /// `clientAuth` extended key usage, and the lifetime is ours (as today).
    ///
    /// # Errors
    /// [`PkiError::BadCsr`] for an invalid CSR or name.
    pub fn sign_client_csr(&self, csr_pem: &str, client_name: &str, lifetime: Duration) -> Result<IssuedClientCert, PkiError> { todo!("T1") }

    #[must_use]
    pub fn ca_cert_pem(&self) -> &str { &self.cert_pem }
}
```

### 1.2 Deterministic signing

`rcgen` signs ECDSA with a random nonce from `ring`. That breaks DST, and `ring` does not
let us pass our own random source. `rcgen` has a hook for this: `RemoteKeyPair`. We sign
with our own key there.

```rust
// foundation_auth/src/pki/signer.rs
/// Signs certificates with a key that came from the `Ctx` entropy (spec 84).
pub struct DeterministicSigner {
    kind: SignerKind,
    public_key: Vec<u8>,
}

enum SignerKind {
    /// RFC 6979: the ECDSA nonce comes from the key and the message.
    P256(p256::ecdsa::SigningKey),
    /// RFC 8032: Ed25519 has no nonce.
    Ed25519(ed25519_compact::KeyPair),
}

impl rcgen::RemoteKeyPair for DeterministicSigner {
    fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    fn sign(&self, msg: &[u8]) -> Result<Vec<u8>, rcgen::Error> {
        match &self.kind {
            SignerKind::P256(key) => {
                let sig: p256::ecdsa::Signature = p256::ecdsa::signature::Signer::sign(key, msg);
                Ok(sig.to_der().as_bytes().to_vec()) // X.509 wants the DER form
            }
            SignerKind::Ed25519(pair) => Ok(pair.sk.sign(msg, None).to_vec()),
        }
    }

    fn algorithm(&self) -> &'static rcgen::SignatureAlgorithm {
        match self.kind {
            SignerKind::P256(_) => &rcgen::PKCS_ECDSA_P256_SHA256,
            SignerKind::Ed25519(_) => &rcgen::PKCS_ED25519,
        }
    }
}
```

What the code does: `rcgen` builds the certificate and asks this signer for the signature.
P-256 signatures use RFC 6979, so the same key and message give the same signature. With the
simulated clock and the seeded entropy of spec 84, the same seed gives byte-identical
certificates and fingerprints. The certificate serial number is already a hash of the public
key, so it is also stable.

### 1.3 The server identity keeps its fingerprint

Today `serve` makes a new server certificate on each start. So the pinned fingerprint in
every payload stops working after a restart. We fix this:

1. The host reads `<state>/server-cert.pem`. The server key is the operator's file (spec 81 feature 00, section 4.5).
2. It reuses the certificate when its public key is the server key's public key, it covers the configured names, and `now < not_after - renew_before`.
3. `renew_before` is one third of the lifetime (30 days of 90). This is when certbot renews a 90-day certificate.
4. Otherwise it makes a new certificate with `CertificateAuthority::issue_server_identity` and writes `server-cert.pem`. The operator must hand out new payloads then. The host logs the new fingerprint.
5. Enrolled clients do not need the pin. They trust the CA, so a renewal does not affect them.

```rust
// foundation_auth/src/pki/identity.rs
/// The configuration of the server certificate (rule 8).
#[derive(Debug, Clone)]
pub struct ServerIdentityConfig {
    /// DNS names and IP addresses in the certificate. Default: "localhost",
    /// "127.0.0.1" and the host of the bind address (as today).
    pub names: Vec<String>,
    /// Default 90 days: the lifetime of a Let's Encrypt certificate.
    pub lifetime: Duration,
    /// Make a new certificate when less than this time is left. Default
    /// `lifetime / 3` (30 days of 90), when certbot renews.
    pub renew_before: Duration,
}
```

What the code does: it holds the three values that steps 1 to 4 use. The host reads them
from its configuration.

## 2. Trusted clients (`foundation_auth::trust`)

### 2.1 The trait

```rust
// foundation_auth/src/trust/mod.rs
use core::future::Future;
use core::pin::Pin;

use futures_core::Stream;

pub type TrustFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, TrustError>> + Send + 'a>>;
pub type TrustedClientStream = Pin<Box<dyn Stream<Item = Result<TrustedClient, TrustError>> + Send>>;

/// One enrolled client (unchanged fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedClient {
    pub fingerprint_sha256: String,
    pub client_name: String,
    pub enrolled_at: u64,
    pub expires_at: u64,
    pub last_seen_at: Option<u64>,
    pub revoked_at: Option<u64>,
}

/// One use of a secret to record.
#[derive(Debug, Clone, Copy)]
pub struct SecretUse<'a> {
    pub secret_id: &'a str,
    pub valid_until: u64,
    pub max_uses: u32,
    pub now: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseOutcome {
    Counted { uses: u32 },
    Spent,
}

/// Durable record of enrolled clients, secret uses and revoked secrets.
pub trait TrustStore: Send + Sync + 'static {
    /// Create tables (SQL) or nothing (key-value).
    fn init(&self) -> TrustFuture<'_, ()>;
    /// Every client, one at a time (never a whole `Vec`).
    fn clients(&self) -> TrustFuture<'_, TrustedClientStream>;
    fn client<'a>(&'a self, fingerprint: &'a str) -> TrustFuture<'a, Option<TrustedClient>>;
    fn record_enrollment<'a>(&'a self, client: &'a TrustedClient, cert_pem: &'a str) -> TrustFuture<'a, ()>;
    fn revoke_client<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, bool>;
    fn mark_seen<'a>(&'a self, fingerprint: &'a str, at: u64) -> TrustFuture<'a, ()>;
    /// Count one use. Must be atomic: two calls with the last free use
    /// cannot both get `Counted`.
    fn consume_secret_use<'a>(&'a self, one_use: SecretUse<'a>) -> TrustFuture<'a, UseOutcome>;
    fn revoke_secret<'a>(&'a self, secret_id: &'a str, valid_until: u64, at: u64) -> TrustFuture<'a, bool>;
    fn is_secret_revoked<'a>(&'a self, secret_id: &'a str) -> TrustFuture<'a, bool>;
    /// Delete use and revocation rows whose secret expired before `cutoff`.
    fn prune<'a>(&'a self, cutoff: u64) -> TrustFuture<'a, u64>;
    fn info(&self) -> TrustStoreInfo;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustStoreInfo {
    pub name: &'static str,
    /// `consume_secret_use` is atomic across processes.
    pub atomic_across_processes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum TrustError {
    #[display("storage error: {_0}")]
    Storage(#[error(not(source))] String),
    #[display("the client already exists: {_0}")]
    Duplicate(#[error(not(source))] String),
    #[display("the secret is spent")]
    Spent,
    #[display("the secret is revoked")]
    SecretRevoked,
    #[display("this store cannot count uses safely across processes")]
    NotAtomic,
}
```

### 2.2 `SqlTrustStore<Q: QueryStore>`

It keeps the SQL of `trust.rs` of today and the same table names (the prefix is
configuration; the default `agent_rpc_` reads existing databases). One table is new:

```sql
-- existing (unchanged)
CREATE TABLE IF NOT EXISTS agent_rpc_trusted_clients (
    fingerprint TEXT PRIMARY KEY,
    client_name TEXT NOT NULL,
    cert_pem TEXT NOT NULL,
    enrolled_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    last_seen_at INTEGER,
    revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS agent_rpc_bootstrap_secret_uses (
    secret_id TEXT PRIMARY KEY,
    uses INTEGER NOT NULL,
    valid_until INTEGER NOT NULL
);
-- new (07)
CREATE TABLE IF NOT EXISTS agent_rpc_revoked_secrets (
    secret_id TEXT PRIMARY KEY,
    revoked_at INTEGER NOT NULL,
    valid_until INTEGER NOT NULL
);
```

`consume_secret_use` keeps the one-statement upsert of today:

```sql
INSERT INTO agent_rpc_bootstrap_secret_uses (secret_id, uses, valid_until)
VALUES (?, 1, ?)
ON CONFLICT(secret_id) DO UPDATE SET uses = uses + 1 WHERE uses < ?
```

One statement is atomic on SQLite, Turso, libsql and D1. So `atomic_across_processes` is
true. The `DELETE` of expired rows moves out of this call into `prune`, which the host runs
on a timer (`TrustConfig::prune_interval`).

The SQL is portable to Postgres except the `?` placeholders. foundation_db has no Postgres
backend (owner decision 7 in [feature.md](../../83-agent-rpc-serving-system/agent-rpc-architecture.md): a separate feature).

### 2.3 `KvTrustStore<K: KeyValueStore>`

For backends without SQL: `MemoryStorage`, `MemoryJsonStore`, `JsonFileStorage`.

- Keys: `trust/v1/client/<fingerprint>`, `trust/v1/use/<secret_id>`, `trust/v1/revoked/<secret_id>`.
- `consume_secret_use` takes an async lock inside the process, then reads and writes. So it is atomic only inside one process: `atomic_across_processes = false`.
- `TrustedClients::open` refuses a store with `atomic_across_processes = false` when `TrustConfig::shared_store` is true.
- Never use it over Workers KV: Workers KV is eventually consistent, so a secret could be spent twice. The Worker host uses D1 (`SqlTrustStore<D1WasmStorage>`).

### 2.4 `TrustedClients`: the store plus an index

```rust
// foundation_auth/src/trust/clients.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use foundation_context::{Ctx, Clock};
use foundation_compact::Duration;

/// How the in-memory index works.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexMode {
    /// Load every client at open. The TLS verifier reads it with no I/O.
    /// For one native process (the default).
    Preload,
    /// Like `Preload`, and reload every interval. For native replicas that
    /// share a database.
    Refresh(Duration),
    /// No index: every check reads the store. For Workers, whose isolates
    /// are short-lived and many.
    StoreOnly,
}

/// Limits and intervals (rule 8).
#[derive(Debug, Clone)]
pub struct TrustConfig {
    pub index: IndexMode,
    /// Write `last_seen_at` at most once per interval per client. Default
    /// 75 s: nginx's default `keepalive_timeout`, so one idle keep-alive
    /// connection makes at most one write.
    pub seen_write_interval: Duration,
    /// Keep use and revocation rows this long after a secret expires.
    /// Default 5 minutes (equal to the secret clock skew).
    pub expired_grace: Duration,
    /// How often `prune` runs. Default 1 hour.
    pub prune_interval: Duration,
    /// Other processes write the same store.
    pub shared_store: bool,
}

/// The clients that the TLS verifier and the auth providers check.
pub struct TrustedClients {
    store: Arc<dyn TrustStore>,
    index: TrustIndex,
    clock: Arc<dyn Clock>,
    config: TrustConfig,
}

/// A cheap handle to the in-memory map, for the sync TLS verifier.
#[derive(Clone, Debug, Default)]
pub struct TrustIndex(Arc<RwLock<HashMap<String, TrustedClient>>>);

impl TrustIndex {
    /// A read lock and a clone. No I/O, no task, no `await`.
    #[must_use]
    pub fn get(&self, fingerprint: &str) -> Option<TrustedClient> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(fingerprint)
            .cloned()
    }

    /// Called only after the store write succeeded.
    pub(crate) fn update_last_seen(&self, fingerprint: &str, at: u64) {
        if let Some(client) = self
            .0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(fingerprint)
        {
            client.last_seen_at = Some(at);
        }
    }
}

impl TrustedClients {
    /// Initialize the store and fill the index (stream, one row at a time).
    ///
    /// # Errors
    /// [`TrustError`].
    pub async fn open(store: Arc<dyn TrustStore>, ctx: &Ctx, config: TrustConfig) -> Result<Arc<Self>, TrustError> { todo!("T2") }

    /// The index handle for `TrustedClientVerifier`.
    #[must_use]
    pub fn index(&self) -> TrustIndex { self.index.clone() }

    /// The client, from the index or (`StoreOnly`) from the store.
    pub async fn get(&self, fingerprint: &str) -> Result<Option<TrustedClient>, TrustError> { todo!("T2") }

    pub async fn record_enrollment(&self, client: TrustedClient, cert_pem: &str) -> Result<(), TrustError> { todo!("T2") }

    /// Revoke: the store first, then the index.
    pub async fn revoke_client(&self, fingerprint: &str) -> Result<bool, TrustError> { todo!("T2") }

    /// Record a seen client at most once per `seen_write_interval`.
    /// Store errors are returned to the caller, never only logged.
    pub async fn touch(&self, fingerprint: &str) -> Result<(), TrustError> {
        let now = foundation_compact::unix_seconds(self.clock.wall_now());
        let due = self.index.get(fingerprint).map_or(true, |c| {
            c.last_seen_at.map_or(true, |seen| now.saturating_sub(seen) >= self.config.seen_write_interval.as_secs())
        });
        if !due {
            return Ok(());
        }
        self.store.mark_seen(fingerprint, now).await?;
        self.index.update_last_seen(fingerprint, now);
        Ok(())
    }

    /// Spend one use of a verified secret.
    ///
    /// # Errors
    /// [`TrustError::Spent`] when no use is left.
    pub async fn spend_secret(&self, secret: &VerifiedSecret, max_uses: u32) -> Result<(), TrustError> { todo!("T2") }

    /// # Errors
    /// [`TrustError::SecretRevoked`].
    pub async fn check_secret_not_revoked(&self, secret_id: &str) -> Result<(), TrustError> { todo!("T2") }

    pub async fn revoke_secret(&self, secret_id: &str, valid_until: u64) -> Result<bool, TrustError> { todo!("T2") }
}
```

What the code does: `TrustedClients` wraps a `TrustStore`. On a native host it keeps an
index in memory, so the TLS verifier needs no database call. `touch` writes the last-seen
time at most once per 75 seconds per client, and it returns store errors.

## 3. TLS verification (`foundation_auth::mtls`, native)

### 3.1 The verifier is a pure check

Today `TrustedClientVerifier` starts a valtron task for `mark_seen` on each handshake
(`auth.rs` lines 630–639). The owner asked to remove that. The rustls verifier is a sync
trait, so it must stay a pure check over the in-memory index.

Before (today):

```rust
// backends/foundation_agent_rpc/src/auth.rs (today)
Some(client) if client.is_active_at(now) => {
    let trusted = Arc::clone(&self.trusted);
    let seen = async move {
        if let Err(e) = trusted.mark_seen(&fingerprint, now).await {
            tracing::warn!(error = %e.current_context(), "recording a client handshake failed");
        }
    };
    if let Err(e) = foundation_core::valtron::send(foundation_core::valtron::from_future(seen)) {
        tracing::warn!(error = %e, "scheduling the last-seen update failed");
    }
    Ok(ClientCertVerified::assertion())
}
```

After:

```rust
// foundation_auth/src/mtls/verifier.rs
/// WebPKI chain check to the CA, then the trusted-client index. No I/O,
/// no task, no write: rustls calls this in the middle of the handshake.
#[derive(Debug)]
pub struct TrustedClientVerifier {
    chain: Arc<dyn ClientCertVerifier>,
    index: TrustIndex,
}

impl ClientCertVerifier for TrustedClientVerifier {
    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        self.chain.verify_client_cert(end_entity, intermediates, now)?;
        let fingerprint = sha256_hex(end_entity);
        match self.index.get(&fingerprint) {
            Some(client) if client.is_active_at(now.as_secs()) => Ok(ClientCertVerified::assertion()),
            Some(client) if client.revoked_at.is_some() => {
                Err(rustls::Error::InvalidCertificate(rustls::CertificateError::Revoked))
            }
            Some(_) => Err(rustls::Error::InvalidCertificate(rustls::CertificateError::Expired)),
            None => Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::ApplicationVerificationFailure,
            )),
        }
    }
    // offer_client_auth, client_auth_mandatory, root_hint_subjects and the
    // two signature checks delegate to `chain`, as today.
}
```

The last-seen write moves to `MtlsAuth` (section 4 of this file). It runs in the async call path,
after the handshake, with `.await`, and it returns its error.

### 3.2 The rustls time comes from the `Ctx` clock

rustls asks a `TimeProvider` for the time when it checks certificate dates. We give it the
`Ctx` clock (spec 76), so certificate expiry follows the injected time too:

```rust
// foundation_auth/src/mtls/time.rs
#[derive(Debug)]
pub struct CtxTimeProvider(pub Arc<dyn foundation_context::Clock>);

impl rustls::time_provider::TimeProvider for CtxTimeProvider {
    fn current_time(&self) -> Option<rustls::pki_types::UnixTime> {
        // `unix_seconds` returns a negative value before the epoch, and rustls
        // has no time before the epoch: `None` makes rustls reject the certificate.
        let secs = foundation_compact::unix_seconds(self.0.wall_now());
        let secs = u64::try_from(secs).ok()?;
        Some(rustls::pki_types::UnixTime::since_unix_epoch(core::time::Duration::from_secs(secs)))
    }
}
```

`TimeProvider` lives in `rustls` and is object-safe, so a test swaps in a fixed time. The
config builders take the `clock` from `ctx.clock()` and wrap it:

```rust
fn time_provider(ctx: &Ctx) -> Arc<dyn rustls::time_provider::TimeProvider> {
    Arc::new(CtxTimeProvider(ctx.clock()))
}
```

The config builders use `rustls::ServerConfig::builder_with_details(provider, time_provider)`.

### 3.3 Config builders

```rust
// foundation_auth/src/mtls/config.rs
/// A server config that requires a client certificate from `ca` that is in `index`.
pub fn server_config_mutual(identity: &ServerIdentity, ca: &CertificateAuthority, index: TrustIndex, ctx: &Ctx)
    -> Result<Arc<rustls::ServerConfig>, PkiError> { todo!("T3") }

/// A server config with no client certificate (the enrollment listener).
pub fn server_config_server_auth(identity: &ServerIdentity, ctx: &Ctx)
    -> Result<Arc<rustls::ServerConfig>, PkiError> { todo!("T3") }

/// A client config that trusts exactly one server certificate (pinned SHA-256).
pub fn client_config_pinned(fingerprint_sha256: &str, ctx: &Ctx)
    -> Result<Arc<rustls::ClientConfig>, PkiError> { todo!("T3") }

/// A client config with a client certificate that trusts `ca_pem`.
pub fn client_config_mutual(ca_pem: &str, cert_pem: &str, key_pem: &str, ctx: &Ctx)
    -> Result<Arc<rustls::ClientConfig>, PkiError> { todo!("T3") }
```

### 3.4 The host must report the peer certificate

`MtlsAuth` needs the client certificate in the call context. `foundation_netio` already has
the field: `ConnectionContext::tls: Option<TlsInfo { peer_certificates, server_name }>`. But
no code fills it today. Task T4 fills it:

1. `RustlsStream<ServerConnection>` gets `fn tls_info(&self) -> Option<TlsInfo>`. It reads `peer_certificates()` and `server_name()`.
2. `foundation_http`'s `ProtocolDetectHandler` peeks the first bytes of a connection. A TLS stream returns bytes only after the handshake. So after the first successful peek, the handler builds a new `Arc<ConnectionContext>` with `tls: Some(…)` and gives it to the HTTP/1.1 or HTTP/2 handler.
3. The RPC dispatcher already copies `request.connection` into `RequestContext::connection`.

## 4. The mTLS providers (`foundation_auth::client_auth`)

```rust
// foundation_auth/src/client_auth/mtls.rs
pub struct MtlsAuth {
    trust: Arc<TrustedClients>,
    /// From `ctx.clock()` in `new` (spec 86: a provider takes its `Ctx` at
    /// construction).
    clock: Arc<dyn Clock>,
    on_seen_error: SeenErrorPolicy,   // default Fail: the error reaches the caller
}

impl ClientAuth for MtlsAuth {
    fn mode(&self) -> &'static str { "mtls" }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        Box::pin(async move {
            let Some(leaf) = call.connection.tls.as_ref().and_then(|t| t.peer_certificates.first()) else {
                return Ok(None);
            };
            let fingerprint = sha256_hex(leaf);
            let client = self.trust.get(&fingerprint).await.map_err(trust_error_to_auth)?
                .ok_or_else(|| AuthError::Invalid("certificate not enrolled here".into()))?;
            let now = foundation_compact::unix_seconds(self.clock.wall_now());
            if client.revoked_at.is_some() { return Err(AuthError::Revoked); }
            if now >= client.expires_at { return Err(AuthError::Expired); }
            // The owner's direction: last-seen is written here, after the
            // handshake, with `.await`, and its error is returned.
            if let Err(e) = self.trust.touch(&fingerprint).await {
                match self.on_seen_error {
                    SeenErrorPolicy::Fail => return Err(trust_error_to_auth(e)),
                    SeenErrorPolicy::Log => tracing::warn!(error = %e, "recording last-seen failed"),
                }
            }
            Ok(Some(auth_info_for_certificate(&client)))
        })
    }
}
```

`MtlsAuth` runs on every call, not only at the handshake. So a revocation stops the next
call on a live keep-alive connection. This closes the gap "revocation does not drop live
keep-alive connections" from the progress notes. A turn that already runs continues to its
end. `TurnLimits::max_duration` bounds that time.

```rust
// foundation_auth/src/client_auth/cloudflare.rs
/// The client-certificate facts that Cloudflare gives a Worker in
/// `request.cf.tlsClientAuth`. The Worker host copies them into the request
/// extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfTlsClientAuth {
    pub cert_presented: bool,
    /// "SUCCESS", "FAILED:<reason>" or "NONE".
    pub cert_verified: String,
    /// Lowercase hex SHA-256 of the certificate (the same form as `sha256_hex`).
    pub cert_fingerprint_sha256: String,
    pub cert_subject_dn: String,
    pub cert_not_after: String,
    pub cert_serial: String,
}

pub struct CfMtlsAuth {
    trust: Arc<TrustedClients>,
    /// The certificate must also be in the trust store (not only signed by
    /// the CA of the Cloudflare zone). Default true.
    require_registered: bool,
}
```

`CfMtlsAuth` returns `Ok(None)` when no `CfTlsClientAuth` is present or `cert_presented` is
false. It returns `Invalid` when `cert_verified` is not `SUCCESS`. With
`require_registered`, it looks up the fingerprint like `MtlsAuth`.
