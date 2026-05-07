//! Bootstrap CLI handler — installs dev tools on running VMs.

use clap::ArgMatches;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::bootstrap;
use crate::bootstrap::BootstrapLogger;
use crate::config::{get_profile, GuestOs};
use crate::ssh;
use crate::winrm::WinRM;
use crate::winrm::elevated::ProgressCallback;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<crate::config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
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

    // Progress callback: reports to logger during long-running elevated steps
    let logger_for_progress = Arc::clone(&logger);
    let progress_cb: ProgressCallback<'_> = Some(&|msg: &str| {
        logger_for_progress.message(msg);
    });

    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => bootstrap_ssh(&profile, name, &logger),
        GuestOs::Windows => bootstrap_windows(&profile, name, &logger, progress_cb, force_vsbuild),
    }
}

fn bootstrap_ssh(profile: &crate::config::VmProfile, name: &str, logger: &BootstrapLogger) -> Result<(), BoxedError> {
    logger.message("→ Connecting via SSH...");
    let mut session = ssh::connect(profile)?;
    logger.message("✓ Connected, starting bootstrap...");
    bootstrap::bootstrap(profile, &mut session, logger, None)?;
    logger.message(&format!("✓ {name} bootstrap complete"));
    Ok(())
}

fn bootstrap_windows(profile: &crate::config::VmProfile, name: &str, logger: &BootstrapLogger, progress: ProgressCallback<'_>, force_vsbuild: bool) -> Result<(), BoxedError> {
    // Wait for WinRM
    let port = profile.winrm_port.ok_or_else(|| {
        format!("No WinRM port configured for '{}'", profile.name)
    })?;
    logger.message(&format!("→ Waiting for WinRM on port {port}..."));
    let start = Instant::now();
    let timeout = Duration::from_secs(300);
    loop {
        if start.elapsed() > timeout {
            return Err(format!("Timed out waiting for WinRM after {:?}", timeout).into());
        }
        let winrm = WinRM::new("127.0.0.1", port, profile.user, profile.pass);
        if winrm.ping() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    logger.message("✓ WinRM ready");

    // Phase 1: WinRM (installs OpenSSH, configures SSH + autologin)
    let winrm = WinRM::new("127.0.0.1", port, profile.user, profile.pass);
    bootstrap::windows::bootstrap_windows_winrm_phase(profile, &winrm, logger, progress)?;

    // Wait for SSH
    logger.message(&format!("→ Waiting for SSH on port {}...", profile.ssh_port));
    let start = Instant::now();
    let timeout = Duration::from_secs(120);
    loop {
        if start.elapsed() > timeout {
            return Err(crate::config::TestbedError::BootstrapFailed {
                step: "wait for SSH".to_string(),
                message: format!("timed out waiting for SSH after {:?}", timeout),
            }.into());
        }
        if ssh::connect(profile).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    logger.message("✓ SSH ready");

    // Phase 2: SSH (installs dev tools)
    let mut session = ssh::connect(profile)?;
    bootstrap::windows::bootstrap_windows_ssh_phase(profile, &winrm, &mut session, logger, progress, force_vsbuild)?;

    // Verify
    if !bootstrap::is_bootstrapped(profile) {
        return Err(crate::config::TestbedError::BootstrapFailed {
            step: "verification".to_string(),
            message: "bootstrap marker not found after completion".to_string(),
        }.into());
    }

    logger.message(&format!("✓ {name} bootstrap complete"));
    Ok(())
}
