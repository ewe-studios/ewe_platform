//! Bundle manifests: generation, signing, and verification (F40).
//!
//! WHY: a bundle version directory is only trustworthy if two independent
//! properties hold. `sha256` per file gives **integrity** — the bytes on disk
//! are the bytes someone declared. An Ed25519 `signature` over the manifest
//! gives **authenticity** — *we* are that someone. Neither is sufficient
//! alone: an attacker who controls the CDN can compute their own hashes, and
//! a signature over nothing says nothing about the files. Together they form
//! the chain: the signature authenticates the hashes, the hashes verify the
//! files.
//!
//! WHAT: the canonical [`Manifest`] schema shared by APK-bundled and OTA
//! manifests, plus the library API that build scripts and the CLI both call —
//! [`ensure_keys`], [`generate_app_manifest`], [`generate_all_manifests`],
//! [`sign_manifest`], [`verify_manifest`], [`verify_file_hash`].
//!
//! HOW: signing covers the *canonical* JSON of every field except
//! `signature` — object keys sorted, no whitespace — so an attacker cannot
//! smuggle meaning through key ordering or formatting. The private key seed
//! never ships: it lives in `keys/ota_private.key` (gitignored) or the
//! `EWE_OTA_PRIVATE_KEY` environment variable. Only the 32-byte public key is
//! baked into the binary.
//!
//! This module is native-only — it does filesystem and key work that has no
//! meaning on wasm32.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use derive_more::{Display, Error};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{info, warn};

/// Manifest schema version understood by this build.
pub const MANIFEST_SCHEMA: u32 = 1;

/// The filename every version directory carries.
pub const MANIFEST_FILENAME: &str = ".ewe_manifest.json";

/// Default CDN domain when `EWE_MANIFEST_DOMAIN` is unset.
pub const DEFAULT_MANIFEST_DOMAIN: &str = "cdn.ewe.studio";

/// Environment variable holding a base64 Ed25519 private key seed.
pub const PRIVATE_KEY_ENV: &str = "EWE_OTA_PRIVATE_KEY";

/// Environment variable overriding the baked CDN domain at build time.
pub const MANIFEST_DOMAIN_ENV: &str = "EWE_MANIFEST_DOMAIN";

/// Directory (relative to a project root) holding the OTA key pair.
pub const KEYS_DIR: &str = "keys";

/// Public key filename — committed to git, baked into every build.
pub const PUBLIC_KEY_FILE: &str = "ota_public.key";

/// Private key seed filename — never committed.
pub const PRIVATE_KEY_FILE: &str = "ota_private.key";

// ── Errors ──────────────────────────────────────────────────────────────

/// Everything that can go wrong generating, signing, or verifying a manifest.
#[derive(Debug, Display, Error)]
pub enum ManifestError {
    /// A filesystem operation failed.
    #[display("manifest I/O at {path}: {source}")]
    Io {
        /// The path being operated on when the failure occurred.
        path: String,
        /// The underlying OS error.
        #[error(source)]
        source: std::io::Error,
    },

    /// The manifest JSON could not be parsed or serialized.
    #[display("manifest JSON: {message}")]
    Json {
        /// What serde reported.
        message: String,
    },

    /// A key file or environment value was not 32 base64-decoded bytes.
    #[display("invalid key material ({context}): {reason}")]
    InvalidKey {
        /// Where the key came from (file path or env var name).
        context: String,
        /// Why it was rejected.
        reason: String,
    },

    /// The manifest carries no `signature` but a public key was supplied.
    #[display("manifest is unsigned but a public key was supplied")]
    MissingSignature,

    /// The signature did not verify against the supplied public key.
    #[display("manifest signature is invalid")]
    BadSignature,

    /// The manifest declares a schema this build does not understand.
    #[display("unsupported manifest schema: {schema} (this build understands {MANIFEST_SCHEMA})")]
    UnsupportedSchema {
        /// The schema version the manifest declared.
        schema: u32,
    },

