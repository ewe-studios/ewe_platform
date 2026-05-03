//! SSH client layer for communicating with guest VMs.
//!
//! Connects via the forwarded port (e.g. 2222 → guest :22).
//! Authentication falls back through: SSH agent → key files → password.
//! Commands are executed via `nu -c "..."` after bootstrap.
//!
//! # SIGHUP / pty safety
//!
//! Non-interactive `exec` uses libssh2 channels (no pty allocated) — safe for
//! backgrounded processes (`setsid -f`, `nohup &`) because closing the channel
//! does **not** send SIGHUP.
//!
//! Interactive `shell` shells out to the `ssh` CLI. It allocates a pty (`-tt`)
//! **only** for Linux guests. Windows guests (`cmd.exe`/PowerShell) break with
//! forced pty allocation, so `-tt` is omitted there.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

use ssh2::Session;

use crate::config::{GuestOs, Result, TestbedError, VmProfile};

pub mod streaming;

/// Established SSH session connected to a guest VM.
pub struct VmSession {
    /// The ssh2 session (connected, authenticated).
    pub session: Session,
    /// The port on localhost that forwards to guest SSH.
    pub port: u16,
    /// The guest OS type (affects shell selection).
    pub os: GuestOs,
}

/// Connect to a VM's SSH server using auth fallback chain.
///
/// Authentication priority: SSH agent → key files → password.
pub fn connect(profile: &VmProfile) -> Result<VmSession> {
    let port = profile.ssh_port;
    let tcp = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| TestbedError::SshFailed {
            port,
            source: anyhow::anyhow!(e),
        })?;

    let mut session = Session::new().map_err(|e| TestbedError::SshFailed {
        port,
        source: anyhow::anyhow!(e),
    })?;

    session.set_tcp_stream(tcp);
    session.handshake().map_err(|e| TestbedError::SshFailed {
        port,
        source: anyhow::anyhow!(e),
    })?;

    // Auth fallback chain: agent → keys → password
    authenticate(&mut session, profile).map_err(|e| TestbedError::SshFailed {
        port,
        source: e,
    })?;

    Ok(VmSession {
        session,
        port,
        os: profile.os,
    })
}

/// Run authentication methods in order.
fn authenticate(session: &mut Session, profile: &VmProfile) -> anyhow::Result<()> {
    // 1. SSH agent
    if session.userauth_agent(profile.user).is_ok() {
        return Ok(());
    }

    // 2. Key files
    let key_names = ["id_ed25519", "id_rsa", "id_ecdsa"];
    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");
        for key_name in &key_names {
            let key_path = ssh_dir.join(key_name);
            if key_path.exists() {
                if session
                    .userauth_pubkey_file(profile.user, None, &key_path, None)
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
    }

    // 3. Password
    session
        .userauth_password(profile.user, profile.pass)
        .map_err(|e| anyhow::anyhow!("all auth methods failed: {e}"))
}

/// Execute a command on the guest and return stdout.
///
/// Commands are wrapped in `nu -c "..."` for nushell consistency.
/// Before bootstrap, falls back to `bash -c` (Linux) or `cmd /c` (Windows).
pub fn exec(session: &mut VmSession, cmd: &str) -> Result<String> {
    let (output, _code) = exec_with_exit(session, cmd)?;
    Ok(output)
}

/// Execute a command and return (stdout, exit_code).
///
/// Uses nushell (`nu -c`) for cross-platform consistency.
pub fn exec_with_exit(session: &mut VmSession, cmd: &str) -> Result<(String, i32)> {
    let wrapped = wrap_command(cmd);
    let mut channel = session
        .session
        .channel_session()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!(e),
        })?;

    channel
        .exec(&wrapped)
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!(e),
        })?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!(e),
        })?;

    let exit_code = channel
        .exit_status()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!(e),
        })?;

    Ok((output, exit_code))
}

/// Upload a file to the guest via SCP.
pub fn upload(session: &mut VmSession, local: &Path, remote: &str) -> Result<()> {
    let mut file = File::open(local).map_err(|e| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("opening {local:?}: {e}"),
    })?;

    let metadata = file
        .metadata()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("stat {local:?}: {e}"),
        })?;

    let mut channel = session
        .session
        .scp_send(
            std::path::Path::new(remote),
            0o644,
            metadata.len(),
            None,
        )
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp_send to {remote}: {e}"),
        })?;

    // Limit writes to channel's window size (libssh2 requirement)
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("reading {local:?}: {e}"),
        })?;
        if n == 0 {
            break;
        }
        channel.write_all(&buf[..n]).map_err(|e| {
            TestbedError::SshFailed {
                port: session.port,
                source: anyhow::anyhow!("scp write: {e}"),
            }
        })?;
    }

    channel
        .send_eof()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp send_eof: {e}"),
        })?;

    channel
        .wait_eof()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp wait_eof: {e}"),
        })?;

    channel
        .close()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp close: {e}"),
        })?;

    channel
        .wait_close()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp wait_close: {e}"),
        })?;

    Ok(())
}

/// Download a file from the guest via SCP.
pub fn download(session: &mut VmSession, remote: &str, local: &Path) -> Result<()> {
    let (mut channel, _stat) = session
        .session
        .scp_recv(std::path::Path::new(remote))
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp_recv {remote}: {e}"),
        })?;

    let mut file = File::create(local).map_err(|e| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("creating {local:?}: {e}"),
    })?;

    // Read with window-size awareness
    let mut buf = [0u8; 65536];
    loop {
        let n = channel.read(&mut buf).map_err(|e| {
            TestbedError::SshFailed {
                port: session.port,
                source: anyhow::anyhow!("scp read: {e}"),
            }
        })?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| {
            TestbedError::SshFailed {
                port: session.port,
                source: anyhow::anyhow!("writing {local:?}: {e}"),
            }
        })?;
    }

    Ok(())
}

/// Health probe: connect + echo test.
pub fn check(profile: &VmProfile) -> Result<()> {
    let mut session = connect(profile)?;
    let output = exec(&mut session, "echo testbed-ping")?;
    if output.trim() != "testbed-ping" {
        return Err(TestbedError::SshFailed {
            port: profile.ssh_port,
            source: anyhow::anyhow!("echo test returned unexpected output: {output:?}"),
        });
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Wrap a command in the appropriate shell invocation.
///
/// After bootstrap: `nu -c "..."` (nushell).
/// Before bootstrap: `bash -c` (Linux) or `cmd /c` (Windows).
fn wrap_command(cmd: &str) -> String {
    // Try nushell first (post-bootstrap)
    format!("nu -c {cmd:?}")
}

/// For Windows guests that haven't been bootstrapped yet, use cmd.
pub fn wrap_command_cmd(cmd: &str) -> String {
    format!("cmd /c {cmd:?}")
}

/// For Linux guests that haven't been bootstrapped yet, use bash.
pub fn wrap_command_bash(cmd: &str) -> String {
    format!("bash -c {cmd:?}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_command_nushell() {
        let wrapped = wrap_command("echo hello");
        assert!(wrapped.starts_with("nu -c"));
        assert!(wrapped.contains("echo hello"));
    }

    #[test]
    fn test_wrap_command_cmd() {
        let wrapped = wrap_command_cmd("echo hello");
        assert!(wrapped.starts_with("cmd /c"));
    }

    #[test]
    fn test_wrap_command_bash() {
        let wrapped = wrap_command_bash("echo hello");
        assert!(wrapped.starts_with("bash -c"));
    }
}
