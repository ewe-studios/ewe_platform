//! Install record, lock file and their JSON format.
//!
//! The record is the installer's private file at
//! `<state_dir>/installed.json`. It holds, for each entry, the version, the
//! artifact, the deployed value and the time. Only the installer reads and
//! writes it; the registrar checks the real host instead.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::config::InstallerConfig;
use crate::context::InstallContext;
use crate::deploy::Deployed;
use crate::error::InstallError;
use crate::manifest::EntryName;
use crate::source::Artifact;

/// One entry's record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedEntry {
    /// The version that the installer applied.
    pub version: String,
    /// What the source gave.
    pub artifact: Artifact,
    /// What the deploy target made.
    pub deployed: Deployed,
    /// When the entry was applied, as Unix seconds.
    pub installed_at: u64,
}

/// The installer's record of what it installed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct InstallRecord {
    /// The entries, keyed by entry name.
    pub entries: BTreeMap<String, RecordedEntry>,
}

impl InstallRecord {
    /// Load the record. A missing file is an empty record.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Record`] when the file exists but cannot be read
    /// or parsed.
    pub fn load(path: &Path) -> Result<Self, InstallError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => {
                return Err(InstallError::Record(format!(
                    "cannot read {}: {err}",
                    path.display()
                )));
            }
        };
        serde_json::from_str(&text)
            .map_err(|err| InstallError::Record(format!("cannot parse {}: {err}", path.display())))
    }

    /// Write the record, replacing the old file in one rename.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Record`] when the directory or the file cannot
    /// be written.
    pub fn save(&self, path: &Path) -> Result<(), InstallError> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent).map_err(|err| {
            InstallError::Record(format!("cannot make {}: {err}", parent.display()))
        })?;
        let text = serde_json::to_vec_pretty(self)
            .map_err(|err| InstallError::Record(format!("cannot encode the record: {err}")))?;
        let temporary = temporary_path(path);
        {
            let mut file = std::fs::File::create(&temporary).map_err(|err| {
                InstallError::Record(format!("cannot write {}: {err}", temporary.display()))
            })?;
            file.write_all(&text).map_err(|err| {
                InstallError::Record(format!("cannot write {}: {err}", temporary.display()))
            })?;
            file.flush().map_err(|err| {
                InstallError::Record(format!("cannot flush {}: {err}", temporary.display()))
            })?;
        }
        if let Err(first) = std::fs::rename(&temporary, path) {
            // Windows cannot replace an existing file with a rename.
            if path.exists() {
                std::fs::remove_file(path).map_err(|err| {
                    InstallError::Record(format!("cannot replace {}: {err}", path.display()))
                })?;
                std::fs::rename(&temporary, path).map_err(|err| {
                    InstallError::Record(format!(
                        "cannot rename {} to {}: {err}",
                        temporary.display(),
                        path.display()
                    ))
                })
            } else {
                Err(InstallError::Record(format!(
                    "cannot rename {} to {}: {first}",
                    temporary.display(),
                    path.display()
                )))
            }
        } else {
            Ok(())
        }
    }

    /// The record of `name`, if the installer applied it before.
    #[must_use]
    pub fn get(&self, name: &EntryName) -> Option<&RecordedEntry> {
        self.entries.get(name.as_str())
    }

    /// Put one entry in the record.
    pub fn insert(&mut self, name: &EntryName, entry: RecordedEntry) {
        self.entries.insert(name.as_str().to_string(), entry);
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

/// The exclusive installer lock.
///
/// It holds an operating-system lock on `<state_dir>/installer.lock`. A second
/// run fails with [`InstallError::Locked`] after `lock_timeout`.
#[derive(Debug)]
pub struct InstallerLock {
    file: std::fs::File,
}

/// Whether a failed `try_lock_exclusive` means another holder has the lock.
///
/// Unix reports `WouldBlock`. Windows reports `ERROR_LOCK_VIOLATION` (33),
/// which does not map to `WouldBlock`.
fn lock_is_held(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::WouldBlock
        || (cfg!(windows) && err.raw_os_error() == Some(33))
}

impl InstallerLock {
    /// Take the lock, retrying until `lock_timeout` elapses.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Locked`] when another run holds the lock, and
    /// [`InstallError::Record`] when the lock file cannot be opened.
    pub async fn acquire(
        config: &InstallerConfig,
        ctx: &InstallContext,
    ) -> Result<Self, InstallError> {
        let path = config.lock_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                InstallError::Record(format!("cannot make {}: {err}", parent.display()))
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|err| {
                InstallError::Record(format!("cannot open {}: {err}", path.display()))
            })?;

        let deadline = ctx.context().clock().now() + config.lock_timeout;
        loop {
            match FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Ok(Self { file }),
                Err(err) if lock_is_held(&err) => {
                    if ctx.context().clock().now() >= deadline {
                        return Err(InstallError::Locked(path));
                    }
                    ctx.sleep(config.lock_poll_interval).await;
                }
                Err(err) => {
                    return Err(InstallError::Record(format!(
                        "cannot lock {}: {err}",
                        path.display()
                    )));
                }
            }
        }
    }
}

impl Drop for InstallerLock {
    fn drop(&mut self) {
        if let Err(err) = FileExt::unlock(&self.file) {
            tracing::warn!(error = %err, "cannot release the installer lock");
        }
    }
}
