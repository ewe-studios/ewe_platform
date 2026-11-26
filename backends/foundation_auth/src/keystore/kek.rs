//! Where a key encryption key (KEK) comes from.
//!
//! WHY: a private key is encrypted at rest, so the KEK must not sit next to it.
//! Each platform has a safer place: systemd encrypted credentials, a Docker or
//! Kubernetes secret file, a passphrase, or (last choice) an environment
//! variable. A Worker uses a Secrets Store secret or a Worker secret.
//!
//! WHAT: [`KekSource`], [`KekSourceInfo`] and the sources. The native sources
//! read their value at a host start, which is the one boundary where real file
//! and environment input is correct.
//!
//! HOW: a source reports where it reads from and never the value. `load` makes
//! a [`KeySealer`]. There is no hidden fallback to a weaker source: the
//! configuration names one source, or the host applies the documented order.

#[cfg(not(target_family = "wasm"))]
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use base64::Engine as _;
use foundation_context::env::Env;
#[cfg(not(target_family = "wasm"))]
use zeroize::Zeroizing;

use super::sealing::SealFuture;
#[cfg(not(target_family = "wasm"))]
use super::sealing::{Argon2Params, PassphraseSealer};
#[cfg(not(target_family = "wasm"))]
use super::KeyStoreError;
use super::{KekSealer, KeySealer};

/// What kind of source a [`KekSource`] is, for a log line or `keys info`.
/// It never contains the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KekSourceInfo {
    /// For example "systemd-credential", "secret-file" or "secrets-store".
    pub kind: &'static str,
    /// The credential name, file path or binding name. Never the value.
    pub location: String,
    /// The KEK ID that this source gives.
    pub kek_id: String,
    /// True for a source that keeps the key out of this process (a remote
    /// key management service).
    pub key_leaves_process: bool,
}

/// Where a KEK comes from. The host calls [`KekSource::load`] one time at
/// start.
pub trait KekSource: Send + Sync + 'static {
    /// What kind of source this is. It never contains the key.
    fn describe(&self) -> KekSourceInfo;

    /// Read the KEK and make a sealer from it.
    ///
    /// # Errors
    /// [`KeyStoreError::KekUnavailable`] names the source and says what is
    /// missing (a file, a credential, a binding), never the value.
    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>>;
}

/// A fixed 32-byte KEK for a test or a deterministic simulation run.
#[cfg(feature = "testing")]
#[derive(Debug, Clone)]
pub struct FixedKek {
    /// The KEK ID.
    pub kek_id: String,
    key: [u8; 32],
}

#[cfg(feature = "testing")]
impl FixedKek {
    /// A fixed KEK.
    #[must_use]
    pub fn new(kek_id: impl Into<String>, key: [u8; 32]) -> Self {
        Self {
            kek_id: kek_id.into(),
            key,
        }
    }
}

#[cfg(feature = "testing")]
impl KekSource for FixedKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "fixed",
            location: "in memory".to_string(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            Ok(
                Arc::new(KekSealer::new(self.kek_id.clone(), self.key, env.clone()))
                    as Arc<dyn KeySealer>,
            )
        })
    }
}

/// The 32 raw bytes or the base64url form of a KEK.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when the value is neither 32 raw bytes
/// nor base64url for 32 bytes.
#[cfg(not(target_family = "wasm"))]
fn fixed_key(value: &[u8], source: &str) -> Result<[u8; 32], KeyStoreError> {
    if value.len() == 32 {
        let mut key = [0u8; 32];
        key.copy_from_slice(value);
        return Ok(key);
    }
    let text = core::str::from_utf8(value)
        .map_err(|_| KeyStoreError::KekUnavailable(format!("{source}: not UTF-8")))?;
    let text = text.trim();
    for engine in [
        base64::engine::general_purpose::URL_SAFE_NO_PAD,
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::STANDARD,
    ] {
        if let Ok(bytes) = engine.decode(text) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                return Ok(key);
            }
        }
    }
    Err(KeyStoreError::KekUnavailable(format!(
        "{source}: the value is not 32 bytes or base64url for 32 bytes"
    )))
}

