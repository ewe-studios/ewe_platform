//! SSH backend abstraction — swappable transport (ssh2, russh, local).

use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use crate::command::{Command, CommandResult};
use crate::host::Host;
use crate::pool::ConnectionPool;

/// SSH transport backend. Implementations: `Ssh2Backend`, future `RusshBackend`.
pub trait Backend: Send + Sync {
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String>;
    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String>;
    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String>;
}

/// ssh2 (libssh2) backend — primary SSH transport.
/// Uses a `ConnectionPool` for session reuse.
pub struct Ssh2Backend {
    pool: Arc<ConnectionPool>,
}

impl Ssh2Backend {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        Self { pool }
    }
}

impl Backend for Ssh2Backend {
    fn execute(&self, host: &Host, cmd: &Command) -> Result<CommandResult, String> {
        let session = self.pool.get(host)?;
        let start = Instant::now();
        let shell_cmd = cmd.to_shell_command();

        let mut channel = session.channel_session()
            .map_err(|e| format!("channel: {e}"))?;
        channel.exec(&shell_cmd)
            .map_err(|e| format!("exec: {e}"))?;

        let mut stdout = String::new();
        channel.read_to_string(&mut stdout)
            .map_err(|e| format!("read stdout: {e}"))?;

        let mut stderr = String::new();
        channel.stderr().read_to_string(&mut stderr).ok();

        channel.wait_close().ok();
        let exit_code = channel.exit_status().unwrap_or(-1);

        Ok(CommandResult {
            exit_code,
            stdout,
            stderr,
            runtime: start.elapsed(),
            host: format!("{}@{}", host.user, host.hostname),
        })
    }

    fn upload(&self, host: &Host, local: &Path, remote: &Path) -> Result<(), String> {
        let session = self.pool.get(host)?;

        let local_size = std::fs::metadata(local)
            .map_err(|e| format!("stat local: {e}"))?.len();
        let local_data = std::fs::read(local)
            .map_err(|e| format!("read local: {e}"))?;

        let mut remote_file = session.scp_send(
            remote,
            0o644,
            local_size,
            None,
        ).map_err(|e| format!("scp send: {e}"))?;

        remote_file.write(&local_data)
            .map_err(|e| format!("scp write: {e}"))?;
        remote_file.send_eof()
            .map_err(|e| format!("scp eof: {e}"))?;
        remote_file.wait_eof()
            .map_err(|e| format!("scp wait: {e}"))?;

        Ok(())
    }

    fn download(&self, host: &Host, remote: &Path, local: &Path) -> Result<(), String> {
        let session = self.pool.get(host)?;

        let (mut remote_file, _stat) = session.scp_recv(remote)
            .map_err(|e| format!("scp recv: {e}"))?;

        let mut data = Vec::new();
        remote_file.read_to_end(&mut data)
            .map_err(|e| format!("scp read: {e}"))?;

        std::fs::write(local, data)
            .map_err(|e| format!("write local: {e}"))?;

        Ok(())
    }
}
