# Authentication in `foundation_auth`

_Part of [feature 11](./feature.md); the shared architecture is [agent-rpc-architecture.md](../../agent-rpc-architecture.md). Written in ASD-STE100 Simplified Technical English (STE).
Code and identifiers stay as they are._

**Abbreviations in this file.** authentication (auth); remote procedure call (RPC);
Certificate Authority (CA); certificate signing request (CSR); Transport Layer Security
(TLS); mutual TLS (mTLS), where both sides show a certificate; JavaScript Object Notation
(JSON); JSON Web Token (JWT); Public-Key Cryptography Standards number 8 (PKCS#8), a file
format for private keys; Privacy-Enhanced Mail (PEM), a text form of keys and
certificates; Distinguished Encoding Rules (DER), a binary form; Secure Hash Algorithm with
256 bits (SHA-256); Rivest–Shamir–Adleman (RSA); Probabilistic Signature Scheme (PSS);
Elliptic Curve Digital Signature Algorithm (ECDSA); key encryption key (KEK); identifier (ID); uniform resource locator (URL);
Durable Object (DO); WebAssembly (wasm); Structured Query Language (SQL); key-value store
(KV); input/output (I/O); deterministic simulation testing (DST); Request for Comments (RFC);
application programming interface (API); central processing unit (CPU); Hypertext Transfer
Protocol (HTTP); Standards for Efficient Cryptography number 1 (SEC1), a file format for
elliptic-curve private keys; Secure Shell (SSH); Representational State Transfer (REST); Domain Name System (DNS); Internet
Protocol (IP); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.
D1 and R2 are names of Cloudflare products (a SQL database and an object store), not
abbreviations.
The [glossary in feature.md](../../agent-rpc-architecture.md#2-glossary) explains each term.

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
| `secret.rs` `SigningKeyConfig`, `default_state_dir`, `write_secret_file` | `foundation_auth::keys::{KeySource, PrivateKey}`; `foundation_agent_rpc::host::native::state_dir` | The operator makes the key; the host only loads it (section 4). |
| `auth.rs` `CertificateAuthority`, `IssuedClientCert`, `ServerIdentity` | `foundation_auth::pki` (native) | Time and randomness come from `Env`. |
| `auth.rs` `CertificateAuthority::load_or_generate` | `foundation_auth::pki::CertificateAuthority::from_key` | The CA key is an operator file (section 4.5); the host makes only the certificate. |
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
├── keys/                 KeySource, PrivateKey, KeyLoadError           all targets
│   ├── mod.rs
│   ├── source.rs         a file (native only) or an inline value
│   └── parse.rs          OpenSSH, PKCS#8 and SEC1 PEM to PKCS#8 DER
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
# Signed secrets with Ed25519 (pure Rust, all targets). `ssh-key` reads
# OpenSSH private key files (section 4.3).
secret = ["dep:ed25519-compact", "dep:ssh-key"]
# Operator-supplied RSA-PSS keys for signed secrets. `ring` is native only.
secret-rsa = ["secret", "dep:ring"]
# The CA and certificate issuance. `rcgen` + `x509-parser` verify CSRs with
# `ring`, so this is native only.
pki = ["secret", "dep:rcgen", "dep:p256", "dep:x509-parser", "dep:ring"]
# rustls verifiers and config builders. Native only.
mtls = ["pki", "dep:rustls"]
# Fakes for tests: MemoryTrustStore, seeded test keys, scripted providers.
testing = ["foundation_compact/testing", "foundation_core/testing"]

[dependencies]
ed25519-compact = { version = "2.6", default-features = false, features = ["std", "pem"], optional = true }
ssh-key = { version = "0.6", default-features = false, features = ["alloc", "ed25519", "p256"], optional = true }

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
| `keys` with `KeySource::Inline` | yes | `ssh-key`, `ed25519-compact` and `p256` are pure Rust. |
| `keys` with `KeySource::File` | no | No file system in a Worker. |
| `pki` | no | CSR verification uses `ring` through `x509-parser`. |
| `trust` | yes | `foundation_db` async traits only. |
| `mtls` | no | `rustls` with `ring`. A Worker has no TLS listener anyway: Cloudflare terminates TLS. |
| `client_auth` | yes | Pure logic over headers, connection facts and stores. |

## 3. Signed secrets (`foundation_auth::secret`)

A **signed secret** is a short string that proves that a key holder made it. It has 32
random bytes, a validity window and a signature. An operator gives it to a client, often
inside a `BootstrapPayload` (see [proto.md](../12-agent-rpc-core/proto.md)). It is **our own format, not a
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

In the proof of concept the operator rotates a key by a restart with a new key file. The
verifier can hold the old public key until its last secret expires (section 4.6). Rotation with
no restart is task K6 of [feature 15](../15-key-protection-at-rest/feature.md).

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

## 4. Key material (`foundation_auth::keys`)

**Proof of concept (owner direction, 2026-10-27; decision 11 in [feature.md](../../agent-rpc-architecture.md)).**
The operator makes each private key with a standard tool (`ssh-keygen` or `openssl`). The host
loads the key from a file, an environment variable or a Worker secret, and uses it. The host
does not make, seal, store or rotate private keys.

These parts moved to the deferred
[feature 15](../15-key-protection-at-rest/feature.md):

- a key store over `foundation_db` backends;
- keys that the host makes and stores, and the race between two processes;
- encryption at rest with a KEK;
- remote key management services;
- passphrase-protected key files;
- rotation by `kid` with no gap.

### 4.1 The keys of a host

| Key | Algorithm | Used for | Which hosts |
|---|---|---|---|
| Secret-signing key | Ed25519 (RSA-PSS on native with feature `secret-rsa`, as today) | Signs signed secrets and the server proof of enrollment (section 9.1.1) | Native. A Worker only when `WorkerHostConfig::enrollment` is set |
| CA key | ECDSA P-256 (default) or Ed25519 | Signs client CSRs and the server certificate (section 5) | Native |
| Server key | ECDSA P-256 (default) or Ed25519 | The key of the TLS server certificate (section 5.3) | Native |

A verify-only host (a Worker without enrollment) holds no private key. It reads only public
keys from configuration (section 4.5).

### 4.2 How the operator makes the keys

```sh
# Secret-signing key, Ed25519. Either tool works.
ssh-keygen -t ed25519 -N "" -C agent-rpc-signing -f bootstrap-signing-key.pem
openssl genpkey -algorithm ed25519 -out bootstrap-signing-key.pem

# CA key and server key, ECDSA P-256 (every TLS client accepts it).
ssh-keygen -t ecdsa -b 256 -N "" -C agent-rpc-ca -f ca-key.pem
openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out server-key.pem

chmod 600 *.pem
```

`-N ""` makes a key with no passphrase. The proof of concept refuses a key with a passphrase
(`KeyLoadError::Encrypted`). `ssh-keygen` also writes a `.pub` file. The host does not need it:
it gets the public key from the private key.

### 4.3 The formats that the host reads

| Format | First line | Algorithms | Parser |
|---|---|---|---|
| OpenSSH private key, no passphrase | `-----BEGIN OPENSSH PRIVATE KEY-----` | Ed25519, ECDSA `nistp256` | `ssh-key` 0.6 (pure Rust, all targets; already in the workspace through `foundation_keychain`) |
| PKCS#8 PEM, version 1 and 2 | `-----BEGIN PRIVATE KEY-----` | Ed25519, ECDSA P-256; RSA on native with `secret-rsa` | `ed25519-compact`, `p256`, `ring` (as today) |
| SEC1 PEM (`openssl ecparam -genkey`) | `-----BEGIN EC PRIVATE KEY-----` | ECDSA P-256 | `p256` |

Any other input gives `KeyLoadError::UnsupportedFormat`. This includes an encrypted PKCS#8
file (`-----BEGIN ENCRYPTED PRIVATE KEY-----`) and an OpenSSH key with a passphrase, which give
`KeyLoadError::Encrypted`. A key with the wrong algorithm for its role gives
`KeyLoadError::WrongAlgorithm`. An example is a P-256 key as the secret-signing key.

The `kid` of a key is the first 16 hexadecimal characters of the SHA-256 of its public key.
Configuration can set another `kid`.

### 4.4 The code

```rust
// foundation_auth/src/keys/mod.rs
use std::path::PathBuf;

use foundation_compact::crypto::SecureBytes;

/// Where a private key comes from. The operator makes the key; the host
/// reads it one time, at start.
#[derive(Clone)]
pub enum KeySource {
    /// A file (native only). On Unix the host refuses a file that the
    /// group or others can read, as OpenSSH does ("UNPROTECTED PRIVATE
    /// KEY FILE"). `NativeHostConfig::allow_loose_key_permissions` turns
    /// the check off for development.
    #[cfg(not(target_family = "wasm"))]
    File(PathBuf),
    /// The key text from an environment variable or a Worker secret: the
    /// PEM text, or the PEM text in standard base64 (for a system that
    /// cannot keep line breaks in a variable).
    Inline(SecureBytes),
}

impl KeySource {
    /// Reads an environment variable as the native host does today with
    /// `EWE_AGENT_RPC_SIGNING_KEY`: a value that starts with `-----BEGIN`
    /// is PEM; a value that decodes from base64 to PEM is base64; any
    /// other value is a file path.
    ///
    /// # Errors
    /// [`KeyLoadError::NotFound`] when the variable is not set.
    #[cfg(not(target_family = "wasm"))]
    pub fn from_env_var(name: &str) -> Result<Self, KeyLoadError> { todo!("T7") }

    /// A Worker secret (`wrangler secret put AGENT_RPC_SIGNING_KEY`):
    /// PEM text or base64 PEM, never a path.
    ///
    /// # Errors
    /// [`KeyLoadError::NotFound`] when the secret is not bound.
    #[cfg(target_family = "wasm")]
    pub fn from_worker_secret(bindings: &worker::Env, name: &str) -> Result<Self, KeyLoadError> { todo!("T7") }
}

/// The algorithm of a loaded private key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivateKeyAlgorithm {
    Ed25519,
    EcdsaP256,
    /// Native with feature `secret-rsa` only.
    Rsa,
}

/// A private key that the host loaded, with its public key. `Debug`
/// prints the algorithm and the `kid`, never key bytes. The key bytes are
/// zeroed on drop.
pub struct PrivateKey {
    algorithm: PrivateKeyAlgorithm,
    kid: String,
    /// The key in PKCS#8 DER form, whatever the input format was.
    pkcs8_der: SecureBytes,
    public_key: Vec<u8>,
}

impl PrivateKey {
    /// Reads and parses the key. File I/O runs inside this async function,
    /// like `foundation_db::JsonFileStorage`.
    ///
    /// # Errors
    /// [`KeyLoadError`].
    pub async fn load(source: &KeySource) -> Result<Self, KeyLoadError> { todo!("T7") }

    /// Parses key text in one of the formats of section 4.3.
    ///
    /// # Errors
    /// [`KeyLoadError::UnsupportedFormat`], [`KeyLoadError::Encrypted`].
    pub fn parse(text: &[u8]) -> Result<Self, KeyLoadError> { todo!("T7") }

    /// Sets a `kid` from configuration in place of the derived one.
    #[must_use]
    pub fn with_kid(self, kid: impl Into<String>) -> Self { todo!("T7") }

    #[must_use]
    pub fn algorithm(&self) -> PrivateKeyAlgorithm { self.algorithm }
    #[must_use]
    pub fn kid(&self) -> &str { &self.kid }

    /// The key as a secret-signing key (section 3.4).
    ///
    /// # Errors
    /// [`KeyLoadError::WrongAlgorithm`] for a P-256 key.
    pub fn into_signing_key(self) -> Result<SigningKey, KeyLoadError> { todo!("T7") }

    /// The PKCS#8 DER bytes, for the CA and the server identity (section 5).
    #[cfg(feature = "pki")]
    #[must_use]
    pub fn pkcs8_der(&self) -> &[u8] { self.pkcs8_der.as_ref() }
}

#[derive(Debug, Clone, PartialEq, Eq, derive_more::Display, derive_more::Error)]
pub enum KeyLoadError {
    /// The message names the path or variable and the command that makes
    /// the key (section 4.2).
    #[display("no key at {_0}; make one with: ssh-keygen -t ed25519 -N \"\" -f {_0}")]
    NotFound(#[error(not(source))] String),
    #[display("cannot read key {_0}")]
    Io(#[error(not(source))] String),
    #[display("key file {_0} can be read by other users; run: chmod 600 {_0}")]
    LoosePermissions(#[error(not(source))] String),
    #[display("the key has a passphrase; the proof of concept reads only keys made with -N \"\"")]
    Encrypted,
    #[display("unsupported key format: {_0}")]
    UnsupportedFormat(#[error(not(source))] String),
    #[display("this role needs a {expected:?} key, found {found:?}")]
    WrongAlgorithm { expected: PrivateKeyAlgorithm, found: PrivateKeyAlgorithm },
}
```

What the code does: `KeySource` says where the key text is. `PrivateKey::load` reads it,
finds the format from the first line and turns every format into PKCS#8 DER. The rest of
`foundation_auth` uses only that one form, so the signing and certificate code does not know
the input format.

### 4.5 Where each host reads its keys

| Key | Native: flag, or environment variable | Native default | Worker |
|---|---|---|---|
| Secret-signing key | `--signing-key`, or `EWE_AGENT_RPC_SIGNING_KEY` (a path, PEM or base64 PEM, as today plus base64) | `<state>/bootstrap-signing-key.pem` | Secret `AGENT_RPC_SIGNING_KEY`, only with `WorkerHostConfig::enrollment` |
| CA key | `--ca-key`, or `EWE_AGENT_RPC_CA_KEY` | `<state>/ca-key.pem` | none (Cloudflare terminates TLS) |
| Server key | `--server-key`, or `EWE_AGENT_RPC_SERVER_KEY` | `<state>/server-key.pem` | none |
| Public verifying keys | from the secret-signing key, plus `--extra-verifying-key <path>` (repeatable, section 4.6) | — | Var `AGENT_RPC_SECRET_PUBLIC_KEYS`: a JSON list of `{"kid": "...", "alg": "ed25519", "public_key": "<base64url>"}` |

- **A missing key stops the host.** The error names the path and the command of section 4.2.
  The host never makes a key in place of a missing one.
- **Several native replicas** get the same key files from the operator. The trusted-client
  store is the shared database (section 6), as before.
- **DST and tests** use `KeySource::Inline` with fixture keys from
  `foundation_auth/tests/fixtures/` (made with the commands of section 4.2), or keys that
  `foundation_auth::testing::ed25519_pem(seed)` and `p256_pem(seed)` make from `SeededEntropy`.

```rust
// foundation_auth/src/secret/keys.rs (added)
impl VerifyingKey {
    /// Parses the JSON list of `AGENT_RPC_SECRET_PUBLIC_KEYS`.
    ///
    /// # Errors
    /// [`KeyLoadError::UnsupportedFormat`] for bad JSON or an unknown `alg`.
    pub fn list_from_json(text: &str) -> Result<Vec<Self>, KeyLoadError> { todo!("T7") }
}
```

### 4.6 Rotation in the proof of concept

The operator makes a new key file and restarts the host. Secrets signed by the old key then fail
verification at once. Clients enroll again, or get new access secrets.

To keep old secrets valid during a change, give the verifier the old public key too:
`--extra-verifying-key <path>` on native (a `.pub` file or a PEM public key), or one more entry
in `AGENT_RPC_SECRET_PUBLIC_KEYS` on a Worker. `SecretVerifier` already takes a list of keys
(section 3.5). Remove the old key when its last secret has expired. Rotation with no restart
and no manual steps is task K6 of [feature 15](../15-key-protection-at-rest/feature.md).

A new CA key makes every client certificate invalid. Clients enroll again (section 9.4).

### 4.7 The legacy files of today

A state directory of today holds `ca-cert.pem`, `ca-key.pem`, `bootstrap-signing-key.pem` and
`trusted-clients.db`. The branch code made the two key files in PKCS#8 PEM with mode 0600.

1. They are the default paths of section 4.5, so a state directory of today loads with no change and no migration step.
2. `ca-cert.pem` is reused when its public key matches `ca-key.pem`. Otherwise the host makes a new self-signed CA certificate from the key and writes it (section 5.1). A certificate is public, so the host can write it.
3. Today the host makes a new server key at each start. Now `server-key.pem` is an operator file. If it is missing, the host stops with the command to make it (section 4.2).
4. `EWE_AGENT_RPC_SIGNING_KEY` keeps its meaning (inline PEM or a path) and also accepts base64 PEM.
5. `trusted-clients.db` keeps its tables. `SqlTrustStore` adds one new table (section 6.2).

### 4.8 What the proof of concept does not protect

The private keys are plain text in a file or a variable. A person or a process that can read
the file, or the environment of the host process, has the key. The proof of concept limits this:

- The host refuses a key file that other users can read (section 4.4).
- Keys never go into a database, a log line, an error message, a payload or `Debug` output.
- On a Worker, the key is a Worker secret: Cloudflare encrypts it at rest and shows it only to
  the bound Worker.
- `SecureBytes` zeroes the key bytes on drop.

Encryption at rest with a KEK, and keys that never leave a hardware module, are
[feature 15](../15-key-protection-at-rest/feature.md).

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
    /// The algorithm that the operator's key must have (section 4.1).
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
    /// The CA over the operator's CA key (section 4.5). Reuses
    /// `existing_cert_pem` (today's `ca-cert.pem`) when its public key is
    /// the key's public key. Otherwise it makes a new self-signed
    /// certificate, with dates from `env` time, and returns its PEM so that
    /// the host writes it.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn from_key(key: &PrivateKey, existing_cert_pem: Option<&str>, config: &CaConfig, env: &Env) -> Result<(Self, Option<String>), PkiError> { todo!("T8") }

    /// A server certificate for `names` over the operator's server key.
    ///
    /// # Errors
    /// [`PkiError`].
    pub fn issue_server_identity(&self, server_key: &PrivateKey, names: &[String], lifetime: Duration) -> Result<ServerIdentity, PkiError> { todo!("T8") }

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

1. The host reads `<state>/server-cert.pem`. The server key is the operator's file (section 4.5).
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
backend (owner decision 7 in [feature.md](../../agent-rpc-architecture.md): a separate feature).

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
let keys = VerifyingKey::list_from_json(&bindings.var("AGENT_RPC_SECRET_PUBLIC_KEYS")?.to_string())?;
let verifier = SecretVerifier::new(
    SecretDomain::AGENT_RPC,
    keys,
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
`WorkerHostConfig::enrollment` is set: the private key then comes from the Worker secret
`AGENT_RPC_SIGNING_KEY` (PEM text or base64 PEM, section 4.5). A Worker without it stays verify-only and
accepts only access secrets that an operator's tool issued. See open question 4 in
[feature.md](../../agent-rpc-architecture.md).

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
| Someone steals the database | private keys | No private key is in a database. The trust store holds only public keys, fingerprints and counts. |
| Someone reads a key file or the host environment | the secret-signing, CA and server keys | Proof of concept: the host refuses a key file that other users can read; Worker secrets are encrypted at rest by Cloudflare. Not blocked for a person with the host user's rights. Recorded as a limit (section 4.8); encryption at rest is [feature 15](../15-key-protection-at-rest/feature.md). After a breach, the operator makes new keys (section 4.6). |
| A wrong client clock | false refusals | `clock_skew` of 5 minutes at both ends of the window; certificates start 5 minutes in the past. |
| A wrong server clock | false acceptance of an expired secret | The window check uses the server clock; operators run time sync. DST tests skew on both sides. |
| Many enrollment attempts | CPU (CSR checks, signing) | A rate limit per peer address on `Enroll` and `RefreshAccess` (`EnrollConfig::rate`, default 1 per second with a burst of 5 — the `limit_req` example in the nginx documentation). Bad secrets fail before the credential is parsed. |
| A client sends a false `ewe-forwarded-principal` header to a Worker | another identity in the DO | The Worker deletes every `ewe-forwarded-*` header before auth. The DO is reachable only through the Worker binding. |
| One client uses another client's instance | sessions and history | Instance names are scoped by owner when `ScopePolicy::PerOwner` is set (owner decision 1 in [feature.md](../../agent-rpc-architecture.md)). |
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
| Key loading | `foundation_auth/tests/keys/load.rs` | Fixture keys made by `ssh-keygen` (Ed25519, ECDSA P-256) and `openssl` (PKCS#8 Ed25519 and P-256, SEC1 P-256); inline PEM; base64 PEM; a key with a passphrase fails with `Encrypted`; a mode-0644 file fails with `LoosePermissions`; a P-256 key as the signing key fails with `WrongAlgorithm`; the public key and `kid` match the fixtures |
| Formats agree | `foundation_auth/tests/keys/formats_agree.rs` | One Ed25519 key in OpenSSH form and in PKCS#8 form gives the same public key and the same signature |
| Missing key | `foundation_agent_rpc/tests/native_missing_key_tests.rs` | A host with no key file does not start, makes no key, and the error names the path and the `ssh-keygen` command |
| Legacy files | `foundation_auth/tests/keys/legacy_state_dir.rs` | A state directory copied from the branch tests loads unchanged; `ca-cert.pem` is reused |
| No key in output | `foundation_auth/tests/keys/debug_output_tests.rs` | `Debug` and error text of `PrivateKey`, `KeySource` and `SigningKey` contain no key bytes |
| `TrustStore` contract | `foundation_auth/tests/trust/contract.rs` | `MemoryTrustStore`, `KvTrustStore<MemoryStorage>`, `SqlTrustStore<TursoStorage>`; D1 behind a feature |
| Atomic use count | `foundation_auth/tests/trust/concurrent_uses.rs` | 16 concurrent `consume_secret_use` calls with one free use: exactly one `Counted` (SQL backends) |
| Verifier | `foundation_auth/tests/mtls/verifier.rs` | Active, revoked, expired, unknown; and no task or write happens (a counting `TrustStore` fake sees zero calls) |
| Providers | `foundation_auth/tests/client_auth/*.rs` | Each provider with `SimClock`: missing, valid, expired, revoked, spent, wrong purpose, wrong audience |
| Interceptor | `foundation_connectrpc/tests/client_auth_interceptor_tests.rs` | `AuthInfo` reaches the handler; failures never run the handler; public procedures skip auth |
