# Authentication in `foundation_auth`

_Part of [feature 07](./feature.md). Written in ASD-STE100 Simplified Technical English (STE).
Code and identifiers stay as they are._

**Abbreviations in this file.** authentication (auth); remote procedure call (RPC);
Certificate Authority (CA); certificate signing request (CSR); Transport Layer Security
(TLS); mutual TLS (mTLS), where both sides show a certificate; JavaScript Object Notation
(JSON); JSON Web Token (JWT); Public-Key Cryptography Standards number 8 (PKCS#8), a file
format for private keys; Privacy-Enhanced Mail (PEM), a text form of keys and
certificates; Distinguished Encoding Rules (DER), a binary form; Secure Hash Algorithm with
256 bits (SHA-256); Rivest–Shamir–Adleman (RSA); Probabilistic Signature Scheme (PSS);
Elliptic Curve Digital Signature Algorithm (ECDSA); authenticated encryption with associated
data (AEAD); key encryption key (KEK); identifier (ID); uniform resource locator (URL);
Durable Object (DO); WebAssembly (wasm); Structured Query Language (SQL); key-value store
(KV); input/output (I/O); deterministic simulation testing (DST); Request for Comments (RFC);
application programming interface (API); central processing unit (CPU); Hypertext Transfer
Protocol (HTTP); additional authenticated data (AAD), the context bytes that AEAD binds to a
sealed blob; Representational State Transfer (REST); Domain Name System (DNS); Internet
Protocol (IP); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.
D1 and R2 are names of Cloudflare products (a SQL database and an object store), not
abbreviations.
The [glossary in feature.md](./feature.md#2-glossary) explains each term.

## 1. What moves, and why

The owner asked for one auth library that every service can use. Today the auth code of
agent-rpc lives in `backends/foundation_agent_rpc/src/{auth,secret,trust}.rs`. Nothing in it
is specific to agents. We move it into `foundation_auth` and build it on `foundation_db`.
agent-rpc keeps only its wiring: the enrollment service and the listeners.

| Today (branch `65-f01-agent-rpc`) | After this feature | Notes |
|---|---|---|
| `secret.rs` `SignedSecret` | `foundation_auth::secret::SignedSecret` | Same version 1 wire format. Adds version 2. |
| `secret.rs` `SigningKey`, `SigningAlgorithm` | `foundation_auth::secret::{SigningKey, VerifyingKey, SigningAlgorithm}` | Ed25519 moves from `ring` to `ed25519-compact`, which also builds for wasm. Signatures are identical (RFC 8032). |
| `secret.rs` `BootstrapSecrets` (issue + verify) | `foundation_auth::secret::{SecretIssuer, SecretVerifier}` | Split: the verifier needs only public keys. |
| `secret.rs` `SigningKeyConfig`, `default_state_dir`, `write_secret_file` | `foundation_auth::keystore::FsKeyStore`; `foundation_agent_rpc::host::native::state_dir` | The key source becomes a `KeyStore`. |
| `auth.rs` `CertificateAuthority`, `IssuedClientCert`, `ServerIdentity` | `foundation_auth::pki` (native) | Time and randomness come from `Env`. |
| `auth.rs` `CertificateAuthority::load_or_generate` | `foundation_auth::keystore::KeyMaterial::certificate_authority` | Any `KeyStore`, not only a directory. |
| `auth.rs` `sha256_fingerprint` | `foundation_auth::fingerprint::sha256_hex` | Shared helper. |
| `auth.rs` `TrustedClientVerifier` | `foundation_auth::mtls::TrustedClientVerifier` (native) | Pure check. No `mark_seen`, no task. |
| `auth.rs` `tls_listeners` | `foundation_auth::mtls::{server_config_mutual, server_config_server_auth}` + `host::native::tls` | Config builders move; the listeners stay in agent-rpc. |
| `client.rs` `PinnedCertVerifier` | `foundation_auth::mtls::PinnedServerVerifier` (native) | Same logic. |
| `trust.rs` `TrustedClientStore` (SQL only) | `foundation_auth::trust::{TrustStore, SqlTrustStore, KvTrustStore, TrustedClients}` | A trait with SQL and key-value implementations. |
| `server.rs` `Auth` enum | `foundation_auth::client_auth::ClientAuth` trait + providers | The handlers see only `AuthInfo`. |
| `foundation_connectrpc::AuthInfo` | `foundation_auth::client_auth::AuthInfo`, re-exported by connectrpc | Same type, same path for connectrpc users. |
| `auth.rs` `EnrollmentService` | `foundation_agent_rpc::core::enroll::EnrollmentService` | Stays in agent-rpc. It is wiring. |

## 2. Module layout and feature gates

```text
backends/foundation_auth/src/
├── fingerprint.rs        sha256_hex                                    all targets
├── secret/               SignedSecret, SecretIssuer, SecretVerifier    all targets (Ed25519)
│   ├── mod.rs                                                           RSA-PSS: native + feature "secret-rsa"
│   ├── codec.rs          version 1 and 2 JSON envelopes, signed message
│   ├── keys.rs           SigningKey, VerifyingKey, PKCS#8 parsing
│   ├── issuer.rs
│   └── verifier.rs
├── keystore/             KeyStore trait, KeyRecord, KeyMaterial        all targets
│   ├── mod.rs
│   ├── sealing.rs        KeySealer, KekSealer, PassphraseSealer        all targets
│   ├── db.rs             DbKeyStore<K: KeyValueStore>             all targets
│   ├── env_source.rs     EnvKeyStore (read-only)                       all targets
│   ├── fs.rs             FsKeyStore                                    native only
│   └── memory.rs         MemoryKeyStore                                feature "testing"
├── pki/                  CertificateAuthority, DeterministicSigner     native + feature "pki"
├── trust/                TrustStore, TrustedClients, TrustIndex        all targets
│   ├── sql.rs            SqlTrustStore<Q: QueryStore>
│   ├── kv.rs             KvTrustStore<K: KeyValueStore>
│   └── memory.rs         MemoryTrustStore                              feature "testing"
├── mtls/                 TrustedClientVerifier, PinnedServerVerifier,  native + feature "mtls"
│                         rustls config builders, EnvTimeProvider
└── client_auth/          ClientAuth, CallCredentials, AuthInfo,        all targets
    ├── signed_secret.rs  SignedSecretAuth
    ├── mtls.rs           MtlsAuth (reads peer certificates that the host reports)
    ├── jwt.rs            JwtAuth
    ├── cloudflare.rs     CfTlsClientAuth, CfMtlsAuth
    ├── insecure.rs       InsecureNoAuth
    ├── forwarded.rs      ForwardedPrincipalAuth (inside a Durable Object)
    └── any_of.rs         AnyOf
```

Feature changes in `backends/foundation_auth/Cargo.toml`:

```toml
[features]
default = ["turso", "secret"]
# Signed secrets with Ed25519 (pure Rust, all targets).
secret = ["dep:ed25519-compact"]
# Operator-supplied RSA-PSS keys for signed secrets. `ring` is native only.
secret-rsa = ["secret", "dep:ring"]
# The CA and certificate issuance. `rcgen` + `x509-parser` verify CSRs with
# `ring`, so this is native only.
pki = ["secret", "dep:rcgen", "dep:p256", "dep:x509-parser", "dep:ring"]
# rustls verifiers and config builders. Native only.
mtls = ["pki", "dep:rustls"]
# Fakes for tests: MemoryKeyStore, MemoryTrustStore, scripted providers.
testing = ["foundation_compact/testing", "foundation_core/testing"]

[dependencies]
ed25519-compact = { version = "2.6", default-features = false, features = ["std", "pem"], optional = true }

[target.'cfg(not(target_family = "wasm"))'.dependencies]
ring = { version = "0.17", optional = true }
rcgen = { version = "0.13", default-features = false, features = ["pem", "x509-parser"], optional = true }
x509-parser = { version = "0.16", features = ["verify"], optional = true }
p256 = { version = "0.13", default-features = false, features = ["ecdsa", "pkcs8", "pem", "std"], optional = true }
rustls = { version = "0.23", default-features = false, features = ["std", "ring", "tls12"], optional = true }
```

What works on wasm (Cloudflare Workers, browsers):

| Module | wasm | Why |
|---|---|---|
| `secret` with Ed25519 | yes | `ed25519-compact` is pure Rust. Randomness comes from `Env`. |
| `secret` with RSA-PSS | no | `ring` is C code. A wasm host refuses an RSA secret with `SecretError::UnsupportedAlgorithm`. |
| `keystore` (`DbKeyStore`, `EnvKeyStore`, sealing) | yes | `foundation_db` async traits, `chacha20poly1305`, `argon2` are pure Rust. |
| `keystore::FsKeyStore` | no | No file system in a Worker. |
| `pki` | no | CSR verification uses `ring` through `x509-parser`. |
| `trust` | yes | `foundation_db` async traits only. |
| `mtls` | no | `rustls` with `ring`. A Worker has no TLS listener anyway: Cloudflare terminates TLS. |
| `client_auth` | yes | Pure logic over headers, connection facts and stores. |

## 3. Signed secrets (`foundation_auth::secret`)

A **signed secret** is a short string that proves that a key holder made it. It has 32
random bytes, a validity window and a signature. An operator gives it to a client, often
inside a `BootstrapPayload` (see [proto.md](./proto.md)). It is **our own format, not a
JWT.**

### 3.1 Version 1 wire format (unchanged)

The shared form is base64url (unpadded) of a JSON object with exactly these seven fields:

```json
{
  "v": 1,
  "alg": "ed25519",
  "public_key": "<base64url raw public key>",
  "secret": "<base64url 32 random bytes>",
  "valid_from": 1792000000,
  "valid_until": 1792086400,
  "signature": "<base64url signature>"
}
```

The signature does not cover the JSON text, because JSON has more than one way to write the
same object. It covers this byte string:

```text
"ewe.agent_rpc.bootstrap_secret.v1"   the domain and version, ASCII
0x00                                  one zero byte
u32be(len(alg))        alg            "ed25519" or "rsa-pss-sha256"
u32be(len(public_key)) public_key     raw bytes, not base64
u32be(len(secret))     secret         the 32 random bytes
u64be(valid_from)                     Unix seconds
u64be(valid_until)                    Unix seconds, exclusive
```

`u32be` is a 4-byte big-endian length. `u64be` is an 8-byte big-endian number. These are
exactly the bytes that `signed_message` makes in `secret.rs` today. A golden test keeps them
fixed (section 3.8).

### 3.2 Version 2 (new, additive)

Owner decision 2: the issuer makes **version 2 by default**. The verifier still accepts version 1.

Version 2 adds five fields:

| Field | Type | Meaning |
|---|---|---|
| `purpose` | `"enroll"` or `"access"` | What the secret is for. A version 1 secret is always `enroll`. |
| `aud` | string, optional | The audience: the base URL of the one host that accepts it. |
| `kid` | string, optional | The key ID. It selects the public key during key rotation. |
| `max_uses` | integer, optional | The most uses that the issuer allows. 0 or absent: no limit from the token (the server policy applies). |
| `sub` | string, optional | The subject: the `client_id` of an enrolled client. Set only on an access secret that enrollment or refresh issued (section 9.1). `SignedSecretAuth` then also checks that this client is not revoked. |

An **enrollment** secret always has `aud` set to the host's `url` from the bootstrap payload. So the
server address is inside the signature: a changed `url` in the payload no longer matches `aud`,
and the client stops (proto.md section 6.3).

A version 2 example (an access secret for a Worker):

```json
{
  "v": 2,
  "alg": "ed25519",
  "kid": "2026-10",
  "public_key": "<base64url raw public key>",
  "secret": "<base64url 32 random bytes>",
  "valid_from": 1792000000,
  "valid_until": 1794592000,
  "purpose": "access",
  "aud": "https://agents.example.com/rpc",
  "max_uses": 0,
  "signature": "<base64url signature>"
}
```

The version 2 signed message:

```text
"ewe.agent_rpc.bootstrap_secret.v2"
0x00
u32be(len) alg
u32be(len) kid           empty when absent
u32be(len) public_key
u32be(len) secret
u32be(len) purpose       "enroll" or "access"
u32be(len) aud           empty when absent
u64be(valid_from)
u64be(valid_until)
u32be(max_uses)          0 when absent
u32be(len) sub           empty when absent
```

Compatibility rules:

1. A new decoder reads version 1 and version 2.
2. The issuer writes version 1 when the request is `purpose = enroll` with no audience, no key ID and no use limit. That is the same secret as today, so old clients still read it.
3. The issuer writes version 2 in every other case.
4. An old decoder refuses version 2 with "version 2 (this build reads 1)". That is correct: an old client cannot use an access secret.
5. The decoder is strict. It refuses unknown fields (`deny_unknown_fields`), as today.
6. The decoder reads `v` first with a small probe type. Then it parses the strict type for that version.

**Why version 2.** Without `purpose`, an enrollment secret could also work as a bearer
credential on a host that shares the key. Without `aud`, a secret for host A could work on
host B. Both fields are inside the signature, so nobody can change them. Section 9.3 shows
how this blocks replay.

### 3.3 Types

```rust
// foundation_auth/src/secret/mod.rs
use zeroize::Zeroizing;

/// Separates the signatures of one use of a key from all other uses.
/// The signed message starts with `<domain>.v<version>` and a zero byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecretDomain(pub &'static str);

impl SecretDomain {
    /// agent-rpc bootstrap secrets. Version 1 signs exactly the bytes of today.
    pub const AGENT_RPC: SecretDomain = SecretDomain("ewe.agent_rpc.bootstrap_secret");
    /// Enrollment tokens of registrar sidecars at the proxy registration
    /// authority (feature 03, section 5). A different domain, so an agent-rpc
    /// secret never enrolls a sidecar, and the other way round.
    pub const PROXY_REGISTRATION: SecretDomain = SecretDomain("ewe.proxy.registrar_secret");
}

/// What a secret may be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretPurpose {
    /// Exchange for a client certificate (`BootstrapService/Enroll`).
    Enroll,
    /// Show as a bearer credential on each call (Workers, browsers).
    Access,
}

/// The signing algorithm. The enum is the same on every target; an
/// algorithm that a build cannot run gives `UnsupportedAlgorithm`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningAlgorithm {
    /// Ed25519 (RFC 8032). The default; all targets.
    Ed25519,
    /// RSA-PSS with SHA-256, 2048 to 8192 bits. Operator keys only;
    /// native with feature `secret-rsa`.
    RsaPssSha256,
}

/// A decoded signed secret. `Debug` and `Display` never show `secret`.
#[derive(Clone)]
pub struct SignedSecret {
    version: u32,
    alg: SigningAlgorithm,
    kid: Option<String>,
    public_key: Vec<u8>,
    secret: Zeroizing<Vec<u8>>,
    valid_from: u64,
    valid_until: u64,
    purpose: SecretPurpose,
    audience: Option<String>,
    max_uses: Option<u32>,
    signature: Vec<u8>,
}

impl SignedSecret {
    /// The shared form (base64url JSON). Treat it as a secret.
    #[must_use]
    pub fn encode(&self) -> Zeroizing<String> { /* codec::encode */ todo!("T5") }

    /// Parse the shared form. This checks the format only.
    ///
    /// # Errors
    /// [`SecretError::Malformed`], [`SecretError::UnsupportedVersion`],
    /// [`SecretError::UnsupportedAlgorithm`].
    pub fn decode(encoded: &str, limits: &DecodeLimits) -> Result<Self, SecretError> { todo!("T5") }

    /// The ID: lowercase hex SHA-256 of the 32 secret bytes. Safe to log
    /// and to store. Nobody can get the secret back from it.
    #[must_use]
    pub fn id(&self) -> String { crate::fingerprint::sha256_hex(&self.secret) }

    /// A client-side check: the signature matches the key that the secret
    /// carries. It finds a typing error. It does not prove who made it.
    #[must_use]
    pub fn verify_self_signed(&self) -> bool { todo!("T5") }

    #[must_use] pub fn version(&self) -> u32 { self.version }
    #[must_use] pub fn purpose(&self) -> SecretPurpose { self.purpose }
    #[must_use] pub fn audience(&self) -> Option<&str> { self.audience.as_deref() }
    #[must_use] pub fn valid_from(&self) -> u64 { self.valid_from }
    #[must_use] pub fn valid_until(&self) -> u64 { self.valid_until }
}

/// Size limits for decoding (rule 8: limits are configuration).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Largest accepted encoded secret, in bytes. Default 8192: nginx's
    /// default `large_client_header_buffers` size (8k), because the secret
    /// travels in an HTTP header.
    pub max_encoded_bytes: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self { max_encoded_bytes: 8192 }
    }
}
```

The `todo!("T5")` marks only show where task T5 puts the body. They are not part of the
design. The real code must have no `todo!` (CLAUDE.md rule 3).

### 3.4 Keys

```rust
// foundation_auth/src/secret/keys.rs
use zeroize::Zeroizing;
use foundation_core::env::Env;

/// The private key that signs secrets. Never printed.
pub struct SigningKey {
    alg: SigningAlgorithm,
    inner: SigningKeyInner,
    verifying: VerifyingKey,
}

enum SigningKeyInner {
    Ed25519(ed25519_compact::KeyPair),
    #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
    RsaPss { pair: ring::signature::RsaKeyPair, pkcs8: Zeroizing<Vec<u8>> },
}

impl SigningKey {
    /// A new Ed25519 key from 32 bytes of `env` entropy.
    ///
    /// # Errors
    /// [`SecretError::Entropy`] when the entropy source fails.
    pub fn generate_ed25519(env: &Env, kid: Option<String>) -> Result<Self, SecretError> {
        let mut seed = Zeroizing::new([0u8; 32]);
        env.fill_random(seed.as_mut()).map_err(|e| SecretError::Entropy(e.to_string()))?;
        let pair = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new(*seed));
        Ok(Self::from_ed25519(pair, kid))
    }

    /// Parse a PKCS#8 private key in DER form. Accepts:
    /// - Ed25519 PKCS#8 version 1 (RFC 8410, 48 bytes), as `ed25519-compact` writes;
    /// - Ed25519 PKCS#8 version 2 (83 bytes), as `ring` writes (every key file of today);
    /// - RSA (native, feature `secret-rsa`).
    ///
    /// # Errors
    /// [`SecretError::BadKey`] for any other input.
    pub fn from_pkcs8_der(der: &[u8], kid: Option<String>) -> Result<Self, SecretError> { todo!("T5") }

    /// Parse a `-----BEGIN PRIVATE KEY-----` PEM.
    ///
    /// # Errors
    /// [`SecretError::BadKey`].
    pub fn from_pkcs8_pem(pem: &str, kid: Option<String>) -> Result<Self, SecretError> { todo!("T5") }

    /// The key as PKCS#8 PEM (Ed25519: version 1 layout). Treat as secret.
    #[must_use]
    pub fn to_pkcs8_pem(&self) -> Zeroizing<String> { todo!("T5") }

    /// The public half, for verifiers.
    #[must_use]
    pub fn verifying_key(&self) -> &VerifyingKey { &self.verifying }

    pub(crate) fn sign(&self, message: &[u8]) -> Result<Vec<u8>, SecretError> {
        match &self.inner {
            // RFC 8032 signatures are deterministic: no randomness, no noise.
            SigningKeyInner::Ed25519(pair) => Ok(pair.sk.sign(message, None).to_vec()),
            #[cfg(all(feature = "secret-rsa", not(target_family = "wasm")))]
            SigningKeyInner::RsaPss { pair, .. } => sign_rsa_pss(pair, message),
        }
    }
}

/// A public key that verifies secrets. A Worker gets it from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyingKey {
    alg: SigningAlgorithm,
    /// Raw Ed25519 key (32 bytes), or DER `RSAPublicKey`.
    bytes: Vec<u8>,
    kid: Option<String>,
}

impl VerifyingKey {
    /// From the base64url form that a `SignedSecret` and the configuration use.
    ///
    /// # Errors
    /// [`SecretError::BadKey`].
    pub fn from_base64url(alg: SigningAlgorithm, b64: &str, kid: Option<String>) -> Result<Self, SecretError> { todo!("T5") }

    /// Lowercase hex SHA-256 of the key bytes. Safe to log.
    #[must_use]
    pub fn fingerprint(&self) -> String { crate::fingerprint::sha256_hex(&self.bytes) }

    pub(crate) fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, SecretError> {
        match self.alg {
            SigningAlgorithm::Ed25519 => {
                let pk = ed25519_compact::PublicKey::from_slice(&self.bytes)
                    .map_err(|_| SecretError::BadKey("Ed25519 public key".into()))?;
                let sig = ed25519_compact::Signature::from_slice(signature)
                    .map_err(|_| SecretError::BadSignature)?;
                Ok(pk.verify(message, &sig).is_ok())
            }
            SigningAlgorithm::RsaPssSha256 => verify_rsa_pss(&self.bytes, message, signature),
        }
    }
}

#[cfg(not(all(feature = "secret-rsa", not(target_family = "wasm"))))]
fn verify_rsa_pss(_: &[u8], _: &[u8], _: &[u8]) -> Result<bool, SecretError> {
    Err(SecretError::UnsupportedAlgorithm("rsa-pss-sha256 needs native + feature secret-rsa".into()))
}
```

The two Ed25519 PKCS#8 layouts that `from_pkcs8_der` accepts. A small strict parser checks
the exact prefix bytes. It refuses everything else.

```text
version 1 (48 bytes):
  30 2e 02 01 00 30 05 06 03 2b 65 70 04 22 04 20 | 32-byte seed

version 2 (83 bytes, the layout of ring's ed25519_pkcs8_v2_template.der):
  30 51 02 01 01 30 05 06 03 2b 65 70 04 22 04 20 | 32-byte seed | 81 21 00 | 32-byte public key
```

For version 2, the parser also checks that the stored public key equals the key that the
seed makes. A file that fails this check is an error. The code never overwrites it.

### 3.5 The issuer and the verifier

The owner asked to split signing from verifying. The **issuer** holds the private key. The
**verifier** holds only public keys. A Worker gets public keys from its configuration and
never sees a private key.

```rust
// foundation_auth/src/secret/issuer.rs
use foundation_core::env::Env;
use foundation_compact::Duration;

/// Makes signed secrets. Needs the private key.
pub struct SecretIssuer {
    domain: SecretDomain,
    key: SigningKey,
    env: Env,
    max_validity: Duration,
}

/// What to issue.
#[derive(Debug, Clone)]
pub struct SecretRequest {
    pub purpose: SecretPurpose,
    pub validity: Duration,
    /// `None` = valid from now.
    pub not_before_unix: Option<u64>,
    pub audience: Option<String>,
    pub max_uses: Option<u32>,
}

impl SecretRequest {
    /// An enrollment secret, as today (version 1 on the wire).
    #[must_use]
    pub fn enroll(validity: Duration) -> Self {
        Self { purpose: SecretPurpose::Enroll, validity, not_before_unix: None, audience: None, max_uses: None }
    }

    /// An access secret for one host (version 2 on the wire).
    #[must_use]
    pub fn access(validity: Duration, audience: impl Into<String>) -> Self {
        Self { purpose: SecretPurpose::Access, validity, not_before_unix: None, audience: Some(audience.into()), max_uses: None }
    }
}

impl SecretIssuer {
    #[must_use]
    pub fn new(domain: SecretDomain, key: SigningKey, env: Env, max_validity: Duration) -> Self {
        Self { domain, key, env, max_validity }
    }

    /// Make a secret.
    ///
    /// # Errors
    /// [`SecretError::BadRequest`] for an empty window or a window longer
    /// than `max_validity`; [`SecretError::Entropy`] when the entropy
    /// source fails.
    pub fn issue(&self, request: &SecretRequest) -> Result<SignedSecret, SecretError> {
        // 1. Times from the injected clock.
        let from = request.not_before_unix.unwrap_or_else(|| self.env.unix_secs());
        let until = from.saturating_add(request.validity.as_secs());
        // 2. Refuse an empty or too long window.
        // 3. 32 secret bytes from the injected entropy (Zeroizing buffer).
        // 4. Version 1 when purpose = Enroll and no audience, kid or max_uses; else 2.
        // 5. Sign codec::signed_message(domain, fields) with self.key.
        todo!("T5")
    }
}
```

```rust
// foundation_auth/src/secret/verifier.rs
use foundation_core::env::Env;
use foundation_compact::Duration;

/// Checks signed secrets. Needs only public keys.
pub struct SecretVerifier {
    domain: SecretDomain,
    /// More than one key during a rotation.
    keys: Vec<VerifyingKey>,
    env: Env,
    policy: VerifyPolicy,
}

/// What the verifier accepts. Every limit is configuration (rule 8).
#[derive(Debug, Clone)]
pub struct VerifyPolicy {
    /// The purpose that this verifier accepts.
    pub purpose: SecretPurpose,
    /// Required audience. `None` = accept a secret with no audience.
    pub audience: Option<String>,
    /// Allowed clock error at both ends of the window. Default 5 minutes:
    /// the default `clockskew` of MIT Kerberos (300 s).
    pub clock_skew: Duration,
    /// Longest window accepted. Default 24 hours for `Enroll` (the kubeadm
    /// bootstrap token TTL, as today) and 30 days for `Access` (the
    /// longest a bearer secret may live without renewal).
    pub max_validity: Duration,
    pub decode: DecodeLimits,
}

/// A secret that passed every check of `verify`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSecret {
    pub id: String,
    pub purpose: SecretPurpose,
    pub valid_from: u64,
    pub valid_until: u64,
    pub max_uses: Option<u32>,
    pub audience: Option<String>,
    /// Fingerprint of the public key that matched.
    pub key_fingerprint: String,
}

impl SecretVerifier {
    #[must_use]
    pub fn new(domain: SecretDomain, keys: Vec<VerifyingKey>, env: Env, policy: VerifyPolicy) -> Self {
        Self { domain, keys, env, policy }
    }

    /// Check `presented` now. The steps run in this order; the cheap
    /// checks come first:
    ///
    /// 1. size: at most `policy.decode.max_encoded_bytes`;
    /// 2. format: base64url, JSON, known version and algorithm, 32 secret
    ///    bytes, a non-empty window;
    /// 3. key: by `kid` (version 2), else by the exact public key bytes;
    /// 4. signature over the domain-separated message;
    /// 5. purpose equal to `policy.purpose` (version 1 = `Enroll`);
    /// 6. audience equal to `policy.audience` when the policy sets one;
    /// 7. window length at most `policy.max_validity`;
    /// 8. time: `valid_from - skew <= now < valid_until + skew`, with
    ///    `now = env.unix_secs()`.
    ///
    /// Use counts and revocation are the next step, in the trust store
    /// (`TrustedClients::spend_secret`, `check_secret_not_revoked`).
    ///
    /// # Errors
    /// One `SecretError` variant per failed step.
    pub fn verify(&self, presented: &str) -> Result<VerifiedSecret, SecretError> { todo!("T5") }
}

/// Every way a secret can fail.
#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum SecretError {
    #[display("malformed secret: {_0}")]
    Malformed(#[error(not(source))] String),
    #[display("secret format version {_0} is not supported")]
    UnsupportedVersion(#[error(not(source))] u32),
    #[display("unsupported algorithm: {_0}")]
    UnsupportedAlgorithm(#[error(not(source))] String),
    #[display("the secret was signed by an unknown key")]
    UnknownKey,
    #[display("the secret signature is not valid")]
    BadSignature,
    #[display("the secret is for another purpose")]
    WrongPurpose,
    #[display("the secret is for another host")]
    WrongAudience,
    #[display("the secret window is longer than allowed")]
    WindowTooLong,
    #[display("the secret is not valid yet")]
    NotYetValid,
    #[display("the secret has expired")]
    Expired,
    #[display("bad key: {_0}")]
    BadKey(#[error(not(source))] String),
    #[display("bad request: {_0}")]
    BadRequest(#[error(not(source))] String),
    #[display("entropy source failed: {_0}")]
    Entropy(#[error(not(source))] String),
}
```

What the code does: `SecretIssuer` makes secrets with the private key and the `Env`.
`SecretVerifier` checks them with public keys and the `Env` clock. The `VerifiedSecret`
carries the ID that the trust store uses for use counts and revocation.

### 3.6 Key rotation

1. Make a new key with a new `kid`.
2. Give the verifiers both public keys.
3. Issue new secrets with the new key only.
4. When the last old secret has expired (its `valid_until` plus `clock_skew`), remove the old public key from the verifiers.

A lost private key on a native host is made again at start (as today). That makes every
open secret invalid, which is the intended behavior.

### 3.7 Use counts

A use count says how often a secret was used. The trust store keeps it (section 6).

| Purpose | Default limit | Where it is checked |
|---|---|---|
| `Enroll` | 1 use (`EnrollConfig::max_enrollments_per_secret`) | `EnrollmentService` calls `TrustedClients::spend_secret` after the CSR is valid. |
| `Access` | no limit (`AccessPolicy::max_uses = None`); the token `max_uses` applies if set | `SignedSecretAuth` calls `spend_secret` only when a limit applies. So an unlimited secret costs no write per call. |

### 3.8 Tests for signed secrets

All in `backends/foundation_auth/tests/secret/` (registered in `tests/secret/mod.rs`):

- `golden_v1_tests.rs`: a secret made by the branch code of today (stored in `tests/fixtures/secret_v1.txt` with its key in `tests/fixtures/test-only-ed25519-key.pem`) still verifies. The signed message bytes equal a stored hex dump.
- `v2_tests.rs`: version 2 round-trips. Each changed signed field breaks the signature.
- `verify_order_tests.rs`: one test per step of `SecretVerifier::verify`, with `SimClock` for the time steps (inside skew, outside skew, at the exact edges).
- `pkcs8_tests.rs`: version 1 and version 2 Ed25519 files load; a version 2 file with a wrong public key fails; the existing RSA fixture loads (native).
- `wasm` gate: `cargo check --target wasm32-unknown-unknown -p foundation_auth --no-default-features --features wasm,secret`.

## 4. Key material store (`foundation_auth::keystore`)

The owner asked for a `CaStore` provider trait. We call it `KeyStore`, because it holds
every kind of key material: the CA, the secret-signing key and the server identity. A
`KeyStore` loads, or makes and then keeps, key material in any backend: the file system,
memory, JSON files, Turso, libsql, D1, Workers KV or R2.

### 4.1 The trait and the record

```rust
// foundation_auth/src/keystore/mod.rs
use core::future::Future;
use core::pin::Pin;

use serde::{Deserialize, Serialize};

/// The future that every `KeyStore` call returns.
pub type KeyFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, KeyStoreError>> + Send + 'a>>;

/// The name of one record: 1 to 64 of `[a-z0-9._-]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyId(String);

impl KeyId {
    pub const CA: &'static str = "ca";
    pub const SECRET_SIGNING: &'static str = "secret-signing";
    pub const SERVER_IDENTITY: &'static str = "server-identity";

    /// # Errors
    /// [`KeyStoreError::BadId`].
    pub fn new(id: &str) -> Result<Self, KeyStoreError> { todo!("T7") }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyKind {
    CertificateAuthority,
    SecretSigning,
    ServerIdentity,
}

/// One stored item of key material.
#[derive(Clone, Serialize, Deserialize)]
pub struct KeyRecord {
    pub id: KeyId,
    pub kind: KeyKind,
    /// Starts at 1. Each replacement adds 1.
    pub revision: u64,
    pub created_at_unix: u64,
    /// Certificates only.
    pub not_after_unix: Option<u64>,
    pub public: PublicMaterial,
    pub private: StoredPrivateKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicMaterial {
    /// "ed25519", "ecdsa-p256" or "rsa-pss-sha256".
    pub algorithm: String,
    pub public_key_b64url: String,
    /// CA and server identity only.
    pub cert_pem: Option<String>,
    /// Server identity only: the DNS names and IP literals it covers.
    pub subject_names: Vec<String>,
}

/// How the private key is kept.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "protection", rename_all = "kebab-case")]
pub enum StoredPrivateKey {
    /// Encrypted with a KEK (section 4.2). The normal case.
    Sealed(SealedBlob),
    /// PKCS#8 PEM in clear text. **Read only**, for the legacy files of
    /// today (section 4.6). No store writes it (owner decision 3: always
    /// encrypt at rest). The host seals it at the first start with a KEK.
    Plaintext { pkcs8_pem: SecretText },
    /// The key lives outside the store (a Worker secret, an environment
    /// variable). The record keeps only the name of that place.
    External { reference: String },
    /// No private key: a verify-only record.
    Absent,
}

/// A `String` that is zeroed on drop and never printed.
#[derive(Clone)]
pub struct SecretText(zeroize::Zeroizing<String>);
// `Serialize`/`Deserialize` are written by hand: they read and write the
// plain string; `Debug` prints "<redacted>".

pub enum CreateOutcome {
    Created,
    /// Another writer made the record first. Use theirs.
    AlreadyExists(KeyRecord),
}

/// Facts about a store, so callers can refuse unsafe setups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyStoreInfo {
    pub name: &'static str,
    /// `create` is atomic across processes ("first writer wins").
    pub atomic_create: bool,
    /// Other processes can write the same records.
    pub shared: bool,
    pub writable: bool,
}

/// Loads and keeps key material in any backend.
pub trait KeyStore: Send + Sync + 'static {
    /// The record, or `None`.
    fn get<'a>(&'a self, id: &'a KeyId) -> KeyFuture<'a, Option<KeyRecord>>;

    /// Store `record` only if no record with its ID exists. The check and
    /// the write are one atomic step when `info().atomic_create` is true.
    fn create<'a>(&'a self, record: &'a KeyRecord) -> KeyFuture<'a, CreateOutcome>;

    /// Replace the record when the stored revision is `expected_revision`.
    /// Rotation is an operator action; it does not race with `create`.
    fn replace<'a>(&'a self, record: &'a KeyRecord, expected_revision: u64) -> KeyFuture<'a, ()>;

    fn info(&self) -> KeyStoreInfo;
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum KeyStoreError {
    #[display("bad key id {_0:?}")]
    BadId(#[error(not(source))] String),
    #[display("no key material named {_0}")]
    NotFound(#[error(not(source))] String),
    #[display("the stored record {_0} is corrupt; it is not overwritten")]
    Corrupt(#[error(not(source))] String),
    #[display("the store is read-only")]
    ReadOnly,
    #[display("this backend cannot create records atomically")]
    NotAtomic,
    #[display("revision conflict for {_0}")]
    Conflict(#[error(not(source))] String),
    #[display("plain-text private keys are not allowed in {_0}")]
    PlaintextRefused(#[error(not(source))] &'static str),
    #[display("sealing failed: {_0}")]
    Seal(#[error(not(source))] String),
    #[display("storage error: {_0}")]
    Storage(#[error(not(source))] String),
}
```

What the code does: `KeyStore` has three operations. `create` is "insert only if absent",
so two processes that start at the same time cannot both write a CA. `KeyRecord` keeps the
public part in clear text and the private part sealed. `KeyStoreInfo` tells the caller if
the backend is safe for shared use.

### 4.2 Encryption at rest

A private key in a database or a file must be sealed. **Sealing** means AEAD encryption
with a KEK. The AEAD also signs some extra bytes (the associated data), so a sealed key
cannot move to another record.

```rust
// foundation_auth/src/keystore/sealing.rs
use foundation_compact::crypto::{EncryptionKey, SecureBytes};
use foundation_core::env::Env;

/// Encrypts and decrypts private keys.
pub trait KeySealer: Send + Sync + 'static {
    /// The name of the KEK, stored with each blob (for rotation).
    fn kek_id(&self) -> &str;
    fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<SealedBlob, KeyStoreError>;
    fn open(&self, aad: &[u8], sealed: &SealedBlob) -> Result<SecureBytes, KeyStoreError>;
}

/// The stored form of a sealed key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedBlob {
    /// Format version: 1.
    pub v: u32,
    /// "chacha20poly1305" or "argon2id+chacha20poly1305".
    pub alg: String,
    pub kek_id: String,
    pub nonce_b64url: String,
    pub ciphertext_b64url: String,
    /// `argon2id+…` only.
    pub salt_b64url: Option<String>,
    /// `argon2id+…` only: memory KiB, iterations, lanes.
    pub argon2: Option<Argon2Params>,
}

/// A 32-byte KEK (from an environment variable, a file or a Worker secret).
pub struct KekSealer {
    kek_id: String,
    key: EncryptionKey<32>,
    env: Env,
}

/// A KEK derived from a passphrase with Argon2id, one salt per blob.
pub struct PassphraseSealer {
    kek_id: String,
    passphrase: zeroize::Zeroizing<String>,
    params: Argon2Params,
    env: Env,
}

/// Argon2id cost. Default: 19 MiB memory, 2 passes, 1 lane — the OWASP
/// Password Storage Cheat Sheet minimum for Argon2id (RFC 9106 profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Argon2Params {
    pub memory_kib: u32,
    pub iterations: u32,
    pub lanes: u32,
}

/// The associated data for one record. It binds the blob to the record
/// ID, kind and revision.
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
```

What the code does: `KekSealer` uses ChaCha20-Poly1305 with a random 12-byte nonce from
`env`. `PassphraseSealer` first turns a passphrase into a key with Argon2id and a random
salt. `record_aad` makes the associated data. A blob for record `ca` revision 3 does not
open as record `secret-signing`.

`foundation_compact::crypto::chacha::{encrypt, decrypt}` take no associated data and read
the nonce from global entropy. Task T1 adds `seal_with(key, nonce, aad, plaintext)` and
`open_with(key, aad, blob)` next to them. The existing functions stay.

Where the KEK comes from: [key-protection.md](./key-protection.md). Owner decision 3: a private
key is **always** encrypted at rest, on every host and every backend. There is no plain-text mode.
key-protection.md also makes `KeySealer` async (for WebCrypto and remote key management services)
and replaces the sync trait above.

### 4.3 Implementations

| Type | Backends | `atomic_create` | `shared` | Notes |
|---|---|---|---|---|
| `FsKeyStore` | a local directory (native) | yes: write a temporary file, then `std::fs::hard_link` it to the final name; `hard_link` fails if the name exists | no | Reads the legacy files of today (section 4.6). File I/O runs inside the async functions, like `foundation_db::JsonFileStorage`. |
| `DbKeyStore<K: KeyValueStore>` | `MemoryStorage`, `MemoryJsonStore`, `JsonFileStorage`, `TursoStorage`, `LibsqlStore`, `D1Store` (native REST), `D1WasmStorage`, `StorageProvider` | yes, through the new `insert_if_absent_async` (task T6) | yes | Workers KV is eventually consistent, so its `insert_if_absent_async` returns `Unsupported` and the store is read-only there. |
| `DbKeyStore<BlobKv<B: BlobStore>>` | `R2WasmStorage`, `R2Store` | no | yes | `BlobKv` adapts a blob store to the key-value trait. Read-only use: the operator uploads the records. |
| `EnvKeyStore` | values from configuration (environment variables, Worker vars and secrets) | — | — | Read-only. A Worker uses it for the public verifying keys. |
| `MemoryKeyStore` | memory (feature `testing`) | yes | no | For unit tests and DST. |

`DbKeyStore` stores each record as JSON under the key `ewe/keystore/v1/<id>` (the prefix is
configuration).

`foundation_db` change (task T6): a new method on `KeyValueStore` with a default body,
so existing implementations outside the workspace still build:

```rust
// foundation_db/src/core/storage_provider.rs (changed)
#[async_trait::async_trait]
pub trait KeyValueStore: Send + Sync {
    // … the five existing methods …

    /// Store `value` under `key` only if `key` is absent. Returns `true`
    /// when this call stored it. Atomic across processes on SQL backends
    /// (`INSERT … ON CONFLICT DO NOTHING`) and inside one process on the
    /// memory and JSON-file backends.
    ///
    /// # Errors
    /// [`StorageError::Unsupported`] when the backend cannot do it
    /// atomically (the default, and Workers KV).
    async fn insert_if_absent_async<V: Serialize + Send + 'static>(
        &self,
        key: &str,
        value: V,
    ) -> StorageResult<bool> {
        let _ = (key, value);
        Err(StorageError::Unsupported("insert_if_absent_async".into()))
    }
}
```

If `StorageError` has no `Unsupported` variant on master, task T6 adds it.

### 4.4 Load or make key material

```rust
// foundation_auth/src/keystore/material.rs
use std::sync::Arc;

use foundation_core::env::Env;

/// How private keys are kept in a store. Owner decision 3: always sealed.
/// There is no plain-text variant ([key-protection.md](./key-protection.md)).
#[derive(Clone)]
pub enum KeyProtection {
    /// The current sealer, and older sealers that can still open blobs
    /// sealed before a KEK rotation.
    Sealed { current: Arc<dyn KeySealer>, previous: Vec<Arc<dyn KeySealer>> },
}

/// Loads or makes the key material of a host.
pub struct KeyMaterial {
    store: Arc<dyn KeyStore>,
    /// `None` for a verify-only host: it never makes or opens a private key.
    protection: Option<KeyProtection>,
    env: Env,
}

impl KeyMaterial {
    #[must_use]
    pub fn new(store: Arc<dyn KeyStore>, protection: KeyProtection, env: Env) -> Self {
        Self { store, protection: Some(protection), env }
    }

    /// For a host that only verifies (a Worker). Every call that needs a
    /// private key returns `KeyStoreError::ReadOnly`.
    #[must_use]
    pub fn verify_only(store: Arc<dyn KeyStore>, env: Env) -> Self {
        Self { store, protection: None, env }
    }

    /// The secret-signing key: loaded, or made (Ed25519) and stored.
    ///
    /// # Errors
    /// [`KeyStoreError`] variants; a corrupt record is an error and is not
    /// replaced.
    pub async fn signing_key(&self, id: &KeyId) -> Result<SigningKey, KeyStoreError> {
        let record = self
            .load_or_create(id, |env| {
                let key = SigningKey::generate_ed25519(env, Some(id.as_str().into()))
                    .map_err(|e| KeyStoreError::Seal(e.to_string()))?;
                self.record_for_signing_key(id, &key)
            })
            .await?;
        self.open_signing_key(&record)
    }

    /// The public keys for a verify-only host (a Worker).
    ///
    /// # Errors
    /// [`KeyStoreError::NotFound`] when a key is missing.
    pub async fn verifying_keys(&self, ids: &[KeyId]) -> Result<Vec<VerifyingKey>, KeyStoreError> { todo!("T7") }

    /// The CA (native): loaded, or made and stored.
    #[cfg(feature = "pki")]
    pub async fn certificate_authority(&self, id: &KeyId, config: &CaConfig) -> Result<CertificateAuthority, KeyStoreError> { todo!("T8") }

    /// The server certificate (native): reused while it covers `names` and
    /// is not close to expiry; else made again and stored (section 5.3).
    #[cfg(feature = "pki")]
    pub async fn server_identity(&self, ca: &CertificateAuthority, config: &ServerIdentityConfig) -> Result<ServerIdentity, KeyStoreError> { todo!("T8") }

    /// The common algorithm: read; if absent, make and try to store;
    /// if another writer won, use theirs.
    async fn load_or_create<F>(&self, id: &KeyId, make: F) -> Result<KeyRecord, KeyStoreError>
    where
        F: FnOnce(&Env) -> Result<KeyRecord, KeyStoreError>,
    {
        if let Some(found) = self.store.get(id).await? {
            return Ok(found);
        }
        let info = self.store.info();
        if !info.writable {
            return Err(KeyStoreError::NotFound(id.as_str().into()));
        }
        if info.shared && !info.atomic_create {
            return Err(KeyStoreError::NotAtomic);
        }
        let fresh = make(&self.env)?;
        match self.store.create(&fresh).await? {
            CreateOutcome::Created => Ok(fresh),
            CreateOutcome::AlreadyExists(winner) => {
                tracing::info!(key = id.as_str(), "another process created this key first; using it");
                Ok(winner) // `fresh` drops here; its key bytes are zeroed
            }
        }
    }
}
```

What the code does: `load_or_create` reads the record. If it is absent, it makes a new key,
seals it and tries to store it with `create`. If another process stored one first, it uses
that one. A shared store that cannot create atomically is refused, because two processes
could then make two different CAs.

This sequence diagram shows two processes that start at the same time on one shared store.
Only one key wins, and both processes use it.

```mermaid
sequenceDiagram
    participant A as Process A
    participant B as Process B
    participant S as KeyStore (shared, atomic create)
    A->>S: get("ca")
    S-->>A: none
    B->>S: get("ca")
    S-->>B: none
    A->>A: make key A and seal it
    B->>B: make key B and seal it
    A->>S: create(record A)
    S-->>A: Created
    B->>S: create(record B)
    S-->>B: AlreadyExists(record A)
    B->>B: drop key B, open record A
    Note over A,B: both processes now use key A
```

### 4.5 Which store on which host

| Host | Signing key | CA and server identity | Trusted clients |
|---|---|---|---|
| Native, one process (default) | `FsKeyStore` in the state directory | `FsKeyStore` | `SqlTrustStore<TursoStorage>` in the state directory |
| Native, several replicas | `DbKeyStore<LibsqlStore>` or `DbKeyStore<D1Store>`, sealed | same | `SqlTrustStore` on the same database |
| Worker, verify only | `EnvKeyStore` (public key from the var `AGENT_RPC_SECRET_PUBLIC_KEYS`) | none (Cloudflare terminates TLS) | `SqlTrustStore<D1WasmStorage>` |
| DST | `MemoryKeyStore` | `MemoryKeyStore` (native DST only) | `MemoryTrustStore`. The `turso` feature of `foundation_db` turns on valtron `multi`, so Turso cannot be in the single-threaded DST build. The `SqlTrustStore` contract tests run in the normal test build. |

### 4.6 The legacy files of today

A state directory of today holds `ca-cert.pem`, `ca-key.pem`,
`bootstrap-signing-key.pem` and `trusted-clients.db`. They must keep working.

1. `FsKeyStore::get("ca")` reads `keys/ca.json`. If that file is absent and both legacy CA files exist, it returns a record built from them (`revision = 1`, `Plaintext`). If only one of them exists, it returns `Corrupt` (as today).
2. `FsKeyStore::get("secret-signing")` reads `keys/secret-signing.json`, else the legacy `bootstrap-signing-key.pem`.
3. The environment variable `EWE_AGENT_RPC_SIGNING_KEY` (inline PEM or a path) still overrides the signing key. The native host reads it into an `EnvKeyStore` that it puts in front of the `FsKeyStore`.
4. A KEK is required (owner decision 3). At the first start with legacy files, the host seals each key into `keys/*.json`, checks that the sealed record opens, and then removes the plain-text file. `agent-rpc keys migrate` (a sub-command of the native binary, task T17) does the same steps by hand. With no KEK, the host does not start ([key-protection.md](./key-protection.md) section 6).
5. `trusted-clients.db` keeps its tables. `SqlTrustStore` adds one new table (section 6.2).

## 5. The CA and certificates (`foundation_auth::pki`, native)

### 5.1 The CA

```rust
// foundation_auth/src/pki/mod.rs
use foundation_core::env::Env;
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
    env: Env,
    config: CaConfig,
}

impl CertificateAuthority {
    /// A new self-signed CA. The key comes from `env` entropy and the dates
    /// from `env` time. Returns the CA and the record to store.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn generate(config: &CaConfig, env: &Env) -> Result<(Self, KeyRecord), PkiError> { todo!("T8") }

    /// Rebuild from a stored record and its opened private key.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn from_record(record: &KeyRecord, private_pkcs8_der: &[u8], env: &Env, config: &CaConfig) -> Result<Self, PkiError> { todo!("T8") }

    /// A server certificate for `names`.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn issue_server_identity(&self, names: &[String], lifetime: Duration) -> Result<(ServerIdentity, KeyRecord), PkiError> { todo!("T8") }

    /// Sign a client CSR. Only the public key of the CSR is used. The
    /// subject is `client_name`, the certificate is a non-CA leaf with the
    /// `clientAuth` extended key usage, and the lifetime is ours (as today).
    ///
    /// # Errors
    /// [`PkiError::BadCsr`] for an invalid CSR or name.
    pub fn sign_client_csr(&self, csr_pem: &str, client_name: &str, lifetime: Duration) -> Result<IssuedClientCert, PkiError> { todo!("T8") }

    #[must_use]
    pub fn ca_cert_pem(&self) -> &str { &self.cert_pem }
}
```

### 5.2 Deterministic signing

`rcgen` signs ECDSA with a random nonce from `ring`. That breaks DST, and `ring` does not
let us pass our own random source. `rcgen` has a hook for this: `RemoteKeyPair`. We sign
with our own key there.

```rust
// foundation_auth/src/pki/signer.rs
/// Signs certificates with a key that came from `Env` entropy.
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
P-256 signatures use RFC 6979, so the same key and message give the same signature. With a
`SimClock` and `SeededEntropy`, the same seed gives byte-identical certificates and
fingerprints. The certificate serial number is already a hash of the public key, so it is
also stable.

### 5.3 The server identity keeps its fingerprint

Today `serve` makes a new server certificate on each start. So the pinned fingerprint in
every payload stops working after a restart. We fix this:

1. `KeyMaterial::server_identity` reads the record `server-identity`.
2. It reuses the record when it covers the configured names and `now < not_after - renew_before`.
3. `renew_before` is one third of the lifetime (30 days of 90). This is when certbot renews a 90-day certificate.
4. Otherwise it makes a new certificate and replaces the record. The operator must hand out new payloads then. The host logs the new fingerprint.
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

## 6. Trusted clients (`foundation_auth::trust`)

### 6.1 The trait

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

### 6.2 `SqlTrustStore<Q: QueryStore>`

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
backend (owner decision 7 in [feature.md](./feature.md): a separate feature).

### 6.3 `KvTrustStore<K: KeyValueStore>`

For backends without SQL: `MemoryStorage`, `MemoryJsonStore`, `JsonFileStorage`.

- Keys: `trust/v1/client/<fingerprint>`, `trust/v1/use/<secret_id>`, `trust/v1/revoked/<secret_id>`.
- `consume_secret_use` takes an async lock inside the process, then reads and writes. So it is atomic only inside one process: `atomic_across_processes = false`.
- `TrustedClients::open` refuses a store with `atomic_across_processes = false` when `TrustConfig::shared_store` is true.
- Never use it over Workers KV: Workers KV is eventually consistent, so a secret could be spent twice. The Worker host uses D1 (`SqlTrustStore<D1WasmStorage>`).

### 6.4 `TrustedClients`: the store plus an index

```rust
// foundation_auth/src/trust/clients.rs
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use foundation_core::env::Env;
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
    env: Env,
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
    pub async fn open(store: Arc<dyn TrustStore>, env: Env, config: TrustConfig) -> Result<Arc<Self>, TrustError> { todo!("T9") }

    /// The index handle for `TrustedClientVerifier`.
    #[must_use]
    pub fn index(&self) -> TrustIndex { self.index.clone() }

    /// The client, from the index or (`StoreOnly`) from the store.
    pub async fn get(&self, fingerprint: &str) -> Result<Option<TrustedClient>, TrustError> { todo!("T9") }

    pub async fn record_enrollment(&self, client: TrustedClient, cert_pem: &str) -> Result<(), TrustError> { todo!("T9") }

    /// Revoke: the store first, then the index.
    pub async fn revoke_client(&self, fingerprint: &str) -> Result<bool, TrustError> { todo!("T9") }

    /// Record a seen client at most once per `seen_write_interval`.
    /// Store errors are returned to the caller, never only logged.
    pub async fn touch(&self, fingerprint: &str) -> Result<(), TrustError> {
        let now = self.env.unix_secs();
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
    pub async fn spend_secret(&self, secret: &VerifiedSecret, max_uses: u32) -> Result<(), TrustError> { todo!("T9") }

    /// # Errors
    /// [`TrustError::SecretRevoked`].
    pub async fn check_secret_not_revoked(&self, secret_id: &str) -> Result<(), TrustError> { todo!("T9") }

    pub async fn revoke_secret(&self, secret_id: &str, valid_until: u64) -> Result<bool, TrustError> { todo!("T9") }
}
```

What the code does: `TrustedClients` wraps a `TrustStore`. On a native host it keeps an
index in memory, so the TLS verifier needs no database call. `touch` writes the last-seen
time at most once per 75 seconds per client, and it returns store errors.

## 7. TLS verification (`foundation_auth::mtls`, native)

### 7.1 The verifier is a pure check

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

The last-seen write moves to `MtlsAuth` (section 8.3). It runs in the async call path,
after the handshake, with `.await`, and it returns its error.

### 7.2 The rustls time comes from `Env`

rustls asks a `TimeProvider` for the time when it checks certificate dates. We give it the
`Env` clock, so certificate expiry follows the injected time too:

```rust
// foundation_auth/src/mtls/time.rs
#[derive(Debug)]
pub struct EnvTimeProvider(pub Arc<dyn foundation_compact::clock::Clock>);

impl rustls::time_provider::TimeProvider for EnvTimeProvider {
    fn current_time(&self) -> Option<rustls::pki_types::UnixTime> {
        let secs = foundation_compact::clock::unix_secs(self.0.wall_now());
        Some(rustls::pki_types::UnixTime::since_unix_epoch(core::time::Duration::from_secs(secs)))
    }
}
```

The config builders use `rustls::ServerConfig::builder_with_details(provider, time_provider)`.

### 7.3 Config builders

```rust
// foundation_auth/src/mtls/config.rs
/// A server config that requires a client certificate from `ca` that is in `index`.
pub fn server_config_mutual(identity: &ServerIdentity, ca: &CertificateAuthority, index: TrustIndex, env: &Env)
    -> Result<Arc<rustls::ServerConfig>, PkiError> { todo!("T10") }

/// A server config with no client certificate (the enrollment listener).
pub fn server_config_server_auth(identity: &ServerIdentity, env: &Env)
    -> Result<Arc<rustls::ServerConfig>, PkiError> { todo!("T10") }

/// A client config that trusts exactly one server certificate (pinned SHA-256).
pub fn client_config_pinned(fingerprint_sha256: &str, env: &Env)
    -> Result<Arc<rustls::ClientConfig>, PkiError> { todo!("T10") }

/// A client config with a client certificate that trusts `ca_pem`.
pub fn client_config_mutual(ca_pem: &str, cert_pem: &str, key_pem: &str, env: &Env)
    -> Result<Arc<rustls::ClientConfig>, PkiError> { todo!("T10") }
```

### 7.4 The host must report the peer certificate

`MtlsAuth` needs the client certificate in the call context. `foundation_netio` already has
the field: `ConnectionContext::tls: Option<TlsInfo { peer_certificates, server_name }>`. But
no code fills it today. Task T4 fills it:

1. `RustlsStream<ServerConnection>` gets `fn tls_info(&self) -> Option<TlsInfo>`. It reads `peer_certificates()` and `server_name()`.
2. `foundation_http`'s `ProtocolDetectHandler` peeks the first bytes of a connection. A TLS stream returns bytes only after the handshake. So after the first successful peek, the handler builds a new `Arc<ConnectionContext>` with `tls: Some(…)` and gives it to the HTTP/1.1 or HTTP/2 handler.
3. The RPC dispatcher already copies `request.connection` into `RequestContext::connection`.

## 8. Client auth (`foundation_auth::client_auth`)

Owner decision 4: `JwtAuth` ships in this feature, behind the `jwt` feature flag. The default
provider stays the signed secret (`SignedSecretAuth`), not a JWT.

### 8.1 The trait

```rust
// foundation_auth/src/client_auth/mod.rs
use core::future::Future;
use core::pin::Pin;

use foundation_netio::shared::context::ConnectionContext;
use foundation_netio::shared::http::{Extensions, SimpleHeaders};

use crate::AuthContext;

/// What a provider may look at for one call.
#[derive(Clone, Copy)]
pub struct CallCredentials<'a> {
    /// The full procedure path, for example "/ewe.agent_rpc.v1.AgentService/Chat".
    pub procedure: &'a str,
    pub headers: &'a SimpleHeaders,
    /// Peer address and TLS facts (peer certificates on a native mTLS host).
    pub connection: &'a ConnectionContext,
    /// Facts that the host added, for example `CfTlsClientAuth` on a Worker.
    pub extensions: &'a Extensions,
}

/// The single auth result that handlers see (moved here from
/// `foundation_connectrpc::shared::auth`; connectrpc re-exports it).
#[derive(Clone, Debug)]
pub struct AuthInfo {
    pub context: AuthContext,
    pub artifacts: Extensions,
}

/// Who called. Every provider puts one into `AuthInfo::artifacts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub subject: String,
    pub method: AuthMethod,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    ClientCertificate { fingerprint_sha256: String },
    SignedSecret { secret_id: String },
    Jwt { issuer: String },
    CloudflareMtls { fingerprint_sha256: String },
    /// `InsecureNoAuth` (development only).
    Insecure,
    /// A Durable Object: the Worker already checked the caller.
    Forwarded,
}

pub type AuthFuture<'a> = Pin<Box<dyn Future<Output = Result<Option<AuthInfo>, AuthError>> + Send + 'a>>;

/// Checks the credentials of one call.
///
/// - `Ok(Some(info))`: the caller is known.
/// - `Ok(None)`: the call has no credentials of this kind.
/// - `Err(_)`: the call has credentials of this kind, and they are not good.
pub trait ClientAuth: Send + Sync + 'static {
    /// For logs and for `GetCapabilities.auth_modes`.
    fn mode(&self) -> &'static str;
    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a>;
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum AuthError {
    #[display("invalid credentials: {_0}")]
    Invalid(#[error(not(source))] String),
    #[display("credentials expired")]
    Expired,
    #[display("credentials not valid yet")]
    NotYetValid,
    #[display("credentials revoked")]
    Revoked,
    #[display("credentials already used")]
    Spent,
    #[display("not allowed: {_0}")]
    Forbidden(#[error(not(source))] String),
    #[display("auth store unavailable: {_0}")]
    Unavailable(#[error(not(source))] String),
    #[display("auth misconfigured: {_0}")]
    Misconfigured(#[error(not(source))] String),
}
```

How an `AuthError` becomes an RPC status:

| `AuthError` | Connect code |
|---|---|
| `Invalid`, `Expired`, `NotYetValid`, `Revoked`, `Spent` | `UNAUTHENTICATED` |
| `Forbidden` | `PERMISSION_DENIED` |
| `Unavailable` | `UNAVAILABLE` |
| `Misconfigured` | `INTERNAL` |

The message names the failure (for example "credentials expired"). It never contains the
secret or a key.

`foundation_auth::AuthToken` gets two new variants, so `AuthContext::is_authenticated()` is
true for our callers: `Certificate { fingerprint_sha256, expires_at }` and
`SignedSecret { secret_id, expires_at }`. It also gets `is_expired_at(now_unix)`, so core
code does not use the global `Utc::now()` in `is_expired()`. Only `foundation_auth` matches
on `AuthToken` today, so the new variants break no other crate.

### 8.2 The RPC interceptor (in `foundation_connectrpc`)

An **interceptor** is code that runs around each RPC handler. It already exists in
connectrpc (`shared/interceptor`). This one runs a `ClientAuth`.

```rust
// foundation_connectrpc/src/shared/auth.rs (new part, feature "auth")
pub use foundation_auth::client_auth::{AuthInfo, CallCredentials, ClientAuth};

/// Runs a `ClientAuth` before each handler and puts the `AuthInfo` into
/// the call `Ctx`, so `get_auth_info(&ctx)` and `AuthzInterceptor` work.
#[derive(Clone)]
pub struct ClientAuthInterceptor {
    inner: Arc<ClientAuthInner>,
}

struct ClientAuthInner {
    auth: Arc<dyn ClientAuth>,
    public_procedures: HashSet<String>,
}

impl ClientAuthInterceptor {
    #[must_use]
    pub fn new(auth: Arc<dyn ClientAuth>) -> Self {
        Self { inner: Arc::new(ClientAuthInner { auth, public_procedures: HashSet::new() }) }
    }

    /// Let `procedure` run without credentials.
    #[must_use]
    pub fn allow_unauthenticated(self, procedure: impl Into<String>) -> Self { todo!("T11") }
}

impl ClientAuthInner {
    async fn check(&self, ctx: &Ctx) -> ConnectResult<Option<AuthInfo>> {
        let procedure = ctx.spec().procedure.as_str();
        if self.public_procedures.contains(procedure) {
            return Ok(None);
        }
        let call = CallCredentials {
            procedure,
            headers: &ctx.request.headers,
            connection: &ctx.request.connection,
            extensions: &ctx.request.extensions,
        };
        match self.auth.authenticate(call).await {
            Ok(Some(info)) => Ok(Some(info)),
            Ok(None) => Err(ConnectError::new(Code::Unauthenticated, "no credentials").into()),
            Err(e) => Err(ConnectError::new(auth_error_code(&e), e.to_string()).into()),
        }
    }
}

impl Interceptor for ClientAuthInterceptor {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let inner = Arc::clone(&self.inner);
        Arc::new(move |ctx: Ctx, call: UnaryCall| {
            let inner = Arc::clone(&inner);
            let next = next.clone();
            Box::pin(async move {
                let ctx = match inner.check(&ctx).await? {
                    Some(info) => ctx.with_extension(info),
                    None => ctx,
                };
                next(ctx, call).await
            })
        })
    }

    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        // The same as `wrap_unary`, with `StreamCall` in place of `UnaryCall`.
        todo!("T11")
    }

    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        next // client side: nothing to check
    }
}
```

What the code does: for each call, the interceptor builds `CallCredentials` from the call
`Ctx` and asks the provider. On success it adds `AuthInfo` to the `Ctx` that the handler
gets. On failure the handler never runs. It needs the `build_ctx` fix of task T3: today
dispatch drops the request extensions, so host facts such as `CfTlsClientAuth` would never
arrive.

### 8.3 The providers

| Provider | Credential | Hosts | Store use per call |
|---|---|---|---|
| `SignedSecretAuth` | `Authorization: Bearer <signed secret>`, purpose `access` | Worker, web, native (TLS server auth) | a revocation read; a use-count write only with a limit |
| `MtlsAuth` | the client certificate in `ConnectionContext::tls` | native | none; `touch` writes at most once per 75 s |
| `JwtAuth` | `Authorization: Bearer <JWT>` | all | none |
| `CfMtlsAuth` | `CfTlsClientAuth` in the request extensions | Worker | one client read (D1) when `require_registered` |
| `ForwardedPrincipalAuth` | the header `ewe-forwarded-principal` | Durable Object only | none |
| `InsecureNoAuth` | nothing | development | none |
| `AnyOf` | the first provider that applies | all | the chosen provider's |

```rust
// foundation_auth/src/client_auth/signed_secret.rs
pub struct SignedSecretAuth {
    verifier: SecretVerifier,              // purpose = Access, audience = this host
    trust: Arc<TrustedClients>,
    policy: AccessPolicy,
}

#[derive(Debug, Clone)]
pub struct AccessPolicy {
    /// `None` = no limit from the server; the token `max_uses` still applies.
    pub max_uses: Option<u32>,
    pub roles: Vec<String>,
}

impl ClientAuth for SignedSecretAuth {
    fn mode(&self) -> &'static str { "signed-secret" }

    fn authenticate<'a>(&'a self, call: CallCredentials<'a>) -> AuthFuture<'a> {
        let token = bearer_from(call.headers);             // case-insensitive "Bearer"
        Box::pin(async move {
            let Some(token) = token else { return Ok(None) };
            if !looks_like_signed_secret(&token) {
                return Ok(None);                            // maybe a JWT: let AnyOf try the next
            }
            let verified = self.verifier.verify(&token).map_err(secret_error_to_auth)?;
            self.trust.check_secret_not_revoked(&verified.id).await.map_err(trust_error_to_auth)?;
            if let Some(client_id) = verified.subject.as_deref() {
                // An access secret from enrollment or refresh: the client must
                // still be enrolled and not revoked (section 9.1).
                self.trust.check_client_not_revoked(client_id).await.map_err(trust_error_to_auth)?;
            }
            if let Some(limit) = effective_limit(self.policy.max_uses, verified.max_uses) {
                self.trust.spend_secret(&verified, limit).await.map_err(trust_error_to_auth)?;
            }
            Ok(Some(auth_info_for_secret(&verified, &self.policy.roles)))
        })
    }
}
```

```rust
// foundation_auth/src/client_auth/mtls.rs
pub struct MtlsAuth {
    trust: Arc<TrustedClients>,
    env: Env,
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
            let now = self.env.unix_secs();
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

```rust
// foundation_auth/src/client_auth/jwt.rs
pub struct JwtAuth {
    verifier: JwtVerifier,
    env: Env,
}
// Uses the new `JwtVerifier::verify_at(token, now_unix)`, which sets
// jwt-simple's `VerificationOptions::artificial_time` from the Env clock.
```

```rust
// foundation_auth/src/client_auth/any_of.rs
/// Tries providers in order. The first `Ok(Some)` wins. An `Err` stops the
/// chain: the caller showed credentials of that kind and they failed, so a
/// weaker provider must not accept the call instead.
pub struct AnyOf(pub Vec<Arc<dyn ClientAuth>>);
```

```rust
// foundation_auth/src/client_auth/insecure.rs
/// Accepts every call. Refuses a non-loopback peer unless
/// `allow_non_loopback` is set (a Worker has no peer address, so a Worker
/// must set it). Logs a warning at start. Development only.
pub struct InsecureNoAuth {
    pub allow_non_loopback: bool,
}
```

### 8.4 Wiring examples

Native host with enrollment and mTLS (the default, the same behavior as today):

```rust
// host::native::security (task T17)
let trust = TrustedClients::open(Arc::new(sql_trust), env.clone(), TrustConfig::native_default()).await?;
let auth: Arc<dyn ClientAuth> = Arc::new(MtlsAuth::new(Arc::clone(&trust), env.clone()));
let router = core.router(Arc::clone(&auth));      // adds ClientAuthInterceptor
```

Native host that also accepts access secrets (for example a browser behind the same host):

```rust
let auth: Arc<dyn ClientAuth> = Arc::new(AnyOf(vec![
    Arc::new(MtlsAuth::new(Arc::clone(&trust), env.clone())),
    Arc::new(SignedSecretAuth::new(access_verifier, Arc::clone(&trust), AccessPolicy::default())),
]));
```

Worker (public key from configuration, D1 trust store, optional Cloudflare mTLS):

```rust
// host::worker::config (task T21)
let keys = EnvKeyStore::from_worker(&bindings)?;                  // AGENT_RPC_SECRET_PUBLIC_KEYS
let verifier = SecretVerifier::new(
    SecretDomain::AGENT_RPC,
    KeyMaterial::verify_only(Arc::new(keys), env.clone())
        .verifying_keys(&[KeyId::new(KeyId::SECRET_SIGNING)?])
        .await?,
    env.clone(),
    VerifyPolicy::access(config.public_url.clone()),
);
let trust = TrustedClients::open(Arc::new(SqlTrustStore::new(d1)), env.clone(), TrustConfig::worker_default()).await?;
let auth: Arc<dyn ClientAuth> = Arc::new(AnyOf(vec![
    Arc::new(CfMtlsAuth::new(Arc::clone(&trust))),
    Arc::new(SignedSecretAuth::new(verifier, trust, AccessPolicy::default())),
]));
```

The constructors and presets that the examples use (task T11 writes them):

```rust
impl MtlsAuth { pub fn new(trust: Arc<TrustedClients>, env: Env) -> Self; }
impl SignedSecretAuth { pub fn new(verifier: SecretVerifier, trust: Arc<TrustedClients>, policy: AccessPolicy) -> Self; }
impl CfMtlsAuth { pub fn new(trust: Arc<TrustedClients>) -> Self; }        // require_registered = true
impl JwtAuth { pub fn new(verifier: JwtVerifier, env: Env) -> Self; }
impl AccessPolicy { pub fn default() -> Self; }                            // max_uses = None, roles = ["agent-rpc:client"]
impl VerifyPolicy {
    pub fn enroll() -> Self;                         // purpose Enroll, no audience, 24 h, 5 min skew
    pub fn access(audience: impl Into<String>) -> Self; // purpose Access, this audience, 30 days, 5 min skew
}
impl TrustConfig {
    pub fn native_default() -> Self;                 // Preload, 75 s, 5 min, 1 h, shared_store = false
    pub fn worker_default() -> Self;                 // StoreOnly, 75 s, 5 min, 1 h, shared_store = true
}
impl EnvKeyStore {
    /// Native: reads EWE_AGENT_RPC_SIGNING_KEY (inline PEM or a path), as today.
    pub fn from_process_env() -> Result<Self, KeyStoreError>;
    /// Worker: reads the var AGENT_RPC_SECRET_PUBLIC_KEYS, a JSON list of
    /// {"kid": "...", "alg": "ed25519", "public_key": "<base64url>"}.
    #[cfg(target_family = "wasm")]
    pub fn from_worker(bindings: &worker::Env) -> Result<Self, KeyStoreError>;
}
```

## 9. Enrollment and the security model

### 9.1 Enrollment (`foundation_agent_rpc::core::enroll`)

**The owner's design (PR #126 review, 2026-10-26).** The bootstrap payload is one base64 string.
It holds everything that a client needs: the server address, a secret, a signature that makes
it tamper-proof, and the server's public key. With it, **any** client (a native program, a
browser page or a Worker) can connect, check that it talks to the right server, and exchange the
secret for a credential that it then uses on every call. Enrollment needs no client certificate
and no pinned TLS channel.

How the payload carries each item:

| Item | Where in the payload | Signed? |
|---|---|---|
| Server address | `BootstrapPayload.url`, and the same value as `aud` inside the token | yes, through `aud` |
| Secret | `token.secret` (32 random bytes) | yes |
| Signature | `token.signature` | it is the signature |
| Server public key | `token.public_key` (with `kid`) | yes |

#### 9.1.1 How the client knows that it talks to the right server

TLS proves only that the client talks to *some* holder of a certificate for that name. A proxy or
Cloudflare can end that TLS. So enrollment adds an application-level proof that does not depend on
TLS:

1. The client makes 32 random bytes, the **nonce**.
2. It sends the nonce in `EnrollRequest`.
3. The server signs a **server proof** with the private key whose public key is in the token:

   ```text
   "ewe.agent_rpc.enroll_proof.v1" 0x00
   u32be(len) secret_id          the SHA-256 of the token secret
   u32be(len) nonce
   u32be(len) client_key         the client public key, or the SHA-256 of the CSR
   u32be(len) client_id
   ```

4. The client checks the proof with `token.public_key`. A server without that private key cannot
   make it. So a wrong server, or a changed `url`, fails here.

A native client can also pin the TLS certificate (`tls_fingerprint_sha256`). That is extra
protection on a self-signed host; it is not required for enrollment.

#### 9.1.2 The two credential kinds

`EnrollRequest` carries one of two things. The client picks:

| Kind | Who uses it | What the client sends | What the client gets | How later calls authenticate |
|---|---|---|---|---|
| **Client key** (`EnrollRequest.client_key`) | every client; the only kind for a browser or a Worker | a public key (Ed25519 or P-256) whose private key never leaves the client. A browser makes it with WebCrypto as a non-extractable key and keeps it in IndexedDB. | `client_id` and an **access secret**: a `SignedSecret` v2 with `purpose = access`, `aud = url`, `sub = client_id`, and a short window | `Authorization: Bearer <access secret>`, checked by `SignedSecretAuth`. Before it expires, the client calls `RefreshAccess`, signed with its private key, to get a new one. |
| **Client certificate** (`EnrollRequest.csr_pem`) | native clients that want mTLS (as today) | a certificate signing request | a client certificate and the CA certificate | mTLS, checked by `TrustedClientVerifier` and `MtlsAuth` |

Both kinds record the client in `TrustedClients` (by key fingerprint or certificate fingerprint),
so revocation works the same way.

#### 9.1.3 Code

```rust
// foundation_agent_rpc/src/core/enroll.rs
/// Signs CSRs. Only a native host has a CA (`foundation_auth::pki`).
pub trait ClientCertIssuer: Send + Sync + 'static {
    fn sign_client_csr(&self, csr_pem: &str, client_name: &str, lifetime: Duration)
        -> Result<IssuedClientCert, AgentRpcError>;
    fn ca_cert_pem(&self) -> &str;
}

pub struct EnrollmentService {
    verifier: SecretVerifier,                  // purpose = Enroll, audience = this host's url
    signer: SecretIssuer,                      // the payload key: server proofs, access secrets
    issuer: Option<Arc<dyn ClientCertIssuer>>, // None on a Worker: no certificate kind
    trust: Arc<TrustedClients>,
    env: Env,
    config: EnrollConfig,
}

impl EnrollmentService {
    /// Verify the secret, check the credential, spend one use, record the
    /// client, and return the credential with a server proof.
    pub async fn enroll(&self, req: &EnrollRequest) -> AgentRpcResult<EnrollResponse> {
        validate_client_name(&req.client_name)?;
        let nonce = check_nonce(&req.nonce)?;                             // exactly 32 bytes
        let verified = self.verifier.verify(&req.token)?;                 // 1. signature, purpose, aud, window
        self.trust.check_secret_not_revoked(&verified.id).await?;         // 2. not revoked
        let client_id = self.env.ids().next_id().to_string();
        let (record, credential) = match &req.credential {               // 3. a bad credential spends no use
            Some(Credential::ClientKey(key)) => {
                let key = ClientPublicKey::parse(key)?;                  // Ed25519 or P-256, size checked
                let access = self.signer.issue(&SecretRequest::access_for(
                    self.config.access_lifetime, self.verifier.audience(), &client_id))?;
                (TrustedClient::for_key(&client_id, &req.client_name, &key, self.env.unix_secs()),
                 Issued::Access { access_secret: access.encode().to_string() })
            }
            Some(Credential::CsrPem(csr)) => {
                let issuer = self.issuer.as_ref().ok_or(AgentRpcError::Unsupported("client certificates"))?;
                let cert = issuer.sign_client_csr(csr, &req.client_name, self.config.client_cert_lifetime)?;
                (TrustedClient::for_cert(&client_id, &req.client_name, &cert, self.env.unix_secs()),
                 Issued::Certificate { cert_pem: cert.cert_pem, ca_cert_pem: issuer.ca_cert_pem().into() })
            }
            None => return Err(AgentRpcError::InvalidArgument("credential is required".into())),
        };
        self.trust.spend_secret(&verified, self.config.max_enrollments_per_secret).await?; // 4. atomic
        self.trust.record_enrollment(record).await?;                      // 5. durable record
        let proof = self.signer.sign_enroll_proof(&verified.id, nonce, &req.credential, &client_id)?; // 6.
        Ok(EnrollResponse::new(client_id, credential, proof))
    }

    /// A new access secret for an enrolled client-key client. The request is
    /// signed with the client's private key over
    /// "ewe.agent_rpc.refresh.v1" 0x00 client_id unix_ms nonce.
    pub async fn refresh_access(&self, req: &RefreshAccessRequest) -> AgentRpcResult<RefreshAccessResponse> {
        let client = self.trust.client_by_id(&req.client_id).await?      // 1. known, not revoked
            .ok_or(AgentRpcError::Unauthenticated)?;
        client.check_refresh_signature(req, self.env.wall_now(), self.config.clock_skew)?; // 2. signature, time
        self.trust.mark_refresh_nonce(&req.client_id, &req.nonce).await?; // 3. each nonce once
        let access = self.signer.issue(&SecretRequest::access_for(
            self.config.access_lifetime, self.verifier.audience(), &req.client_id))?;
        Ok(RefreshAccessResponse { access_secret: access.encode().to_string(), ..Default::default() })
    }
}
```

What the code does: `enroll` checks the token, then the credential, and only then spends a use,
so a bad request never wastes a token. It records the client and signs the server proof last.
`refresh_access` gives a client-key client a new short access secret. The client proves that it
still holds its private key, and the trust store refuses a revoked client.

```rust
// foundation_agent_rpc/src/core/config.rs
/// Enrollment limits (rule 8).
#[derive(Debug, Clone)]
pub struct EnrollConfig {
    /// Default 90 days: the lifetime of a Let's Encrypt certificate (as today).
    pub client_cert_lifetime: Duration,
    /// Lifetime of an access secret that enrollment or refresh issues.
    /// Default 1 hour: the lifetime of a Google OAuth 2.0 access token.
    pub access_lifetime: Duration,
    /// Allowed difference between client and server clocks on a refresh.
    /// Default 5 minutes (as `VerifyPolicy::clock_skew`).
    pub clock_skew: Duration,
    /// Default 1: an enrollment secret enrolls one client (as today).
    pub max_enrollments_per_secret: u32,
    /// `Enroll` and `RefreshAccess` calls per peer address. Default 1 per
    /// second with a burst of 5: the `limit_req` example in the nginx documentation.
    pub rate: RateLimit,
    /// Largest CSR in PEM. Default 8 KiB: the size of one nginx
    /// `large_client_header_buffers` buffer. An RSA 8192-bit CSR is about 3.3 KiB.
    pub max_csr_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub per_second: u32,
    pub burst: u32,
}
```

The rate limit is a small token bucket in memory, one per peer address, on the `Env` clock. We do
not use `foundation_db`'s `AsyncRateLimiterStore`: its backends read the global clock
(`unix_now_secs()`), so DST could not move their time. That is recorded as a bug in
`docs/bug-backlog.md`. On a Worker, the bucket lives in the isolate, so it is a per-isolate
limit; Cloudflare rate limiting rules can add a global limit.

If the process stops between step 4 and step 5, the secret is spent and no client is recorded.
The client then gets an error, and the operator issues a new secret. This is safe: it can only
refuse a client, it can never accept one twice.

**Which hosts enroll.** A host that enrolls must hold the private key of the payload, because it
signs server proofs and access secrets. The native host has it. A Worker host enrolls only when
`WorkerHostConfig::enrollment` is set: the private key then comes from a Worker secret, sealed as
[key-protection.md](./key-protection.md) requires. A Worker without it stays verify-only and
accepts only access secrets that an operator's tool issued. See open question 4 in
[feature.md](./feature.md).

This sequence diagram shows enrollment with a client key, for example from a browser page. The
TLS channel uses the normal certificate roots; no pin and no client certificate are needed.

```mermaid
sequenceDiagram
    autonumber
    participant Op as Operator
    participant H as Host (native or Worker)
    participant C as Client (browser, Worker or native)
    participant T as Trust store
    Op->>H: ask for a payload
    H->>H: SecretIssuer.issue (purpose enroll, aud = url, window from Env clock)
    H-->>Op: BootstrapPayload (url, enroll_url, signed secret with public key)
    Op->>C: give the payload (string or QR code)
    C->>C: decode; check signature, window, and aud = url
    C->>C: make a key pair (WebCrypto, non-extractable) and a nonce
    C->>H: Enroll(secret, client name, client public key, nonce) over HTTPS
    H->>H: SecretVerifier.verify (signature, purpose, aud, window with skew)
    H->>T: is the secret revoked?
    T-->>H: no
    H->>T: consume one use (atomic)
    H->>T: record the client (client_id, key fingerprint)
    H->>H: sign server proof (nonce, client key, client_id) and an access secret (sub = client_id)
    H-->>C: client_id, access secret, server proof
    C->>C: check the server proof with the payload public key
    C->>H: RPC calls with Authorization: Bearer <access secret>
    H->>H: SignedSecretAuth: signature, aud, window, client not revoked
    C->>H: before expiry: RefreshAccess(client_id, time, nonce, signature by client key)
    H-->>C: a new access secret
```

This sequence diagram shows the native certificate kind. It is the flow of today, plus the nonce
and the server proof.

```mermaid
sequenceDiagram
    autonumber
    participant C as Native client
    participant H as Native host
    participant T as Trust store
    C->>H: TLS to enroll_url (pinned fingerprint, if the payload has one)
    C->>H: Enroll(secret, client name, CSR, nonce)
    H->>H: verify the secret; sign the CSR (CA)
    H->>T: consume one use; record the client (certificate fingerprint)
    H-->>C: client certificate, CA certificate, server proof
    C->>C: check the server proof
    C->>H: mTLS to the agent listener
    H->>H: TrustedClientVerifier checks the index (no I/O)
    C->>H: an RPC call
    H->>H: MtlsAuth finds the client, touch writes last-seen (await)
    H-->>C: the reply
```

### 9.2 Threat model

| Threat | Asset at risk | Defense |
|---|---|---|
| Someone copies a payload or an enrollment secret | the right to enroll | One use only (atomic count). A short window (24 hours). The operator gives the payload over a trusted path. Revocation by secret ID. |
| Someone forges a secret | the right to enroll or call | Ed25519 signature with a domain-separated message. Unknown keys fail. |
| Someone changes the purpose or audience of a secret | the right to call another host | Both fields are inside the signature (version 2). |
| Someone sits between the client and the enrollment endpoint, or changes the `url` in the payload | the enrollment | The server proof (section 9.1.1): only the holder of the payload private key can sign the client's nonce. `aud = url` is inside the token signature. A native client can also pin the certificate. |
| Someone steals an access secret (client-key kind) | the right to call until it expires | Short window (`access_lifetime`, default 1 hour); revoking the client stops it at once (`SignedSecretAuth` checks `sub`); a new one needs the client private key (`RefreshAccess`). |
| Someone steals a client private key | the right to call | The key never leaves the client (a browser key is non-extractable). Revoke the client; the next call fails (`MtlsAuth` and `SignedSecretAuth` check each call). |
| Someone steals the database | private keys | Private keys in databases are sealed with a KEK that is not in the database. AEAD binds each blob to its record. |
| Someone copies an old sealed record back (rollback) | an old key | Not blocked: it needs a counter outside the database. Recorded as a limit; the operator rotates keys after a breach. |
| A wrong client clock | false refusals | `clock_skew` of 5 minutes at both ends of the window; certificates start 5 minutes in the past. |
| A wrong server clock | false acceptance of an expired secret | The window check uses the server clock; operators run time sync. DST tests skew on both sides. |
| Many enrollment attempts | CPU (CSR checks, signing) | A rate limit per peer address on `Enroll` and `RefreshAccess` (`EnrollConfig::rate`, default 1 per second with a burst of 5 — the `limit_req` example in the nginx documentation). Bad secrets fail before the credential is parsed. |
| A client sends a false `ewe-forwarded-principal` header to a Worker | another identity in the DO | The Worker deletes every `ewe-forwarded-*` header before auth. The DO is reachable only through the Worker binding. |
| One client uses another client's instance | sessions and history | Instance names are scoped by owner when `ScopePolicy::PerOwner` is set (owner decision 1 in [feature.md](./feature.md)). |
| Secrets or keys in logs | all secrets | Redacting `Debug` and `Display` (as today). A log test captures the log output at trace level for the whole flow (`auth_log_tests` moves to the native host tests). |

### 9.3 Replay protection

- **Enrollment secrets:** one use (atomic in SQL), inside a short window. A replay gets `UNAUTHENTICATED` ("credentials already used"). The server proof binds the reply to the client's own nonce and key, so a recorded reply is useless to anyone else.
- **Refresh requests:** signed by the client key over a time and a nonce; the time must be inside `clock_skew` and each nonce works once (`mark_refresh_nonce`).
- **Access secrets:** a bearer credential works for many calls inside its window, like an API token. Defenses: the audience binds it to one host; the window is limited (30 days by default); `max_uses` can limit it; revocation by ID stops it at once; TLS protects it in transit.
- **Cross-purpose replay:** a version 1 secret is always `enroll`. `SignedSecretAuth` requires `access`. So an enrollment secret never works as a bearer credential.

### 9.4 Revocation

| What | How | When it takes effect |
|---|---|---|
| One client certificate | `TrustedClients::revoke_client` | The next handshake (verifier) and the next call (`MtlsAuth`). |
| One client-key client | `TrustedClients::revoke_client` | The next call (`SignedSecretAuth` checks `sub`) and the next `RefreshAccess`. |
| One secret | `TrustedClients::revoke_secret` | The next enrollment or call that shows it. |
| All secrets of a key | Remove the public key from the verifiers (rotation) | When the host reloads its configuration. |
| All client certificates | Make a new CA; clients enroll again | At once for new handshakes. |

On a Worker, D1 has read replicas only when the Sessions API is used. The Worker host does
not use it for the trust store, so a revocation is visible to the next call.

### 9.5 Clock skew

- The window check allows `clock_skew` (default 5 minutes) at both ends.
- `CertificateAuthority` backdates `not_before` by `backdate` (default 5 minutes), as today.
- The client checks the window before it dials. If the client clock is wrong, the error says so ("outside its validity window by this machine's clock"), as today.
- `TrustConfig::expired_grace` keeps use rows until the secret window plus the skew has passed. So a secret cannot come back after its count row is pruned.
- In a Worker, `Date.now()` moves only between input/output events (a Cloudflare defense against timing attacks). This changes nothing for windows measured in minutes.

## 10. Tests

All tests go in each crate's `tests/` directory (CLAUDE.md rule 7). The **contract tests**
are one set of tests that every implementation of a trait must pass. A macro runs the same
test bodies against each implementation.

| Test | Crate and file | Implementations it runs against |
|---|---|---|
| `KeyStore` contract | `foundation_auth/tests/keystore/contract.rs` | `MemoryKeyStore`, `FsKeyStore` (temp dir), `DbKeyStore<MemoryStorage>`, `DbKeyStore<JsonFileStorage>`, `DbKeyStore<TursoStorage>`; `DbKeyStore<D1Store>` behind `--features d1-tests` |
| Sealing | `foundation_auth/tests/keystore/sealing.rs` | `KekSealer`, `PassphraseSealer`: a blob opens only with the same AAD; a wrong KEK fails; nonces come from `Env` |
| Load-or-create race | `foundation_auth/tests/keystore/race.rs` | Two `KeyMaterial` over one store; both get the same key |
| Legacy files | `foundation_auth/tests/keystore/legacy_fs.rs` | A state directory copied from the branch tests loads unchanged |
| `TrustStore` contract | `foundation_auth/tests/trust/contract.rs` | `MemoryTrustStore`, `KvTrustStore<MemoryStorage>`, `SqlTrustStore<TursoStorage>`; D1 behind a feature |
| Atomic use count | `foundation_auth/tests/trust/concurrent_uses.rs` | 16 concurrent `consume_secret_use` calls with one free use: exactly one `Counted` (SQL backends) |
| Verifier | `foundation_auth/tests/mtls/verifier.rs` | Active, revoked, expired, unknown; and no task or write happens (a counting `TrustStore` fake sees zero calls) |
| Providers | `foundation_auth/tests/client_auth/*.rs` | Each provider with `SimClock`: missing, valid, expired, revoked, spent, wrong purpose, wrong audience |
| Interceptor | `foundation_connectrpc/tests/client_auth_interceptor_tests.rs` | `AuthInfo` reaches the handler; failures never run the handler; public procedures skip auth |
