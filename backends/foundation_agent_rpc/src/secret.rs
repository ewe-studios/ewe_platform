//! Signed, time-boxed bootstrap secrets.
//!
//! WHY: a client starts with nothing but a secret handed to it out of band (a
//! string or a QR code). That secret must prove the server issued it, must stop
//! working after a while, and must be unusable once the server forgets its key.
//!
//! WHAT:
//! - [`SigningKey`] — the server's private key, used **only** to sign
//!   bootstrap secrets (Ed25519 by default; an operator-supplied RSA key is
//!   accepted too). Loaded from the environment variable named in
//!   [`SigningKeyConfig`] when set, else from a file in the state directory,
//!   generated there when missing.
//! - [`SignedSecret`] — 32 random bytes plus a validity window, signed. Its
//!   shared form is base64url of a JSON object `{v, alg, public_key, secret,
//!   valid_from, valid_until, signature}`; the bootstrap payload (and so the QR
//!   code) carries exactly that string.
//! - [`BootstrapSecrets`] — issues secrets and verifies presented ones.
//!
//! HOW: the signature covers a domain-separated, length-prefixed encoding of
//! the algorithm, public key, secret and window, never the JSON text (JSON is
//! not canonical). Verification uses the server's own key: a secret signed by
//! any other key — including this server's previous key, after the key file
//! was lost and regenerated — is refused. Single use is enforced by the
//! trusted-client store ([`crate::trust`]), which records each secret's id
//! (the SHA-256 of its random bytes, never the bytes).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine;
use foundation_errstacks::ErrorTrace;
use ring::signature::{self, KeyPair as _};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

use crate::auth::sha256_fingerprint;
use crate::error::{AgentRpcError, AgentRpcResult};

/// The format version of a [`SignedSecret`]'s JSON form.
pub const SECRET_FORMAT_VERSION: u32 = 1;

/// The environment variable [`SigningKeyConfig`] reads by default.
pub const DEFAULT_SIGNING_KEY_ENV: &str = "EWE_AGENT_RPC_SIGNING_KEY";

/// File name of the signing key inside a state directory.
pub const SIGNING_KEY_FILE: &str = "bootstrap-signing-key.pem";

/// Domain separation: these signatures can never be confused with any other
/// use of the key.
const SIGNING_CONTEXT: &[u8] = b"ewe.agent_rpc.bootstrap_secret.v1";

/// Bytes of entropy in a secret. A security parameter, not a tunable limit:
/// 256 bits, as the token it replaces.
const SECRET_BYTES: usize = 32;

const PEM_BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
const PEM_END: &str = "-----END PRIVATE KEY-----";

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

fn config_err(what: impl Into<String>) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Config(what.into()))
}

fn auth_err(what: impl Into<String>) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Auth(what.into()))
}

/// Seconds since the Unix epoch (0 for times before it).
#[must_use]
pub fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The default state directory: `$XDG_STATE_HOME/ewe/agent-rpc`, else
/// `$HOME/.local/state/ewe/agent-rpc` (the XDG base-directory default).
///
/// # Errors
/// [`AgentRpcError::Config`] when neither variable is set.
pub fn default_state_dir() -> AgentRpcResult<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".local").join("state"))
        })
        .ok_or_else(|| config_err("no state directory: set XDG_STATE_HOME or HOME"))?;
    Ok(base.join("ewe").join("agent-rpc"))
}

// ---------------------------------------------------------------------------
// SigningKey

/// The algorithm a [`SigningKey`] signs with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningAlgorithm {
    /// Ed25519 (RFC 8032). What [`SigningKey::generate`] creates.
    Ed25519,
    /// RSA-PSS with SHA-256, 2048–8192-bit keys (operator-supplied keys only;
    /// RSA keys are never generated here).
    RsaPssSha256,
}