    /// No private key was available for an operation that requires signing.
    #[display("no private key available — set {PRIVATE_KEY_ENV} or create keys/{PRIVATE_KEY_FILE}")]
    NoPrivateKey,
}

/// Convenience alias for manifest operations.
pub type ManifestResult<T> = Result<T, ManifestError>;

fn io_err(path: &Path, source: std::io::Error) -> ManifestError {
    ManifestError::Io {
        path: path.display().to_string(),
        source,
    }
}

// ── Manifest schema ─────────────────────────────────────────────────────

/// Where a manifest came from.
///
/// Both kinds are signed. A key pair is minted on the first build, so there
/// is no situation in which signing is unavailable and therefore no reason
/// to define what an unsigned manifest would mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ManifestSource {
    /// Generated at build time and shipped inside the APK/IPA/bundle.
    #[display("apk")]
    Apk,

    /// Fetched from the CDN.
    #[display("ota")]
    Ota,
}

/// A single file inside an app bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Path relative to the app's version directory (e.g. `index.html`).
    pub path: String,
    /// Lowercase hex SHA-256 of the file contents.
    pub sha256: String,
    /// Size in bytes. Checked before the hash so a huge body is rejected early.
    pub size: u64,
}

impl std::fmt::Display for ManifestFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} bytes, sha256:{})", self.path, self.size, self.sha256)
    }
}

/// One app's entry in a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestApp {
    /// App identifier — also the directory name under the resource root.
    pub app_id: String,
    /// Semver version of this bundle.
    pub bundle_version: String,
    /// Every file in the bundle, with hash and size.
    pub files: Vec<ManifestFile>,
}

impl std::fmt::Display for ManifestApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}@{} ({} files)",
            self.app_id,
            self.bundle_version,
            self.files.len()
        )
    }
}

/// The canonical manifest schema. APK-bundled and OTA manifests share it —
/// only `source`, `sequence`, and `signature` differ in practice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Schema version. Rejected if it is not [`MANIFEST_SCHEMA`].
    pub schema: u32,
    /// Whether this manifest shipped in the bundle or came over the air.
    pub source: ManifestSource,
    /// The CDN domain this manifest belongs to. Checked against the baked
    /// domain for OTA manifests.
    pub manifest_domain: String,
    /// RFC 3339 timestamp of generation.
    pub created_at: String,
    /// Monotonic counter. OTA manifests must strictly increase; APK uses 0.
    pub sequence: u64,
    /// Base URL for downloads. Must be a sub-path of the baked domain.
    pub base_url: String,
    /// Force activation of this version after processing, if present.
    #[serde(default)]
    pub rollback_to: Option<String>,
    /// Delete this version directory after a forced rollback, if present.
    #[serde(default)]
    pub delete_after: Option<String>,
    /// Base64 Ed25519 signature over the canonical JSON of every other field.
    ///
    /// Always populated by [`generate_app_manifest`]. It stays `Option` on
    /// the wire type so a manifest that *omits* it still deserializes and can
    /// be rejected by [`verify_manifest`] with a reason — a parse error would
    /// say only "bad JSON", which is a worse answer to an attack.
    #[serde(default)]
    pub signature: Option<String>,
    /// Per-app bundle entries.
    pub apps: Vec<ManifestApp>,
}

impl std::fmt::Display for Manifest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Manifest(schema={}, source={}, domain={}, seq={}, apps={}, signed={})",
            self.schema,
            self.source,
            self.manifest_domain,
            self.sequence,
            self.apps.len(),
            self.signature.is_some(),
        )
    }
}

impl Manifest {
    /// Look up an app entry by id.
    #[must_use]
    pub fn app(&self, app_id: &str) -> Option<&ManifestApp> {
        self.apps.iter().find(|a| a.app_id == app_id)
    }

