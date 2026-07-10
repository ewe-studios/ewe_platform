//! SSH backend abstraction — swappable transport (ssh2, russh, local).

use crate::command::{Command, CommandResult};
use crate::host::Host;
use std::path::Path;

/// SSH transport backend. Implementations: `Ssh2Backend`, future `RusshBackend`.
pub trait Backend: Send + Sync {
    /// Execute a command on a remote host.
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String>;

    /// Upload a file to a remote host.
    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String>;

    /// Download a file from a remote host.
    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String>;
}

/// ssh2 (libssh2) backend — primary SSH transport.
pub struct Ssh2Backend;

impl Backend for Ssh2Backend {
    fn execute(&self, _host: &Host, cmd: &Command) -> Result<CommandResult, String> {
        // TODO: ssh2 session, exec, capture output
        Err(format!("ssh2 execute not yet implemented: {}", cmd.program))
    }

    fn upload(&self, _host: &Host, _local: &Path, _remote: &Path) -> Result<(), String> {
        Err("ssh2 upload not yet implemented".to_string())
    }

    fn download(&self, _host: &Host, _remote: &Path, _local: &Path) -> Result<(), String> {
        Err("ssh2 download not yet implemented".to_string())
    }
}