impl SigningAlgorithm {
    /// The name used in a secret's `alg` field.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ed25519 => "ed25519",
            Self::RsaPssSha256 => "rsa-pss-sha256",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        match name {
            "ed25519" => Some(Self::Ed25519),
            "rsa-pss-sha256" => Some(Self::RsaPssSha256),
            _ => None,
        }
    }

    fn verifier(self) -> &'static dyn signature::VerificationAlgorithm {
        match self {
            Self::Ed25519 => &signature::ED25519,
            Self::RsaPssSha256 => &signature::RSA_PSS_2048_8192_SHA256,
        }
    }
}

enum Pair {
    Ed25519(signature::Ed25519KeyPair),
    Rsa(signature::RsaKeyPair),
}

/// The private key that signs bootstrap secrets. Never printed; its PKCS#8
/// bytes are zeroed on drop.
pub struct SigningKey {
    alg: SigningAlgorithm,
    pkcs8: Zeroizing<Vec<u8>>,
    public_key: Vec<u8>,
    pair: Pair,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigningKey")
            .field("alg", &self.alg)
            .field("public_key_sha256", &self.public_key_fingerprint())
            .finish_non_exhaustive()
    }
}

/// Where [`SigningKey::load_or_generate`] got the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SigningKeyOrigin {
    /// From the configured environment variable (inline PEM or a path).
    Environment,
    /// Loaded from this file.
    Loaded(PathBuf),
    /// Generated and written to this file (there was none).
    Generated(PathBuf),
}

