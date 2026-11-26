//! The real host file system.
//!
//! [`NativeFs`] is the [`HostFs`] that reads and writes the real files of the
//! host. Only a boundary installs it: the artifact store (feature 03) uses it
//! first, and feature 05 adds the process spawner and the connector to this
//! module.
//!
//! The module is native only. A Worker has no host file system.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::HostFs;

/// The host file system behind [`HostFs`].
///
/// The value holds no state, so a clone is free.
#[derive(Clone, Copy, Debug, Default)]
pub struct NativeFs;

impl NativeFs {
    /// The real host file system.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// A host input or output error with its operation and path.
fn io_error(op: &str, path: &Path, error: &std::io::Error) -> SandboxError {
    SandboxError::Io {
        op: op.to_string(),
        message: format!("{}: {error}", path.display()),
    }
}

impl HostFs for NativeFs {
    fn create_dir_all(&self, path: &Path, mode: u32) -> SandboxResult<()> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(mode & 0o7777);
        }
        #[cfg(not(unix))]
        let _ = mode;
        builder
            .create(path)
            .map_err(|error| io_error("create_dir_all", path, &error).into())
    }

    fn remove_dir_all(&self, path: &Path) -> SandboxResult<()> {
        match std::fs::remove_dir_all(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(io_error("remove_dir_all", path, &error).into()),
        }
    }

    fn write(&self, path: &Path, data: &[u8]) -> SandboxResult<()> {
        std::fs::write(path, data).map_err(|error| io_error("write", path, &error).into())
    }

    fn read(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let file = File::open(path).map_err(|error| io_error("read", path, &error))?;
        let mut bytes = Vec::new();
        file.take(max as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("read", path, &error))?;
        Ok(bytes)
    }

    fn tail(&self, path: &Path, max: usize) -> SandboxResult<Vec<u8>> {
        let mut file = File::open(path).map_err(|error| io_error("tail", path, &error))?;
        let length = file
            .seek(SeekFrom::End(0))
            .map_err(|error| io_error("tail", path, &error))?;
        let start = length.saturating_sub(max as u64);
        file.seek(SeekFrom::Start(start))
            .map_err(|error| io_error("tail", path, &error))?;
        let mut bytes = Vec::new();
        file.take(max as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error("tail", path, &error))?;
        Ok(bytes)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn rename(&self, from: &Path, to: &Path) -> SandboxResult<()> {
        std::fs::rename(from, to).map_err(|error| {
            SandboxError::Io {
                op: "rename".to_string(),
                message: format!("{} to {}: {error}", from.display(), to.display()),
            }
            .into()
        })
    }

    fn create_file(&self, path: &Path) -> SandboxResult<Box<dyn Write + Send>> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
            .map_err(|error| io_error("create_file", path, &error))?;
        Ok(Box::new(file))
    }
}
