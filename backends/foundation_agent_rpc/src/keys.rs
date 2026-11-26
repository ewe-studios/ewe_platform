//! The `agent-rpc keys` commands and the native key encryption key (KEK)
//! selection.
//!
//! WHY: the operator must seal the clear-text key files of the first version
//! (`agent-rpc keys migrate`) and move every record to a new KEK
//! (`agent-rpc keys rotate`). The host must find its KEK in the documented
//! order and must never fall back to a weaker source on its own.
//!
//! WHAT: [`KekConfig`], [`select_native_kek`], [`KeysCommand`],
//! [`parse_keys_command`], [`migrate`], [`rotate`] and [`KeysReport`].
//!
//! HOW: the commands build a file key store over the state directory and run
//! the key-store migration or rotation. The KEK source reads its value one
//! time at this boundary.

use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use foundation_auth::keystore::{
    migrate_legacy_keys, FsKeyStore, KekSource, KekSourceInfo, KeyId, KeyMaterial, KeyProtection,
    KeySealer, KeyStore, KeyStoreError, MigrationReport, PassphrasePromptKek, SecretFileKek,
    SystemdCredentialKek,
};
use foundation_context::Ctx;
use foundation_core::env::Env;

/// The default file of the KEK for Docker, Kubernetes and a bare-metal host.
pub const DEFAULT_SECRET_FILE: &str = "/run/secrets/agent-rpc-kek";
/// The default name of the systemd credential.
pub const DEFAULT_CREDENTIAL_NAME: &str = "agent-rpc-kek";

/// The message of a host that finds no KEK. It lists every option and the
/// design link, and it never names a value.
pub const NO_KEK_MESSAGE: &str = "no key encryption key (KEK) is available. Configure one of: \
    (1) a systemd encrypted credential named agent-rpc-kek, loaded with LoadCredentialEncrypted=; \
    (2) a secret file at /run/secrets/agent-rpc-kek (Docker or Kubernetes secrets); \
    (3) a passphrase at a terminal; or (4) an explicit EnvironmentVariableKek with \
    allow_environment_variable = true. See \
    specifications/83-agent-rpc-serving-system/features/08-key-protection-at-rest/key-protection.md";

/// One KEK source, as the native host configuration names it.
#[derive(Debug, Clone)]
pub enum KekConfig {
    /// A systemd encrypted credential. The default name is
    /// [`DEFAULT_CREDENTIAL_NAME`].
    SystemdCredential {
        /// The credential name.
        name: String,
    },
    /// A secret file. The default path is [`DEFAULT_SECRET_FILE`].
    SecretFile {
        /// The file path.
        path: PathBuf,
    },
    /// Ask for a passphrase on the terminal at start.
    PassphrasePrompt,
    /// Read an environment variable. Only when the configuration allows it.
    EnvironmentVariable {
        /// The variable name.
        name: String,
        /// The operator allows an environment variable.
        allow_environment_variable: bool,
    },
    /// Envelope encryption with a remote key management service.
    RemoteKms(foundation_auth::keystore::KmsConfig),
}

impl KekConfig {
    /// A systemd credential named [`DEFAULT_CREDENTIAL_NAME`].
    #[must_use]
    pub fn default_systemd() -> Self {
        Self::SystemdCredential {
            name: DEFAULT_CREDENTIAL_NAME.to_string(),
        }
    }

    /// A secret file at [`DEFAULT_SECRET_FILE`].
    #[must_use]
    pub fn default_secret_file() -> Self {
        Self::SecretFile {
            path: PathBuf::from(DEFAULT_SECRET_FILE),
        }
    }
}

/// The KEK source that a host with no configuration tries, in the documented
/// order.
///
/// 1. a systemd credential named `agent-rpc-kek`, when `$CREDENTIALS_DIRECTORY`
///    holds it;
/// 2. `/run/secrets/agent-rpc-kek`, when it exists;
/// 3. a passphrase prompt, when standard input is a terminal.
///
/// An environment variable is never chosen by itself.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] with [`NO_KEK_MESSAGE`] when no option
/// works.
pub fn default_kek_source() -> Result<Box<dyn KekSource>, KeyStoreError> {
    if let Some(dir) = std::env::var_os("CREDENTIALS_DIRECTORY") {
        let path = PathBuf::from(dir).join(DEFAULT_CREDENTIAL_NAME);
        if path.exists() {
            return Ok(Box::new(SystemdCredentialKek::new(DEFAULT_CREDENTIAL_NAME)));
        }
    }
    if Path::new(DEFAULT_SECRET_FILE).exists() {
        return Ok(Box::new(SecretFileKek::new(DEFAULT_SECRET_FILE)));
    }
    if std::io::stdin().is_terminal() {
        return Ok(Box::new(PassphrasePromptKek::default()));
    }
    Err(KeyStoreError::KekUnavailable(NO_KEK_MESSAGE.to_string()))
}