/// Read and remove a KEK environment variable.
///
/// Call this in `main` before the valtron pool or any other thread starts:
/// changing the environment is not safe while other threads run.
///
/// # Errors
/// [`KeyStoreError::KekUnavailable`] when the variable is not set.
#[cfg(not(target_family = "wasm"))]
pub fn take_kek_variable(name: &str) -> Result<Zeroizing<String>, KeyStoreError> {
    let value = std::env::var(name).map_err(|_| {
        KeyStoreError::KekUnavailable(format!("environment variable {name} is not set"))
    })?;
    // SAFETY: the host calls this before any other thread starts.
    unsafe { std::env::remove_var(name) };
    Ok(Zeroizing::new(value))
}

/// A KEK in a systemd encrypted credential.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone)]
pub struct SystemdCredentialKek {
    /// The credential name, for example `agent-rpc-kek`.
    pub name: String,
    /// The KEK ID. Default `systemd-credential`.
    pub kek_id: String,
}

#[cfg(not(target_family = "wasm"))]
impl SystemdCredentialKek {
    /// A credential named `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            kek_id: format!("systemd-credential:{name}"),
            name,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl KekSource for SystemdCredentialKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "systemd-credential",
            location: format!("$CREDENTIALS_DIRECTORY/{}", self.name),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            let dir = std::env::var_os("CREDENTIALS_DIRECTORY").ok_or_else(|| {
                KeyStoreError::KekUnavailable(
                    "CREDENTIALS_DIRECTORY is not set; start under systemd with LoadCredentialEncrypted"
                        .to_string(),
                )
            })?;
            let path = PathBuf::from(dir).join(&self.name);
            let value = std::fs::read(&path).map_err(|e| {
                KeyStoreError::KekUnavailable(format!("credential {}: {e}", path.display()))
            })?;
            let key = fixed_key(&value, &format!("credential {}", path.display()))?;
            Ok(
                Arc::new(KekSealer::new(self.kek_id.clone(), key, env.clone()))
                    as Arc<dyn KeySealer>,
            )
        })
    }
}

/// A KEK in a secret file, for Docker, Kubernetes or a bare-metal host.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone)]
pub struct SecretFileKek {
    /// The file path, for example `/run/secrets/agent-rpc-kek`.
    pub path: PathBuf,
    /// The KEK ID. Default `secret-file`.
    pub kek_id: String,
}

#[cfg(not(target_family = "wasm"))]
impl SecretFileKek {
    /// A secret file at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        Self {
            // The KEK ID must be stable across restarts and different for two
            // paths, so that rotation can tell the old KEK from the new one.
            kek_id: format!("secret-file:{}", path.display()),
            path,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl KekSource for SecretFileKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "secret-file",
            location: self.path.display().to_string(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            let value = std::fs::read(&self.path).map_err(|e| {
                KeyStoreError::KekUnavailable(format!("secret file {}: {e}", self.path.display()))
            })?;
            let key = fixed_key(&value, &format!("secret file {}", self.path.display()))?;
            Ok(
                Arc::new(KekSealer::new(self.kek_id.clone(), key, env.clone()))
                    as Arc<dyn KeySealer>,
            )
        })
    }
}

/// A KEK from a passphrase typed at start, with no echo.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone)]
pub struct PassphrasePromptKek {
    /// The KEK ID. Default `passphrase-prompt`.
    pub kek_id: String,
}

#[cfg(not(target_family = "wasm"))]
impl Default for PassphrasePromptKek {
    fn default() -> Self {
        Self {
            kek_id: "passphrase-prompt".to_string(),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl KekSource for PassphrasePromptKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "passphrase-prompt",
            location: "the terminal".to_string(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            use std::io::IsTerminal as _;
            if !std::io::stdin().is_terminal() {
                return Err(KeyStoreError::KekUnavailable(
                    "standard input is not a terminal, so the passphrase cannot be typed"
                        .to_string(),
                ));
            }
            let passphrase = rpassword::prompt_password("agent-rpc key encryption passphrase: ")
                .map_err(|e| {
                    KeyStoreError::KekUnavailable(format!("reading the passphrase: {e}"))
                })?;
            Ok(Arc::new(PassphraseSealer::new(
                self.kek_id.clone(),
                Zeroizing::new(passphrase),
                Argon2Params::default(),
                env.clone(),
            )) as Arc<dyn KeySealer>)
        })
    }
}