impl SigningKey {
    /// A new Ed25519 key.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when key generation fails.
    pub fn generate() -> AgentRpcResult<Self> {
        let rng = ring::rand::SystemRandom::new();
        let pkcs8 = signature::Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|e| config_err(format!("generating the signing key: {e}")))?;
        Self::from_pkcs8_der(pkcs8.as_ref())
    }

    /// Parse a PKCS#8 private key (DER): Ed25519, or RSA (2048–8192 bits).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for anything else.
    pub fn from_pkcs8_der(der: &[u8]) -> AgentRpcResult<Self> {
        let pkcs8 = Zeroizing::new(der.to_vec());
        if let Ok(pair) = signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(der) {
            return Ok(Self {
                alg: SigningAlgorithm::Ed25519,
                public_key: pair.public_key().as_ref().to_vec(),
                pkcs8,
                pair: Pair::Ed25519(pair),
            });
        }
        match signature::RsaKeyPair::from_pkcs8(der) {
            Ok(pair) => Ok(Self {
                alg: SigningAlgorithm::RsaPssSha256,
                public_key: pair.public().as_ref().to_vec(),
                pkcs8,
                pair: Pair::Rsa(pair),
            }),
            Err(e) => Err(config_err(format!(
                "signing key is neither an Ed25519 nor an RSA PKCS#8 key: {e}"
            ))),
        }
    }

    /// Parse a PKCS#8 PEM (`-----BEGIN PRIVATE KEY-----`).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for another PEM label, bad base64, or an
    /// unsupported key.
    pub fn from_pkcs8_pem(pem: &str) -> AgentRpcResult<Self> {
        let body = pem
            .trim()
            .strip_prefix(PEM_BEGIN)
            .and_then(|rest| rest.trim_end().strip_suffix(PEM_END))
            .ok_or_else(|| config_err("signing key must be a PKCS#8 \"PRIVATE KEY\" PEM"))?;
        let compact: Zeroizing<String> =
            Zeroizing::new(body.chars().filter(|c| !c.is_whitespace()).collect());
        let der = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(compact.as_bytes())
                .map_err(|e| config_err(format!("signing key PEM: {e}")))?,
        );
        Self::from_pkcs8_der(&der)
    }

    /// The key as PKCS#8 PEM, for persisting. Treat as a secret.
    #[must_use]
    pub fn to_pkcs8_pem(&self) -> Zeroizing<String> {
        let encoded = Zeroizing::new(base64::engine::general_purpose::STANDARD.encode(&*self.pkcs8));
        let mut pem = Zeroizing::new(String::with_capacity(encoded.len() + 64));
        pem.push_str(PEM_BEGIN);
        pem.push('\n');
        for line in encoded.as_bytes().chunks(64) {
            // base64 output is ASCII.
            pem.push_str(std::str::from_utf8(line).unwrap_or_default());
            pem.push('\n');
        }
        pem.push_str(PEM_END);
        pem.push('\n');
        pem
    }

    /// The signing algorithm.
    #[must_use]
    pub fn algorithm(&self) -> SigningAlgorithm {
        self.alg
    }

    /// The public key (raw Ed25519 bytes, or DER `RSAPublicKey`).
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Lowercase hex SHA-256 of the public key — safe to log.
    #[must_use]
    pub fn public_key_fingerprint(&self) -> String {
        sha256_fingerprint(&self.public_key)
    }

    fn sign(&self, message: &[u8]) -> AgentRpcResult<Vec<u8>> {
        match &self.pair {
            Pair::Ed25519(pair) => Ok(pair.sign(message).as_ref().to_vec()),
            Pair::Rsa(pair) => {
                let rng = ring::rand::SystemRandom::new();
                let mut sig = vec![0u8; pair.public().modulus_len()];
                pair.sign(&signature::RSA_PSS_SHA256, &rng, message, &mut sig)
                    .map_err(|e| config_err(format!("signing the bootstrap secret: {e}")))?;
                Ok(sig)
            }
        }
    }

    /// Load the key named by `config`, generating one when it has none.
    ///
    /// 1. The environment variable, when set and non-empty: an inline PKCS#8
    ///    PEM, or the path of one. It must load — an operator who names a key
    ///    gets that key or an error, never a substitute.
    /// 2. Otherwise `config.path()`: loaded when it exists; generated (Ed25519)
    ///    and written there with owner-only permissions when it does not. A
    ///    lost key file therefore yields a new key on the next start, which
    ///    invalidates every secret signed by the old one.
    ///
    /// A key file that exists but does not parse is an error, not a reason to
    /// overwrite it.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for an unreadable or invalid key, or a key
    /// file that cannot be written.
    pub fn load_or_generate(config: &SigningKeyConfig) -> AgentRpcResult<(Self, SigningKeyOrigin)> {
        if let Some(var) = &config.env_var {
            if let Some(value) = std::env::var_os(var).filter(|v| !v.is_empty()) {
                let value = Zeroizing::new(value.to_string_lossy().into_owned());
                let key = if value.trim_start().starts_with("-----BEGIN") {
                    Self::from_pkcs8_pem(&value)
                } else {
                    Self::read_pem_file(Path::new(value.trim()))
                }
                .map_err(|e| {
                    config_err(format!("signing key from ${var}: {}", e.current_context()))
                })?;
                return Ok((key, SigningKeyOrigin::Environment));
            }
        }
        match std::fs::metadata(&config.path) {
            Ok(_) => Ok((
                Self::read_pem_file(&config.path)?,
                SigningKeyOrigin::Loaded(config.path.clone()),
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let key = Self::generate()?;
                write_secret_file(&config.path, key.to_pkcs8_pem().as_bytes())?;
                tracing::info!(
                    path = %config.path.display(),
                    public_key_sha256 = %key.public_key_fingerprint(),
                    "generated a new bootstrap signing key (earlier bootstrap secrets are now invalid)"
                );
                Ok((key, SigningKeyOrigin::Generated(config.path.clone())))
            }
            Err(e) => Err(config_err(format!(
                "signing key {}: {e}",
                config.path.display()
            ))),
        }
    }

    fn read_pem_file(path: &Path) -> AgentRpcResult<Self> {
        let pem = Zeroizing::new(
            std::fs::read_to_string(path)
                .map_err(|e| config_err(format!("reading {}: {e}", path.display())))?,
        );
        Self::from_pkcs8_pem(&pem)
    }
}

/// Write `bytes` to `path` atomically (temp file + rename), creating parent
/// directories, readable by the owner only on Unix.
pub(crate) fn write_secret_file(path: &Path, bytes: &[u8]) -> AgentRpcResult<()> {
    use std::io::Write;
    let io = |what: &str, e: std::io::Error| config_err(format!("{what} {}: {e}", path.display()));
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| io("creating the directory of", e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|e| io("creating", e))?;
    file.write_all(bytes).map_err(|e| io("writing", e))?;
    file.sync_all().map_err(|e| io("syncing", e))?;
    drop(file);
    std::fs::rename(&tmp, path).map_err(|e| io("installing", e))
}

