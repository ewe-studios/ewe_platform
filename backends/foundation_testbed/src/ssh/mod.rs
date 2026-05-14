//! SSH client layer for communicating with guest VMs.
//!
//! Connects via the forwarded port (e.g. 2222 → guest :22).
//! Authentication falls back through: SSH agent → key files → password.
//! Commands are executed via `bash -c` (Linux) or `cmd /c` (Windows).
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

use std::io::Read;
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
    /// The guest username used for authentication.
    pub user: String,
}

/// Connect to a VM's SSH server using auth fallback chain.
///
/// Authentication priority: SSH agent → key files → password.
pub fn connect(profile: &VmProfile) -> Result<VmSession> {
    let port = profile.ssh_port;
    connect_raw(port, profile.user, profile.os)
}

/// Connect to a VM's SSH server given just a port and user.
///
/// Used by the script runner which doesn't have a full VmProfile.
pub fn connect_from_port(port: u16, user: &str, os: GuestOs) -> Result<VmSession> {
    connect_raw(port, user, os)
}

fn connect_raw(port: u16, user: &str, os: GuestOs) -> Result<VmSession> {
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
    authenticate_raw(&mut session, user).map_err(|e| TestbedError::SshFailed {
        port,
        source: e,
    })?;

    Ok(VmSession {
        session,
        port,
        os,
        user: user.to_string(),
    })
}

/// Authenticate without a profile — just user + port.
fn authenticate_raw(session: &mut Session, user: &str) -> anyhow::Result<()> {
    // 1. SSH agent
    if session.userauth_agent(user).is_ok() {
        return Ok(());
    }

    // 2. Key files
    let key_names = ["id_ed25519", "id_rsa", "id_ecdsa"];
    if let Some(home) = dirs::home_dir() {
        let ssh_dir = home.join(".ssh");
        for key_name in &key_names {
            let key_path = ssh_dir.join(key_name);
            if key_path.exists()
                && session
                    .userauth_pubkey_file(user, None, &key_path, None)
                    .is_ok()
                {
                    return Ok(());
                }
        }
    }

    // 2b. Vagrant insecure key (for Vagrant-sourced VM images)
    if let Some(config_dir) = dirs::config_dir() {
        let vagrant_key = config_dir.join("foundation_testbed/vagrant_insecure_key");
        if vagrant_key.exists()
            && session
                .userauth_pubkey_file(user, None, &vagrant_key, None)
                .is_ok()
            {
                return Ok(());
            }
    }

    // 3. Password (not available without profile — skip)
    anyhow::bail!("all auth methods failed for user '{user}'")
}

/// Execute a command on the guest and return stdout.
///
/// Commands are wrapped in `bash -c` (Linux) or `cmd /c` (Windows).
pub fn exec(session: &mut VmSession, cmd: &str) -> Result<String> {
    let (output, _code) = exec_with_exit(session, cmd)?;
    Ok(output)
}

/// Execute a command and return (stdout, exit_code).
///
/// Uses `bash -c` (Linux) or `cmd /c` (Windows) for guest command execution.
pub fn exec_with_exit(session: &mut VmSession, cmd: &str) -> Result<(String, i32)> {
    let wrapped = wrap_command(cmd, session.os);
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

/// Run PowerShell over SSH on Windows guests, stripping CLIXML envelope.
///
/// PowerShell over non-interactive SSH folds info/progress streams onto stderr
/// as a CLIXML envelope — a ~6 KB `<Objs>` XML blob prefixed by `#< CLIXML`.
/// This function encodes the script as UTF-16LE + Base64, runs it via
/// `powershell -NoProfile -EncodedCommand`, and strips the CLIXML noise so
/// callers get clean stdout + exit code.
pub fn exec_ps_windows(session: &mut VmSession, script: &str) -> Result<(String, i32)> {
    // Encode script as UTF-16LE + Base64 for -EncodedCommand
    let utf16le: Vec<u8> = script
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let encoded =
        base64::engine::Engine::encode(&base64::engine::general_purpose::STANDARD, &utf16le);

    let cmd = format!("powershell -NoProfile -EncodedCommand {encoded}");
    let (mut stdout, exit_code) = exec_with_exit(session, &cmd)?;

    // Strip CLIXML envelope: everything from "#< CLIXML" onward is noise.
    // This appears on stdout when PowerShell emits info/progress streams
    // over a non-interactive transport.
    if let Some(idx) = stdout.find("#< CLIXML") {
        stdout.truncate(idx);
    }

    Ok((stdout, exit_code))
}

/// Upload a file to the guest via `scp` CLI with key-based auth.
///
/// Uses the same key files as `authenticate_raw`: ~/.ssh/id_ed25519, ~/.ssh/id_rsa,
/// ~/.ssh/id_ecdsa, or the Vagrant insecure key.
pub fn upload(session: &mut VmSession, local: &Path, remote: &str) -> Result<()> {
    let key_path = find_ssh_key().ok_or_else(|| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("no SSH key found for SCP upload"),
    })?;

    let status = Command::new("scp")
        .args([
            "-P", &session.port.to_string(),
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=quiet",
            "-i", &key_path,
            local.to_str().ok_or_else(|| TestbedError::SshFailed {
                port: session.port,
                source: anyhow::anyhow!("local path {local:?} is not valid UTF-8"),
            })?,
            &format!("{}@127.0.0.1:{remote}", session.user),
        ])
        .status()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("spawning scp: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp upload failed (exit {:?})", status.code()),
        });
    }

    Ok(())
}