    /// Serialize to pretty JSON for writing to disk.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Json`] if serialization fails.
    pub fn to_json(&self) -> ManifestResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| ManifestError::Json {
            message: e.to_string(),
        })
    }
}

// ── Key pair ────────────────────────────────────────────────────────────

/// An Ed25519 key pair for manifest signing.
///
/// The public half is always present — it is what gets baked into binaries.
/// The private seed is `None` on machines that only verify (every device, and
/// any CI job that is not the release signer).
#[derive(Debug, Clone)]
pub struct KeyPair {
    /// 32-byte Ed25519 public key.
    pub public: [u8; 32],
    /// 32-byte Ed25519 private key seed, when available locally.
    pub private: Option<[u8; 32]>,
}

impl std::fmt::Display for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "KeyPair(public={}, private={})",
            BASE64.encode(self.public),
            if self.private.is_some() { "present" } else { "absent" },
        )
    }
}

impl KeyPair {
    /// Whether this key pair can sign (as opposed to only verify).
    #[must_use]
    pub fn can_sign(&self) -> bool {
        self.private.is_some()
    }
}

/// Decode a base64 string into exactly 32 bytes.
fn decode_key(context: &str, encoded: &str) -> ManifestResult<[u8; 32]> {
    let bytes = BASE64
        .decode(encoded.trim())
        .map_err(|e| ManifestError::InvalidKey {
            context: context.to_string(),
            reason: format!("not valid base64: {e}"),
        })?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| ManifestError::InvalidKey {
        context: context.to_string(),
        reason: format!("expected 32 bytes, got {}", bytes.len()),
    })
}

/// Read the private key seed from the environment, if set.
fn private_key_from_env() -> ManifestResult<Option<[u8; 32]>> {
    match std::env::var(PRIVATE_KEY_ENV) {
        Ok(v) if !v.trim().is_empty() => decode_key(PRIVATE_KEY_ENV, &v).map(Some),
        _ => Ok(None),
    }
}

/// Ensure an Ed25519 key pair exists under `{project_root}/keys/`.
///
/// WHY: signing must be zero-config on a fresh clone — a developer who runs
/// `cargo build` gets a working key pair without reading any docs, and CI
/// overrides the private half through [`PRIVATE_KEY_ENV`] without ever
/// writing it to disk.
///
/// WHAT: returns the key pair, generating one on first call. Idempotent —
/// an existing public key is read, never regenerated, because regenerating
/// would invalidate every already-shipped binary that baked the old key.
///
/// HOW: on first call, generates from the OS CSPRNG, writes both halves
/// base64-encoded, and writes `keys/.gitignore` containing the private key
/// filename so it cannot be committed by accident. The environment variable
/// always wins over the file.
///
/// # Errors
///
/// Returns [`ManifestError::Io`] if the keys directory cannot be created or
/// written, or [`ManifestError::InvalidKey`] if an existing key file does not
/// decode to 32 bytes.
pub fn ensure_keys(project_root: &Path) -> ManifestResult<KeyPair> {
    let keys_dir = project_root.join(KEYS_DIR);
    let public_path = keys_dir.join(PUBLIC_KEY_FILE);
    let private_path = keys_dir.join(PRIVATE_KEY_FILE);

    let env_private = private_key_from_env()?;

    if public_path.exists() {
        let encoded = std::fs::read_to_string(&public_path).map_err(|e| io_err(&public_path, e))?;
        let public = decode_key(&public_path.display().to_string(), &encoded)?;

        // The environment always wins: CI holds the signing key, the working
        // tree usually does not.
        let private = match env_private {
            Some(seed) => Some(seed),
            None if private_path.exists() => {
                let encoded =
                    std::fs::read_to_string(&private_path).map_err(|e| io_err(&private_path, e))?;
                Some(decode_key(&private_path.display().to_string(), &encoded)?)
            }
            None => None,
        };

        if private.is_none() {
            warn!(
                path = %private_path.display(),
                "no OTA private key — manifests will be generated unsigned; set {PRIVATE_KEY_ENV} to sign"
            );
        }

        return Ok(KeyPair { public, private });
    }

    // First run: mint a key pair.
    std::fs::create_dir_all(&keys_dir).map_err(|e| io_err(&keys_dir, e))?;

    let signing = match env_private {
        // CI supplied the seed; derive the public half from it rather than
        // minting a second, conflicting identity.
        Some(seed) => SigningKey::from_bytes(&seed),
        None => SigningKey::generate(&mut rand_core::OsRng),
    };
    let public = signing.verifying_key().to_bytes();
    let private = signing.to_bytes();

    std::fs::write(&public_path, BASE64.encode(public)).map_err(|e| io_err(&public_path, e))?;

    // Only persist a seed we generated ourselves. A seed handed to us through
    // the environment belongs to CI's secret store, not the working tree.
    if env_private.is_none() {
        std::fs::write(&private_path, BASE64.encode(private))
            .map_err(|e| io_err(&private_path, e))?;
    }

    let gitignore = keys_dir.join(".gitignore");
    std::fs::write(&gitignore, format!("{PRIVATE_KEY_FILE}\n"))
        .map_err(|e| io_err(&gitignore, e))?;

    info!(
        path = %keys_dir.display(),
        "generated OTA Ed25519 key pair ({PUBLIC_KEY_FILE} is safe to commit, {PRIVATE_KEY_FILE} is not)"
    );

    Ok(KeyPair {
        public,
        private: Some(private),
    })
}