/// Where the signing key comes from.
#[derive(Debug, Clone)]
pub struct SigningKeyConfig {
    env_var: Option<String>,
    path: PathBuf,
}

impl SigningKeyConfig {
    /// The key file at `path`, overridable by [`DEFAULT_SIGNING_KEY_ENV`].
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            env_var: Some(DEFAULT_SIGNING_KEY_ENV.to_string()),
            path: path.into(),
        }
    }

    /// [`SIGNING_KEY_FILE`] inside `state_dir`, overridable by
    /// [`DEFAULT_SIGNING_KEY_ENV`].
    #[must_use]
    pub fn in_state_dir(state_dir: impl AsRef<Path>) -> Self {
        Self::at(state_dir.as_ref().join(SIGNING_KEY_FILE))
    }

    /// Read the override from `name` instead of [`DEFAULT_SIGNING_KEY_ENV`].
    #[must_use]
    pub fn with_env_var(mut self, name: impl Into<String>) -> Self {
        self.env_var = Some(name.into());
        self
    }

    /// Ignore the environment; use the file only.
    #[must_use]
    pub fn without_env_var(mut self) -> Self {
        self.env_var = None;
        self
    }

    /// The key file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

// ---------------------------------------------------------------------------
// SignedSecret

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    v: u32,
    alg: String,
    public_key: String,
    secret: String,
    valid_from: u64,
    valid_until: u64,
    signature: String,
}

/// A bootstrap secret: random bytes, a validity window, and the server's
/// signature over both. `Debug`/`Display` never show the secret.
#[derive(Clone)]
pub struct SignedSecret {
    alg: SigningAlgorithm,
    public_key: Vec<u8>,
    secret: Zeroizing<Vec<u8>>,
    valid_from: u64,
    valid_until: u64,
    signature: Vec<u8>,
}

impl std::fmt::Debug for SignedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignedSecret")
            .field("alg", &self.alg)
            .field("valid_from", &self.valid_from)
            .field("valid_until", &self.valid_until)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Display for SignedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted bootstrap secret>")
    }
}

fn signed_message(
    alg: SigningAlgorithm,
    public_key: &[u8],
    secret: &[u8],
    valid_from: u64,
    valid_until: u64,
) -> Zeroizing<Vec<u8>> {
    let mut m = Zeroizing::new(Vec::with_capacity(
        SIGNING_CONTEXT.len() + public_key.len() + secret.len() + 64,
    ));
    m.extend_from_slice(SIGNING_CONTEXT);
    m.push(0);
    for part in [alg.as_str().as_bytes(), public_key, secret] {
        // Every part is far below 4 GiB.
        m.extend_from_slice(&u32::try_from(part.len()).unwrap_or(u32::MAX).to_be_bytes());
        m.extend_from_slice(part);
    }
    m.extend_from_slice(&valid_from.to_be_bytes());
    m.extend_from_slice(&valid_until.to_be_bytes());
    m
}

impl SignedSecret {
    /// The shared form: base64url (unpadded) of the JSON object. This is
    /// what goes in the bootstrap payload and the QR code.
    #[must_use]
    pub fn encode(&self) -> Zeroizing<String> {
        let mut envelope = Envelope {
            v: SECRET_FORMAT_VERSION,
            alg: self.alg.as_str().to_string(),
            public_key: b64().encode(&self.public_key),
            secret: b64().encode(&*self.secret),
            valid_from: self.valid_from,
            valid_until: self.valid_until,
            signature: b64().encode(&self.signature),
        };
        // Serializing plain strings and integers cannot fail.
        let json = Zeroizing::new(serde_json::to_vec(&envelope).unwrap_or_default());
        envelope.secret.zeroize();
        Zeroizing::new(b64().encode(&*json))
    }