/// Find an SSH key file for SCP authentication.
/// Tries the same locations as `authenticate_raw`, with Vagrant key first.
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

/// Download a file from the guest via `scp` CLI with key-based auth.
///
/// Uses the same key files as `authenticate_raw`.
pub fn download(session: &mut VmSession, remote: &str, local: &Path) -> Result<()> {
    let key_path = find_ssh_key().ok_or_else(|| TestbedError::SshFailed {
        port: session.port,
        source: anyhow::anyhow!("no SSH key found for SCP download"),
    })?;

    let status = Command::new("scp")
        .args([
            "-P", &session.port.to_string(),
            "-o", "StrictHostKeyChecking=no",
            "-o", "UserKnownHostsFile=/dev/null",
            "-o", "LogLevel=quiet",
            "-i", &key_path,
            &format!("{}@127.0.0.1:{remote}", session.user),
            local.to_str().ok_or_else(|| TestbedError::SshFailed {
                port: session.port,
                source: anyhow::anyhow!("local path {local:?} is not valid UTF-8"),
            })?,
        ])
        .status()
        .map_err(|e| TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("spawning scp: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::SshFailed {
            port: session.port,
            source: anyhow::anyhow!("scp download failed (exit {:?})", status.code()),
        });
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

/// Open an interactive SSH session to the guest.
///
/// # pty allocation
///
/// - **Linux guests**: `-tt` is used to force pty allocation so the user gets
///   a proper line-buffered terminal.
/// - **Windows guests**: `-tt` is **omitted** because `cmd.exe`/PowerShell
///   break with forced pty allocation.
///
/// # SIGHUP safety
///
/// Unlike non-interactive `exec` (which uses libssh2 channels with no pty),
/// this allocates a pty for Linux guests. When the SSH connection closes,
/// the pty's process group receives SIGHUP — this is expected behaviour for
/// interactive sessions. Do NOT use this for launching background daemons.
pub fn shell(profile: &VmProfile) -> Result<()> {
    let mut args = vec![
        "-p".to_string(),
        profile.ssh_port.to_string(),
        "-o".to_string(),
        "StrictHostKeyChecking=no".to_string(),
        "-o".to_string(),
        "UserKnownHostsFile=/dev/null".to_string(),
        "-o".to_string(),
        "LogLevel=quiet".to_string(),
        format!("{}@127.0.0.1", profile.user),
    ];

    // Only allocate pty for Linux guests
    if profile.os == GuestOs::Linux {
        args.insert(0, "-tt".to_string());
    }

    let status = Command::new("ssh")
        .args(&args)
        .status()
        .map_err(|e| TestbedError::SshFailed {
            port: profile.ssh_port,
            source: anyhow::anyhow!("spawning ssh: {e}"),
        })?;

    if !status.success() {
        return Err(TestbedError::SshFailed {
            port: profile.ssh_port,
            source: anyhow::anyhow!("ssh exited with status {status:?}"),
        });
    }

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Wrap a command in the appropriate shell invocation.
///
/// Linux: `bash -c "..."`
/// Windows: `cmd /c "..."`
fn wrap_command(cmd: &str, os: GuestOs) -> String {
    match os {
        GuestOs::Linux | GuestOs::MacOS => wrap_command_bash(cmd),
        GuestOs::Windows => wrap_command_cmd(cmd),
    }
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
    fn test_wrap_command_linux() {
        let wrapped = wrap_command("echo hello", GuestOs::Linux);
        assert!(wrapped.starts_with("bash -c"));
    }

    #[test]
    fn test_wrap_command_windows() {
        let wrapped = wrap_command("echo hello", GuestOs::Windows);
        assert!(wrapped.starts_with("cmd /c"));
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