/// Read just the public key from a file, for baking into a binary.
///
/// # Errors
///
/// Returns [`ManifestError::Io`] if the file cannot be read, or
/// [`ManifestError::InvalidKey`] if it does not decode to 32 bytes.
pub fn read_public_key(path: &Path) -> ManifestResult<[u8; 32]> {
    let encoded = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    decode_key(&path.display().to_string(), &encoded)
}

// ── Canonical JSON + signing ────────────────────────────────────────────

/// Render a `serde_json::Value` with sorted object keys and no whitespace.
///
/// WHY: the signature must cover *meaning*, not formatting. Two byte
/// sequences that deserialize to the same manifest must produce the same
/// signing input, otherwise an attacker could reorder keys or add whitespace
/// to make a signed manifest verify against different bytes than we checked.
fn canonicalize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let sorted: BTreeMap<&String, &serde_json::Value> = map.iter().collect();
            let body = sorted
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        serde_json::Value::String((*k).clone()),
                        canonicalize(v)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        serde_json::Value::Array(items) => {
            let body = items.iter().map(canonicalize).collect::<Vec<_>>().join(",");
            format!("[{body}]")
        }
        other => other.to_string(),
    }
}

/// Build the exact bytes that a manifest signature covers: canonical JSON of
/// every field except `signature`.
fn signing_payload(manifest_json: &str) -> ManifestResult<Vec<u8>> {
    let mut value: serde_json::Value =
        serde_json::from_str(manifest_json).map_err(|e| ManifestError::Json {
            message: e.to_string(),
        })?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("signature");
    }
    Ok(canonicalize(&value).into_bytes())
}

/// Sign a manifest JSON document with an Ed25519 private key seed.
///
/// The `signature` field of the input (if any) is ignored — the signature
/// covers the canonical form of every *other* field.
///
/// # Errors
///
/// Returns [`ManifestError::Json`] if the input is not valid JSON.
pub fn sign_manifest(manifest_json: &str, private_key_seed: &[u8; 32]) -> ManifestResult<String> {
    let payload = signing_payload(manifest_json)?;
    let signing = SigningKey::from_bytes(private_key_seed);
    Ok(BASE64.encode(signing.sign(&payload).to_bytes()))
}