    /// Parse the shared form. This checks the format only — use
    /// [`BootstrapSecrets::verify`] (server) or
    /// [`verify_self_signed`](Self::verify_self_signed) (client) for the
    /// signature.
    ///
    /// # Errors
    /// [`AgentRpcError::Codec`] for invalid base64/JSON, an unknown version
    /// or algorithm, a wrong-length secret, or an empty window.
    pub fn decode(encoded: &str) -> AgentRpcResult<Self> {
        let codec = |what: String| -> ErrorTrace<AgentRpcError> {
            AgentRpcError::Codec(format!("bootstrap secret: {what}")).into()
        };
        let json = Zeroizing::new(
            b64()
                .decode(encoded.trim())
                .map_err(|e| codec(format!("not base64url: {e}")))?,
        );
        let mut envelope: Envelope =
            serde_json::from_slice(&json).map_err(|e| codec(format!("not a secret: {e}")))?;
        let secret = Zeroizing::new(
            b64()
                .decode(envelope.secret.as_bytes())
                .map_err(|e| codec(format!("secret: {e}")))?,
        );
        envelope.secret.zeroize();
        if envelope.v != SECRET_FORMAT_VERSION {
            return Err(codec(format!(
                "version {} (this build reads {SECRET_FORMAT_VERSION})",
                envelope.v
            )));
        }
        let alg = SigningAlgorithm::parse(&envelope.alg)
            .ok_or_else(|| codec(format!("unknown algorithm {:?}", envelope.alg)))?;
        if secret.len() != SECRET_BYTES {
            return Err(codec(format!("secret must be {SECRET_BYTES} bytes")));
        }
        if envelope.valid_until <= envelope.valid_from {
            return Err(codec("empty validity window".into()));
        }
        Ok(Self {
            alg,
            public_key: b64()
                .decode(envelope.public_key.as_bytes())
                .map_err(|e| codec(format!("public_key: {e}")))?,
            secret,
            valid_from: envelope.valid_from,
            valid_until: envelope.valid_until,
            signature: b64()
                .decode(envelope.signature.as_bytes())
                .map_err(|e| codec(format!("signature: {e}")))?,
        })
    }

    /// The signing algorithm.
    #[must_use]
    pub fn algorithm(&self) -> SigningAlgorithm {
        self.alg
    }

    /// The public key the secret claims to be signed by.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// Start of the validity window (Unix seconds).
    #[must_use]
    pub fn valid_from(&self) -> u64 {
        self.valid_from
    }

    /// End of the validity window (Unix seconds, exclusive).
    #[must_use]
    pub fn valid_until(&self) -> u64 {
        self.valid_until
    }

    /// Whether `now` (Unix seconds) is inside the window.
    #[must_use]
    pub fn is_valid_at(&self, now: u64) -> bool {
        self.valid_from <= now && now < self.valid_until
    }

    /// The secret's id: lowercase hex SHA-256 of its random bytes. Safe to
    /// store and log; it cannot be turned back into the secret.
    #[must_use]
    pub fn id(&self) -> String {
        sha256_fingerprint(&self.secret)
    }

    /// Whether the signature verifies against the public key the secret
    /// carries. A client-side integrity check (catches a mistyped or
    /// truncated secret before contacting the server); it does not prove who
    /// issued it — only the server's own key does that.
    #[must_use]
    pub fn verify_self_signed(&self) -> bool {
        self.verify_with(&self.public_key)
    }

    fn verify_with(&self, public_key: &[u8]) -> bool {
        let message = signed_message(
            self.alg,
            &self.public_key,
            &self.secret,
            self.valid_from,
            self.valid_until,
        );
        signature::UnparsedPublicKey::new(self.alg.verifier(), public_key)
            .verify(&message, &self.signature)
            .is_ok()
    }
}

// ---------------------------------------------------------------------------
// BootstrapSecrets

/// A secret that passed [`BootstrapSecrets::verify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedSecret {
    /// See [`SignedSecret::id`].
    pub id: String,
    /// End of the validity window (Unix seconds).
    pub valid_until: u64,
}

/// Issues and verifies [`SignedSecret`]s with the server's [`SigningKey`].
#[derive(Debug)]
pub struct BootstrapSecrets {
    key: SigningKey,
    default_validity: Duration,
}

