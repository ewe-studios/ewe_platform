//! Bootstrap CLI handler — installs dev tools on running VMs.
//!
//! Automatically imports and launches the VM if the image is not cached
//! or the VM is not already running, then proceeds with bootstrap.

use clap::ArgMatches;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::bootstrap;
use crate::bootstrap::BootstrapLogger;
use crate::config::{get_profile, GuestOs, DisplayMode};
use crate::providers::{default_provider, ResolvedPorts};
use crate::ssh;
use crate::state;
use crate::winrm::WinRM;
use crate::winrm::elevated::ProgressCallback;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<crate::config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
}

/// Ensure the VM image is available in the cache.
fn ensure_image(profile: &crate::config::VmProfile) -> Result<(), BoxedError> {
    let cache_path = profile.image_cache_path();
    if cache_path.exists() {
        return Ok(());
    }

    eprintln!("  Image not cached, importing...");
    let provider = default_provider()?;
    let path = provider.ensure_image(profile)
        .map_err(|e| format!("Failed to import image: {e}"))?;
    eprintln!("  Image imported at {}", path.display());
    Ok(())
}

/// Launch the VM if not already running. Returns resolved ports.
fn ensure_vm_running(profile: &crate::config::VmProfile) -> Result<ResolvedPorts, BoxedError> {
    // Check if there's existing state with a live PID
    if state::is_running(&profile.name) {
        let vm_state = state::load(&profile.name)
            .expect("is_running returned true but load failed");
        eprintln!("  VM '{}' already running (PID {:?})", profile.name, vm_state.pid);
        return Ok(ResolvedPorts {
            ssh_port: vm_state.ssh_port,
            winrm_port: vm_state.winrm_port,
            rdp_port: vm_state.rdp_port,
            vnc_port: vm_state.vnc_port,
        });
    }

    eprintln!("  VM '{}' not running, launching...", profile.name);
    let provider = default_provider()?;
    let handle = provider.launch(profile, DisplayMode::Headless)
        .map_err(|e| format!("Failed to launch VM: {e}"))?;

    let ports = handle.resolved_ports.clone();
    drop(handle); // Provider saved state already

    eprintln!("  VM launched: SSH={}, VNC={}, WinRM={:?}",
        ports.ssh_port, ports.vnc_port, ports.winrm_port);
    Ok(ports)
}

pub fn cmd_bootstrap(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let force_vsbuild = args.get_flag("force-vsbuild");
    let profile = resolve_profile(name)?;

    // Create logger — writes to $PWD/.testbed/<vm-name>/bootstrap.log
    let logger = Arc::new(
        BootstrapLogger::new(name)
            .map_err(|e| format!("Failed to create bootstrap logger: {e}"))?,
    );

    // Check if already bootstrapped
    if bootstrap::is_bootstrapped(&profile) {
        logger.message("already bootstrapped, skipping.");
        return Ok(());
    }

    // Ensure image is imported
    ensure_image(&profile)?;

    // Ensure VM is running (auto-launch if needed)
    let ports = ensure_vm_running(&profile)?;

    // Progress callback: reports to logger during long-running elevated steps
    let logger_for_progress = Arc::clone(&logger);
    let progress_cb: ProgressCallback<'_> = Some(&|msg: &str| {
        logger_for_progress.message(msg);
    });

    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => bootstrap_ssh(&profile, name, &logger, &ports),
        GuestOs::Windows => bootstrap_windows(&profile, name, &logger, progress_cb, force_vsbuild, &ports),
    }
}

fn bootstrap_ssh(profile: &crate::config::VmProfile, name: &str, logger: &BootstrapLogger, ports: &ResolvedPorts) -> Result<(), BoxedError> {
    // Use resolved ports for SSH connection
    let ssh_port = ports.ssh_port;

    logger.message("→ Connecting via SSH...");
    let mut session = ssh::connect_from_port(ssh_port, &profile.user, profile.os)
        .map_err(|e| format!("SSH connection failed: {e}"))?;
    logger.message("✓ Connected, starting bootstrap...");
    bootstrap::bootstrap(profile, &mut session, logger, None)?;
    logger.message(&format!("✓ {name} bootstrap complete"));
    Ok(())
}

fn bootstrap_windows(profile: &crate::config::VmProfile, name: &str, logger: &BootstrapLogger, progress: ProgressCallback<'_>, force_vsbuild: bool, ports: &ResolvedPorts) -> Result<(), BoxedError> {
    let winrm_port = ports.winrm_port
        .ok_or_else(|| format!("No WinRM port configured for '{}'", profile.name))?;

    logger.message(&format!("→ Waiting for WinRM on port {winrm_port}..."));
    let start = Instant::now();
    let timeout = Duration::from_secs(300);
    loop {
        if start.elapsed() > timeout {
            return Err(format!("Timed out waiting for WinRM after {:?}", timeout).into());
        }
        let winrm = WinRM::new("127.0.0.1", winrm_port, profile.user, profile.pass);
        if winrm.ping() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    logger.message("✓ WinRM ready");

    // Phase 1: WinRM (installs OpenSSH, configures SSH + autologin)
    let winrm = WinRM::new("127.0.0.1", winrm_port, profile.user, profile.pass);
    bootstrap::windows::bootstrap_windows_winrm_phase(profile, &winrm, logger, progress)?;

    // Wait for SSH
    logger.message(&format!("→ Waiting for SSH on port {}...", ports.ssh_port));
    let start = Instant::now();
    let timeout = Duration::from_secs(120);
    loop {
        if start.elapsed() > timeout {
            return Err(crate::config::TestbedError::BootstrapFailed {
                step: "wait for SSH".to_string(),
                message: format!("timed out waiting for SSH after {:?}", timeout),
            }.into());
        }
        if ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    logger.message("✓ SSH ready");

    // Phase 2: SSH (installs dev tools)
    let mut session = ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os)
        .map_err(|e| format!("SSH connection failed: {e}"))?;
    bootstrap::windows::bootstrap_windows_ssh_phase(profile, &winrm, &mut session, logger, progress, force_vsbuild)?;

    // Verify with retry — WinRM may need time to stabilize after nushell setup
    let mut verified = false;
    for attempt in 0..5 {
        if bootstrap::is_bootstrapped(profile) {
            verified = true;
            break;
        }
        if attempt < 4 {
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }
    if !verified {
        return Err(crate::config::TestbedError::BootstrapFailed {
            step: "verification".to_string(),
            message: "bootstrap marker not found after completion".to_string(),
        }.into());
    }

    logger.message(&format!("✓ {name} bootstrap complete"));
    Ok(())
}