/// Verify a manifest's Ed25519 signature and return the parsed manifest.
///
/// WHY: this is the only step that establishes *who* declared the hashes in
/// the manifest. Everything downstream — file hashes, versions, rollback
/// directives — is only as trustworthy as this check.
///
/// # Errors
///
/// - [`ManifestError::Json`] — the input is not a valid manifest document.
/// - [`ManifestError::UnsupportedSchema`] — the schema version is not understood.
/// - [`ManifestError::MissingSignature`] — no `signature` field is present.
/// - [`ManifestError::BadSignature`] — the signature does not verify, the key
///   is not a valid Ed25519 point, or the signature is malformed.
pub fn verify_manifest(manifest_json: &str, public_key: &[u8; 32]) -> ManifestResult<Manifest> {
    let manifest: Manifest =
        serde_json::from_str(manifest_json).map_err(|e| ManifestError::Json {
            message: e.to_string(),
        })?;

    if manifest.schema != MANIFEST_SCHEMA {
        return Err(ManifestError::UnsupportedSchema {
            schema: manifest.schema,
        });
    }

    let encoded = manifest
        .signature
        .as_deref()
        .ok_or(ManifestError::MissingSignature)?;

    let raw = BASE64.decode(encoded).map_err(|_| ManifestError::BadSignature)?;
    let bytes = <[u8; 64]>::try_from(raw.as_slice()).map_err(|_| ManifestError::BadSignature)?;
    let signature = Signature::from_bytes(&bytes);

    let verifying =
        VerifyingKey::from_bytes(public_key).map_err(|_| ManifestError::BadSignature)?;

    let payload = signing_payload(manifest_json)?;
    verifying
        .verify(&payload, &signature)
        .map_err(|_| ManifestError::BadSignature)?;

    Ok(manifest)
}

/// Verify a file's bytes against the SHA-256 declared in a manifest.
///
/// Returns `false` when the app or path is not in the manifest — an
/// undeclared file is never trusted, which is what makes a manifest a
/// complete description of a bundle rather than a partial one.
#[must_use]
pub fn verify_file_hash(manifest: &Manifest, app_id: &str, path: &str, bytes: &[u8]) -> bool {
    let Some(app) = manifest.app(app_id) else {
        return false;
    };
    let Some(entry) = app.files.iter().find(|f| f.path == path) else {
        return false;
    };
    entry.size == bytes.len() as u64 && entry.sha256 == sha256_hex(bytes)
}

/// Lowercase hex SHA-256 of a byte slice.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().fold(String::with_capacity(64), |mut acc, b| {
        use std::fmt::Write as _;
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

// ── Manifest generation ─────────────────────────────────────────────────

/// RFC 3339 timestamp for "now", without pulling in a date library.
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    // Civil-from-days (Howard Hinnant's algorithm), shifted to a March-based
    // year so leap days land at the end of the cycle.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Collect every regular file under `dir`, relative to it, sorted.