impl BootstrapSecrets {
    /// Default validity of an issued secret: 24 hours, the default TTL of a
    /// kubeadm bootstrap token (`kubeadm token create --ttl`).
    pub const DEFAULT_VALIDITY: Duration = Duration::from_secs(24 * 3600);

    /// Sign with `key`; secrets last [`Self::DEFAULT_VALIDITY`].
    #[must_use]
    pub fn new(key: SigningKey) -> Self {
        Self {
            key,
            default_validity: Self::DEFAULT_VALIDITY,
        }
    }

    /// How long [`issue`](Self::issue) makes a secret valid.
    #[must_use]
    pub fn with_default_validity(mut self, validity: Duration) -> Self {
        self.default_validity = validity;
        self
    }

    /// The default validity.
    #[must_use]
    pub fn default_validity(&self) -> Duration {
        self.default_validity
    }

    /// The signing key.
    #[must_use]
    pub fn signing_key(&self) -> &SigningKey {
        &self.key
    }

    /// A secret valid from now for the default validity.
    ///
    /// # Errors
    /// See [`issue_window`](Self::issue_window).
    pub fn issue(&self) -> AgentRpcResult<SignedSecret> {
        self.issue_for(self.default_validity)
    }

    /// A secret valid from now for `validity`.
    ///
    /// # Errors
    /// See [`issue_window`](Self::issue_window).
    pub fn issue_for(&self, validity: Duration) -> AgentRpcResult<SignedSecret> {
        let now = SystemTime::now();
        self.issue_window(now, now + validity)
    }

    /// A secret valid in `[valid_from, valid_until)` (whole seconds).
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] for an empty window or when entropy or
    /// signing fails.
    pub fn issue_window(
        &self,
        valid_from: SystemTime,
        valid_until: SystemTime,
    ) -> AgentRpcResult<SignedSecret> {
        let (from, until) = (unix_secs(valid_from), unix_secs(valid_until));
        if until <= from {
            return Err(config_err("a bootstrap secret needs a non-empty validity window"));
        }
        let mut secret = Zeroizing::new(vec![0u8; SECRET_BYTES]);
        foundation_compact::entropy::fill(&mut secret)
            .map_err(|e| config_err(format!("entropy for the bootstrap secret: {e}")))?;
        let message = signed_message(self.key.alg, &self.key.public_key, &secret, from, until);
        let signature = self.key.sign(&message)?;
        Ok(SignedSecret {
            alg: self.key.alg,
            public_key: self.key.public_key.clone(),
            secret,
            valid_from: from,
            valid_until: until,
            signature,
        })
    }

    /// Verify a presented secret now. See [`verify_at`](Self::verify_at).
    ///
    /// # Errors
    /// As [`verify_at`](Self::verify_at).
    pub fn verify(&self, presented: &str) -> AgentRpcResult<VerifiedSecret> {
        self.verify_at(presented, unix_secs(SystemTime::now()))
    }

    /// Verify `presented` at `now` (Unix seconds): well formed, signed by
    /// **this** server's key, and inside its window. Single use is the
    /// caller's next step ([`crate::trust::TrustedClientStore::consume_secret_use`]).
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] for a malformed secret, one from another key
    /// (including this server's previous key), a bad signature, or a time
    /// outside the window.
    pub fn verify_at(&self, presented: &str, now: u64) -> AgentRpcResult<VerifiedSecret> {
        let secret = SignedSecret::decode(presented)
            .map_err(|e| auth_err(format!("malformed bootstrap secret ({})", e.current_context())))?;
        if secret.alg != self.key.alg || secret.public_key != self.key.public_key {
            return Err(auth_err(
                "bootstrap secret was issued under a different signing key",
            ));
        }
        if !secret.verify_with(&self.key.public_key) {
            return Err(auth_err("bootstrap secret signature is invalid"));
        }
        if now < secret.valid_from {
            return Err(auth_err("bootstrap secret is not valid yet"));
        }
        if now >= secret.valid_until {
            return Err(auth_err("bootstrap secret has expired"));
        }
        Ok(VerifiedSecret {
            id: secret.id(),
            valid_until: secret.valid_until,
        })
    }
}