/// Build the source that `config` names, or the default source of
/// [`default_kek_source`].
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when no source is available.
pub fn kek_source(
    config: Option<&KekConfig>,
    clock: std::sync::Arc<dyn foundation_context::Clock>,
) -> Result<Box<dyn KekSource>, KeyStoreError> {
    match config {
        Some(KekConfig::SystemdCredential { name }) => {
            Ok(Box::new(SystemdCredentialKek::new(name.clone())))
        }
        Some(KekConfig::SecretFile { path }) => Ok(Box::new(SecretFileKek::new(path.clone()))),
        Some(KekConfig::PassphrasePrompt) => Ok(Box::new(PassphrasePromptKek::default())),
        Some(KekConfig::EnvironmentVariable {
            name,
            allow_environment_variable,
        }) => Ok(Box::new(
            foundation_auth::keystore::EnvironmentVariableKek::new(
                name.clone(),
                *allow_environment_variable,
            ),
        )),
        Some(KekConfig::RemoteKms(config)) => Ok(Box::new(
            foundation_auth::keystore::RemoteKmsKek::new(config.clone(), "remote-kms", clock)?,
        )),
        None => default_kek_source(),
    }
}

/// Load the sealer that `config` names.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when the source cannot read the KEK.
pub async fn select_native_kek(
    config: Option<&KekConfig>,
    env: &Env,
    clock: std::sync::Arc<dyn foundation_context::Clock>,
) -> Result<Arc<dyn KeySealer>, KeyStoreError> {
    kek_source(config, clock)?.load(env).await
}

/// The `agent-rpc keys` subcommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeysCommand {
    /// Seal the clear-text key files of the first version, then remove them.
    Migrate,
    /// Reseal every record with the current key encryption key (KEK).
    RotateKek,
    /// Make a new secret-signing key and keep the old public key.
    RotateKey,
}

/// Parse the arguments of `agent-rpc keys ...`.
///
/// A leading `keys` word is optional, so `["keys", "migrate"]` and
/// `["migrate"]` are the same command.
///
/// # Errors
/// A message that names the accepted subcommands.
pub fn parse_keys_command(args: &[String]) -> Result<KeysCommand, String> {
    let args: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|arg| *arg != "keys")
        .collect();
    match args.as_slice() {
        ["migrate"] => Ok(KeysCommand::Migrate),
        ["rotate-kek"] => Ok(KeysCommand::RotateKek),
        ["rotate"] => Ok(KeysCommand::RotateKey),
        _ => Err("usage: agent-rpc keys <migrate|rotate|rotate-kek>".to_string()),
    }
}

/// What a `keys` command changed.
#[derive(Debug, Clone, Default)]
pub struct KeysReport {
    /// The records that the migration sealed.
    pub migrated: Vec<KeyId>,
    /// The clear-text files that the migration removed.
    pub removed: Vec<PathBuf>,
    /// The records that a rotation resealed or replaced.
    pub rotated: usize,
    /// The key ID (`kid`) of a new secret-signing key.
    pub new_kid: Option<String>,
}

impl KeysReport {
    fn from_migration(report: MigrationReport) -> Self {
        Self {
            migrated: report.migrated,
            removed: report.removed,
            rotated: 0,
            new_kid: None,
        }
    }
}

impl core::fmt::Display for KeysReport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if !self.migrated.is_empty() {
            writeln!(
                f,
                "migrated {} key record(s); removed {} clear-text file(s)",
                self.migrated.len(),
                self.removed.len()
            )?;
            for path in &self.removed {
                writeln!(f, "  removed {}", path.display())?;
            }
        }
        if self.rotated > 0 {
            writeln!(f, "resealed or replaced {} key record(s)", self.rotated)?;
        }
        if let Some(kid) = &self.new_kid {
            writeln!(f, "the new secret-signing key has kid {kid}")?;
        }
        if self.migrated.is_empty() && self.rotated == 0 {
            f.write_str("nothing to do")?;
        }
        Ok(())
    }
}

/// The host context needs an `Env` for the KEK source and the sealers.
fn require_env(ctx: &Ctx) -> Result<Arc<Env>, KeyStoreError> {
    ctx.require_env()
        .map_err(|e| KeyStoreError::KekUnavailable(e.to_string()))
}

