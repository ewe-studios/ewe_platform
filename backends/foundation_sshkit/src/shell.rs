//! Interactive shell and streaming command execution over SSH.
//!
//! Spawns the `ssh` CLI for features that libssh2 doesn't provide:
//! pty allocation, real-time terminal output, and interactive sessions.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::Host;

/// Open an interactive SSH session to `host`.
///
/// Allocates a pty (`-tt`). Inherits stdin/stdout/stderr from the
/// calling process.
///
/// # Errors
/// Returns an error if `ssh` fails to spawn or exits non-zero.
pub fn interactive(host: &Host) -> Result<(), String> {
    let status = Command::new("ssh")
        .args([
            "-tt",
            "-p", &host.port.to_string(),
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=quiet",
            &format!("{}@{}", host.user, host.hostname),
        ])
        .status()
        .map_err(|e| format!("spawning ssh: {e}"))?;

    if !status.success() {
        return Err(format!("ssh exited with status {status:?}"));
    }
    Ok(())
}

/// Execute a command on `host` via the `ssh` CLI, streaming stdout and
/// stderr to the terminal in real time.
///
/// Returns the exit code. Unlike libssh2's `exec`, this streams output
/// live — ideal for long-running commands like `cargo build`.
///
/// # Errors
/// Returns an error if `ssh` fails to spawn or the process exits abnormally.
pub fn exec_streaming(host: &Host, cmd: &str) -> Result<i32, String> {
    let key_path = find_ssh_key().ok_or_else(|| "no SSH key found".to_string())?;

    let mut child = Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=ERROR",
            "-o", "BatchMode=yes",
            "-p", &host.port.to_string(),
            "-i", &key_path,
            &format!("{}@{}", host.user, host.hostname),
            cmd,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawning ssh: {e}"))?;

    // Stream stdout and stderr to terminal in real time
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let stdout_reader = BufReader::new(stdout);
    let stderr_reader = BufReader::new(stderr);

    std::thread::spawn(move || {
        for line in stdout_reader.lines() {
            if let Ok(line) = line {
                println!("{line}");
            }
        }
    });
    std::thread::spawn(move || {
        for line in stderr_reader.lines() {
            if let Ok(line) = line {
                eprintln!("{line}");
            }
        }
    });

    let status = child.wait().map_err(|e| format!("waiting for ssh: {e}"))?;
    Ok(status.code().unwrap_or(1))
}

/// Find an SSH key file for the `ssh` CLI.
///
/// Tries the Vagrant insecure key first, then the user's `~/.ssh/id_*` keys.
fn find_ssh_key() -> Option<String> {
    if let Some(config_dir) = dirs::config_dir() {
        let vagrant_key = config_dir.join("foundation_testbed/vagrant_insecure_key");
        if vagrant_key.exists() {
            return Some(vagrant_key.to_string_lossy().to_string());
        }
    }
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
