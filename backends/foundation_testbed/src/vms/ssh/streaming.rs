//! Live-output command execution via `ssh` CLI subprocess.
//!
//! For long-running commands (e.g. `cargo build`), using libssh2's
//! `read_to_string` would block for minutes. Instead, we spawn the
//! `ssh` CLI directly so output streams to the terminal in real time.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::vms::config::{Result, TestbedError, VmProfile};
use crate::vms::ssh::VmSession;

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

/// Execute a command on the VM with live terminal output using an existing session.
///
/// Spawns `ssh` CLI subprocess. Streams stdout/stderr to terminal in real-time.
/// Uses the session's port and user for the connection.
pub fn exec_streaming_session(session: &VmSession, cmd: &str) -> Result<i32> {
    let key_path = find_ssh_key().ok_or_else(|| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("no SSH key found for streaming exec"),
    })?;

    let mut ssh_cmd = Command::new("ssh");
    ssh_cmd
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=ERROR",
            "-o", "BatchMode=yes",
            "-p", &session.port.to_string(),
            "-i", &key_path,
        ])
        .arg(format!("{}@127.0.0.1", session.user))
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = ssh_cmd.spawn().map_err(|e| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("spawning ssh: {e}"),
    })?;

    // Stream stdout and stderr to terminal in real-time
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    // Print stdout lines
    std::thread::spawn(move || {
        for line in stdout_reader.lines() {
            if let Ok(line) = line {
                println!("{line}");
            }
        }
    });

    // Print stderr lines
    std::thread::spawn(move || {
        for line in stderr_reader.lines() {
            if let Ok(line) = line {
                eprintln!("{line}");
            }
        }
    });

    let status = child.wait().map_err(|e| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("waiting for ssh: {e}"),
    })?;

    Ok(status.code().unwrap_or(1))
}

/// Find an SSH key file for SSH CLI authentication.
/// Tries the same locations as `crate::vms::ssh::find_ssh_key`.
fn find_ssh_key() -> Option<String> {
    // 1. Vagrant insecure key (for Vagrant-sourced VM images) - try first
    if let Some(config_dir) = dirs::config_dir() {
        let vagrant_key = config_dir.join("foundation_testbed/vagrant_insecure_key");
        if vagrant_key.exists() {
            return Some(vagrant_key.to_string_lossy().to_string());
        }
    }

    // 2. User's key files
    let key_names = ["id_ed25519", "id_rsa", "id_ecdsa"];
    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");
        for key_name in &key_names {
            let key_path = ssh_dir.join(key_name);
            if key_path.exists() {
                return Some(key_path.to_string_lossy().to_string());
            }
        }
    }

    None
}

