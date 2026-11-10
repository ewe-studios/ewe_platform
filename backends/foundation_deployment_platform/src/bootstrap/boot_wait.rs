//! Wait for VM services to become available after launch.
//!
//! Polls SSH (port 2222 etc.) and `WinRM` (port 5985) with timeout and
//! progress indication.

use std::thread;
use std::time::Duration;

use foundation_context::Ctx;
use indicatif::ProgressBar;

use crate::config::{GuestOs, Result, TestbedError, VmProfile};

/// Wait for SSH to become available on the VM.
///
/// Polls the forwarded SSH port with a TCP connect. Returns when
/// connection succeeds or timeout is reached.
///
/// # Errors
/// Returns an error if the SSH port does not respond before the timeout.
pub fn wait_for_ssh(profile: &VmProfile, timeout_secs: u64, ctx: &Ctx) -> Result<()> {
    let port = profile.ssh_port;
    let deadline = ctx.clock().now() + Duration::from_secs(timeout_secs);
    let pb = ProgressBar::new_spinner();
    pb.set_message(format!("Waiting for SSH on port {port}..."));
    pb.enable_steady_tick(Duration::from_millis(200));

    while ctx.clock().now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            pb.finish_with_message(format!("SSH ready on 127.0.0.1:{port}"));
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }

    pb.finish_with_message("SSH wait timed out");
    Err(TestbedError::SshFailed {
        port,
        source: anyhow::anyhow!("SSH not available after {timeout_secs}s"),
    })
}

/// Wait for `WinRM` to become available on a Windows VM.
///
/// Polls the forwarded `WinRM` port with a TCP connect. Returns when
/// connection succeeds or timeout is reached.
///
/// # Errors
/// Returns an error if no `WinRM` port is set or the port does not respond before the timeout.
pub fn wait_for_winrm(profile: &VmProfile, timeout_secs: u64, ctx: &Ctx) -> Result<()> {
    let port = profile
        .winrm_port
        .ok_or_else(|| TestbedError::WinrmNotReachable { port: 0 })?;
    let deadline = ctx.clock().now() + Duration::from_secs(timeout_secs);
    let pb = ProgressBar::new_spinner();
    pb.set_message(format!("Waiting for WinRM on port {port}..."));
    pb.enable_steady_tick(Duration::from_millis(200));

    while ctx.clock().now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            pb.finish_with_message(format!("WinRM ready on 127.0.0.1:{port}"));
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }

    pb.finish_with_message("WinRM wait timed out");
    Err(TestbedError::WinrmNotReachable { port })
}

/// Wait for the appropriate services based on profile OS.
///
/// For Windows: waits for both SSH and `WinRM`.
/// For Linux: waits for SSH only.
///
/// # Errors
/// Returns an error if SSH or `WinRM` does not become ready.
pub fn wait_for_vm(profile: &VmProfile, timeout_secs: u64, ctx: &Ctx) -> Result<()> {
    wait_for_ssh(profile, timeout_secs, ctx)?;
    if profile.os == GuestOs::Windows {
        // Give WinRM a bit more time to start after SSH is ready
        thread::sleep(Duration::from_secs(5));
        wait_for_winrm(profile, timeout_secs, ctx)?;
    }
    Ok(())
}