/// A KEK in an environment variable. It is the last choice: other processes of
/// the same user, root, `docker inspect` and crash dumps can read it.
#[cfg(not(target_family = "wasm"))]
#[derive(Debug, Clone)]
pub struct EnvironmentVariableKek {
    /// The variable name.
    pub name: String,
    /// The operator allows an environment variable, in the configuration.
    pub allow_environment_variable: bool,
    /// The KEK ID. Default `environment-variable`.
    pub kek_id: String,
}

#[cfg(not(target_family = "wasm"))]
impl EnvironmentVariableKek {
    /// An environment variable `name`, allowed only when
    /// `allow_environment_variable` is true.
    #[must_use]
    pub fn new(name: impl Into<String>, allow_environment_variable: bool) -> Self {
        let name = name.into();
        Self {
            kek_id: format!("environment-variable:{name}"),
            name,
            allow_environment_variable,
        }
    }
}

#[cfg(not(target_family = "wasm"))]
impl KekSource for EnvironmentVariableKek {
    fn describe(&self) -> KekSourceInfo {
        KekSourceInfo {
            kind: "environment-variable",
            location: self.name.clone(),
            kek_id: self.kek_id.clone(),
            key_leaves_process: false,
        }
    }

    fn load<'a>(&'a self, env: &'a Env) -> SealFuture<'a, Arc<dyn KeySealer>> {
        Box::pin(async move {
            if !self.allow_environment_variable {
                return Err(KeyStoreError::KekUnavailable(format!(
                    "environment variable {} needs allow_environment_variable = true",
                    self.name
                )));
            }
            let value = take_kek_variable(&self.name)?;
            // A base64url value of 32 bytes is a raw KEK; any other value is a
            // passphrase.
            match fixed_key(
                value.as_bytes(),
                &format!("environment variable {}", self.name),
            ) {
                Ok(key) => Ok(
                    Arc::new(KekSealer::new(self.kek_id.clone(), key, env.clone()))
                        as Arc<dyn KeySealer>,
                ),
                Err(_) => Ok(Arc::new(PassphraseSealer::new(
                    self.kek_id.clone(),
                    Zeroizing::new(value.to_string()),
                    Argon2Params::default(),
                    env.clone(),
                )) as Arc<dyn KeySealer>),
            }
        })
    }
}

/// The prompt source also gives the passphrase of an encrypted key file.
#[cfg(not(target_family = "wasm"))]
impl crate::keys::PassphraseSource for PassphrasePromptKek {
    fn passphrase(&self) -> Result<Zeroizing<String>, crate::keys::KeyLoadError> {
        use crate::keys::KeyLoadError;
        use std::io::IsTerminal as _;
        if !std::io::stdin().is_terminal() {
            return Err(KeyLoadError::PassphraseUnavailable(
                "standard input is not a terminal".to_string(),
            ));
        }
        rpassword::prompt_password("agent-rpc key passphrase: ")
            .map(Zeroizing::new)
            .map_err(|e| {
                KeyLoadError::PassphraseUnavailable(format!("reading the passphrase: {e}"))
            })
    }
}

/// The environment variable source also gives the passphrase of an encrypted
/// key file. It does not remove the variable: the host reads it one time.
#[cfg(not(target_family = "wasm"))]
impl crate::keys::PassphraseSource for EnvironmentVariableKek {
    fn passphrase(&self) -> Result<Zeroizing<String>, crate::keys::KeyLoadError> {
        use crate::keys::KeyLoadError;
        if !self.allow_environment_variable {
            return Err(KeyLoadError::PassphraseUnavailable(format!(
                "environment variable {} needs allow_environment_variable = true",
                self.name
            )));
        }
        std::env::var(&self.name).map(Zeroizing::new).map_err(|_| {
            KeyLoadError::PassphraseUnavailable(format!(
                "environment variable {} is not set",
                self.name
            ))
        })
    }
}