///
/// Dotfiles are skipped — they are build/editor artefacts, and the manifest
/// itself must never describe itself. Sorting keeps manifests reproducible so
/// an unchanged bundle produces byte-identical output.
fn collect_files(dir: &Path) -> ManifestResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    collect_files_into(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn collect_files_into(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> ManifestResult<()> {
    let entries = std::fs::read_dir(dir).map_err(|e| io_err(dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(dir, e))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let file_type = entry.file_type().map_err(|e| io_err(&path, e))?;
        if file_type.is_dir() {
            collect_files_into(root, &path, out)?;
        } else if file_type.is_file() {
            if let Ok(relative) = path.strip_prefix(root) {
                out.push(relative.to_path_buf());
            }
        }
        // Symlinks are deliberately skipped: a manifest describes bytes we
        // shipped, and a link's target is not under our control.
    }
    Ok(())
}

/// Build and sign a manifest for one app directory, writing it to
/// `{app_dir}/.ewe_manifest.json`.
///
/// WHY: every version directory carries a signed manifest — no "maybe it's
/// there", and no "maybe it's signed", for tooling, rollback, or
/// verification to special-case. [`ensure_keys`] mints a pair on the first
/// build, so a signing key is always obtainable and an unsigned manifest is
/// a build misconfiguration rather than a supported mode.
///
/// WHAT: scans `app_dir` recursively, records `sha256` + `size` for each
/// file, and emits a signed `source: "apk"` manifest with `sequence: 0`.
///
/// # Errors
///
/// - [`ManifestError::NoPrivateKey`] if `keypair` cannot sign.
/// - [`ManifestError::Io`] if the directory cannot be scanned or the manifest
///   cannot be written.
/// - [`ManifestError::Json`] on serialization failure.
pub fn generate_app_manifest(
    app_dir: &Path,
    app_id: &str,
    bundle_version: &str,
    manifest_domain: &str,
    keypair: &KeyPair,
) -> ManifestResult<Manifest> {
    // Refuse before doing the work: producing an unsigned manifest would
    // hand every downstream consumer something it must reject anyway.
    let seed = keypair.private.ok_or(ManifestError::NoPrivateKey)?;

    let mut files = Vec::new();
    for relative in collect_files(app_dir)? {
        let full = app_dir.join(&relative);
        let bytes = std::fs::read(&full).map_err(|e| io_err(&full, e))?;
        files.push(ManifestFile {
            path: relative.to_string_lossy().replace('\\', "/"),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
        });
    }

    let mut manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        source: ManifestSource::Apk,
        manifest_domain: manifest_domain.to_string(),
        created_at: now_rfc3339(),
        sequence: 0,
        base_url: format!("https://{manifest_domain}/bundles"),
        rollback_to: None,
        delete_after: None,
        signature: None,
        apps: vec![ManifestApp {
            app_id: app_id.to_string(),
            bundle_version: bundle_version.to_string(),
            files,
        }],
    };

    let unsigned = manifest.to_json()?;
    manifest.signature = Some(sign_manifest(&unsigned, &seed)?);

    let path = app_dir.join(MANIFEST_FILENAME);
    std::fs::write(&path, manifest.to_json()?).map_err(|e| io_err(&path, e))?;

    info!(
        app = app_id,
        version = bundle_version,
        files = manifest.apps[0].files.len(),
        "wrote signed {MANIFEST_FILENAME}"
    );

    Ok(manifest)
}

/// Generate a signed manifest for every app directory under `public_dir`.
///
/// A subdirectory is an app when it contains an `index.html` — that is what
/// makes it servable. Directories without one are build scratch space and are
/// skipped rather than described.
///
/// # Errors
///
/// Returns [`ManifestError::NoPrivateKey`] if `keypair` cannot sign,
/// [`ManifestError::Io`] if `public_dir` cannot be listed, or propagates any
/// per-app failure from [`generate_app_manifest`].
pub fn generate_all_manifests(
    public_dir: &Path,
    bundle_version: &str,
    manifest_domain: &str,
    keypair: &KeyPair,
) -> ManifestResult<Vec<Manifest>> {
    if !public_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut app_dirs = Vec::new();
    let entries = std::fs::read_dir(public_dir).map_err(|e| io_err(public_dir, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| io_err(public_dir, e))?;
        let path = entry.path();
        if path.is_dir() && path.join("index.html").is_file() {
            app_dirs.push(path);
        }
    }
    app_dirs.sort();

    let mut manifests = Vec::with_capacity(app_dirs.len());
    for dir in app_dirs {
        let app_id = dir
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().to_string());
        manifests.push(generate_app_manifest(
            &dir,
            &app_id,
            bundle_version,
            manifest_domain,
            keypair,
        )?);
    }
    Ok(manifests)
}

/// The CDN domain for this build: [`MANIFEST_DOMAIN_ENV`] or
/// [`DEFAULT_MANIFEST_DOMAIN`].
#[must_use]
pub fn manifest_domain_from_env() -> String {
    std::env::var(MANIFEST_DOMAIN_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_MANIFEST_DOMAIN.to_string())
}
