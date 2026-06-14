//! Build CLI command — run cargo tauri build inside a VM and retrieve artifacts.
//!
//! Usage: testbed build <profile> --project <path> [--target <triple>]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::ArgMatches;

use crate::vms::bootstrap::BootstrapLogger;
use crate::vms::config::{get_profile, DisplayMode, GuestOs};
use crate::vms::providers::{default_provider, ResolvedPorts};
use crate::vms::state;
use crate::vms::winrm::WinRM;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<crate::vms::config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
}

/// Ensure the VM image is available in the cache.
fn ensure_image(profile: &crate::vms::config::VmProfile) -> Result<(), BoxedError> {
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
fn ensure_vm_running(profile: &crate::vms::config::VmProfile) -> Result<ResolvedPorts, BoxedError> {
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

/// Ensure the VM is bootstrapped with dev tools.
fn ensure_bootstrapped(
    profile: &crate::vms::config::VmProfile,
    ports: &ResolvedPorts,
) -> Result<(), BoxedError> {
    if crate::vms::bootstrap::is_bootstrapped(profile) {
        eprintln!("  VM already bootstrapped, skipping.");
        return Ok(());
    }

    eprintln!("  Bootstrapping VM...");
    let logger = Arc::new(
        BootstrapLogger::new(&profile.name)
            .map_err(|e| format!("Failed to create bootstrap logger: {e}"))?,
    );

    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => {
            let mut session = crate::vms::ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os)
                .map_err(|e| format!("SSH connection failed: {e}"))?;
            crate::vms::bootstrap::bootstrap(profile, &mut session, &logger, None)?;
        }
        GuestOs::Windows => {
            bootstrap_windows(profile, &logger, ports)?;
        }
    }

    // Update state to mark as bootstrapped
    if let Ok(mut state) = state::load(&profile.name) {
        state.bootstrapped = true;
        let _ = state::save(&state);
    }

    eprintln!("  Bootstrap complete.");
    Ok(())
}

/// Bootstrap Windows VM (WinRM phase + SSH phase).
fn bootstrap_windows(
    profile: &crate::vms::config::VmProfile,
    logger: &BootstrapLogger,
    ports: &ResolvedPorts,
) -> Result<(), BoxedError> {
    use crate::vms::winrm::elevated::ProgressCallback;
    use std::time::Instant;

    let winrm_port = ports.winrm_port
        .ok_or_else(|| format!("No WinRM port configured for '{}'", profile.name))?;

    eprintln!("  Waiting for WinRM on port {winrm_port}...");
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
    eprintln!("  WinRM ready");

    let winrm = WinRM::new("127.0.0.1", winrm_port, profile.user, profile.pass);
    let progress_cb: ProgressCallback<'_> = Some(&|msg: &str| {
        eprintln!("    {}", msg);
    });

    // Phase 1: WinRM
    crate::vms::bootstrap::windows::bootstrap_windows_winrm_phase(profile, &winrm, logger, progress_cb)?;

    // Wait for SSH
    eprintln!("  Waiting for SSH on port {}...", ports.ssh_port);
    let start = Instant::now();
    let timeout = Duration::from_secs(120);
    loop {
        if start.elapsed() > timeout {
            return Err("Timed out waiting for SSH".into());
        }
        if crate::vms::ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_secs(5));
    }
    eprintln!("  SSH ready");

    // Phase 2: SSH
    let mut session = crate::vms::ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os)
        .map_err(|e| format!("SSH connection failed: {e}"))?;
    crate::vms::bootstrap::windows::bootstrap_windows_ssh_phase(profile, &winrm, &mut session, logger, progress_cb, false)?;

    Ok(())
}

/// Build a project inside a VM and retrieve artifacts.
pub fn cmd_build(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let project_dir = args.get_one::<String>("project").unwrap();
    let _target_override = args.get_one::<String>("target");
    let no_artifacts = args.get_flag("no-artifacts");

    let profile = resolve_profile(name)?;

    let project_path = PathBuf::from(project_dir);
    if !project_path.exists() {
        return Err(format!("Project directory does not exist: {}", project_path.display()).into());
    }
    if !project_path.is_dir() {
        return Err(format!("Project path is not a directory: {}", project_path.display()).into());
    }

    // Ensure image is imported
    ensure_image(&profile)?;

    // Ensure VM is running
    let ports = ensure_vm_running(&profile)?;

    // Ensure VM is bootstrapped
    ensure_bootstrapped(&profile, &ports)?;

    // Connect via SSH
    eprintln!("Connecting to VM via SSH...");
    let mut session = crate::vms::ssh::connect_from_port(ports.ssh_port, &profile.user, profile.os)
        .map_err(|e| format!("SSH connection failed: {e}"))?;

    // Run the build
    eprintln!("Building project in VM...");
    eprintln!("  Project: {}", project_path.display());

    let artifact_dir = match crate::vms::build::build_in_vm(&profile, &mut session, &project_path) {
        Ok(dir) => dir,
        Err(crate::vms::config::TestbedError::BuildFailed { target, code }) => {
            eprintln!("\nBuild failed for target {} (exit code: {})", target, code);
            eprintln!("Check logs with: testbed logs {} --errors", name);
            return Err(format!("Build failed with exit code {}", code).into());
        }
        Err(e) => {
            return Err(format!("Build error: {e}").into());
        }
    };

    if no_artifacts {
        eprintln!("\nBuild complete. Artifacts left in VM (--no-artifacts specified).");
    } else {
        eprintln!("\nBuild complete. Artifacts retrieved to: {}", artifact_dir.display());

        // List artifacts
        if let Ok(entries) = std::fs::read_dir(&artifact_dir) {
            eprintln!("\nArtifacts:");
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    let size_mb = meta.len() as f64 / 1_048_576.0;
                    eprintln!("  {:.2} MB  {}", size_mb, entry.file_name().to_string_lossy());
                }
            }
        }
    }

    Ok(())
}
