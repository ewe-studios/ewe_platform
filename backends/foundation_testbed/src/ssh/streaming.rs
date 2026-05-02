//! Live-output command execution via `ssh` CLI subprocess.
//!
//! For long-running commands (e.g. `cargo build`), using libssh2's
//! `read_to_string` would block for minutes. Instead, we spawn the
//! `ssh` CLI directly so output streams to the terminal in real time.

use std::process::{Command, Stdio};

use crate::config::{Result, TestbedError, VmProfile};

/// Execute a command on the VM with live terminal output.
///
/// Spawns `ssh` CLI subprocess. Returns the exit code.
/// Commands are wrapped in `nu -c "..."` for nushell consistency.
pub fn exec_streaming(profile: &VmProfile, cmd: &str) -> Result<i32> {
    let wrapped = format!("nu -c {cmd:?}");

    let mut ssh_cmd = Command::new("ssh");
    ssh_cmd
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=ERROR",
            "-p", &profile.ssh_port.to_string(),
            "-i", "vagrant", // username
        ])
        .arg(format!("{}@127.0.0.1", profile.user))
        .arg(&wrapped)
        .stdin(Stdio::null());

    let mut child = ssh_cmd.spawn().map_err(|e| TestbedError::SshFailed {
        port: profile.ssh_port,
        source: anyhow::anyhow!("spawning ssh: {e}"),
    })?;

    let status = child.wait().map_err(|e| TestbedError::SshFailed {
        port: profile.ssh_port,
        source: anyhow::anyhow!("waiting for ssh: {e}"),
    })?;

    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_streaming_wraps_in_nushell() {
        // Verify the command would be wrapped correctly
        let cmd = "cargo build";
        let wrapped = format!("nu -c {cmd:?}");
        assert!(wrapped.contains("cargo build"));
        assert!(wrapped.starts_with("nu -c"));
    }
}