fn material(state_dir: &Path, sealers: KeyProtection, ctx: &Ctx) -> (Arc<FsKeyStore>, KeyMaterial) {
    let store = Arc::new(FsKeyStore::new(state_dir));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn foundation_auth::keystore::KeyStore>,
        sealers,
        ctx.clone(),
    );
    (store, material)
}

/// Seal every clear-text key file in `state_dir` and remove it.
///
/// # Errors
/// [`KeyStoreError`] variants; a file whose sealed record does not open stays.
pub async fn migrate(
    state_dir: &Path,
    config: Option<&KekConfig>,
    ctx: &Ctx,
) -> Result<KeysReport, KeyStoreError> {
    let env = require_env(ctx)?;
    let sealer = select_native_kek(config, &env, ctx.clock()).await?;
    let (store, material) = material(state_dir, KeyProtection::new(sealer), ctx);
    let report = migrate_legacy_keys(&store, &material).await?;
    Ok(KeysReport::from_migration(report))
}

/// Move every sealed record to the current KEK, opening old blobs with the
/// previous KEKs.
///
/// # Errors
/// [`KeyStoreError`] variants.
pub async fn rotate_kek(
    state_dir: &Path,
    current: Option<&KekConfig>,
    previous: &[KekConfig],
    ctx: &Ctx,
) -> Result<KeysReport, KeyStoreError> {
    let env = require_env(ctx)?;
    let current = select_native_kek(current, &env, ctx.clock()).await?;
    let mut old = Vec::with_capacity(previous.len());
    for config in previous {
        old.push(select_native_kek(Some(config), &env, ctx.clock()).await?);
    }
    let (_store, material) = material(state_dir, KeyProtection::with_previous(current, old), ctx);
    let rotated = material.rotate_kek().await?;
    Ok(KeysReport {
        migrated: Vec::new(),
        removed: Vec::new(),
        rotated,
        new_kid: None,
    })
}

/// Make a new secret-signing key with a new key ID (`kid`), and keep the old
/// public key under `secret-signing-<kid>`.
///
/// The verifier reads every record of the signing kind, so an old secret still
/// verifies while its record is present. New secrets use the new current key.
/// The operator removes the old record after the last old secret expires.
///
/// # Errors
/// [`KeyStoreError::NotFound`] when the state directory has no signing key, and
/// the other [`KeyStoreError`] variants.
pub async fn rotate_signing_key(
    state_dir: &Path,
    config: Option<&KekConfig>,
    ctx: &Ctx,
) -> Result<KeysReport, KeyStoreError> {
    use foundation_auth::keystore::{
        record_aad, CreateOutcome, KeyKind, KeyRecord, StoredPrivateKey,
    };

    let env = require_env(ctx)?;
    let sealer = select_native_kek(config, &env, ctx.clock()).await?;
    let (store, material) = material(state_dir, KeyProtection::new(sealer), ctx);
    let id = KeyId::new(KeyId::SECRET_SIGNING)?;
    let current = store
        .get(&id)
        .await?
        .ok_or_else(|| KeyStoreError::NotFound(id.to_string()))?;

    // Archive the old key under a `kid` suffix, so the verifier keeps it.
    let old_private = material.open_private(&current).await?;
    let archive_id = id.with_suffix(&current.kid())?;
    if store.get(&archive_id).await?.is_none() {
        let aad = record_aad(&archive_id, KeyKind::SecretSigning, 1);
        let archive = KeyRecord {
            id: archive_id,
            revision: 1,
            private: StoredPrivateKey::Sealed(
                material.seal_bytes(&aad, old_private.as_slice()).await?,
            ),
            ..current.clone()
        };
        // Another process may have archived the same key first.
        if let CreateOutcome::Created = store.create(&archive).await? {
            tracing::info!(key = archive.id.as_str(), "archived the old signing key");
        }
    }

    // Replace the current record with a new key at the next revision.
    let new_record = material
        .fresh_signing_record(&id, current.revision + 1)
        .await?;
    let new_kid = new_record.kid();
    store.replace(&new_record, current.revision).await?;
    Ok(KeysReport {
        migrated: Vec::new(),
        removed: Vec::new(),
        rotated: 1,
        new_kid: Some(new_kid),
    })
}

/// The `keys info` view: where the KEK comes from, never its value.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when no source is available.
pub fn describe(
    config: Option<&KekConfig>,
    clock: std::sync::Arc<dyn foundation_context::Clock>,
) -> Result<KekSourceInfo, KeyStoreError> {
    Ok(kek_source(config, clock)?.describe())
}
