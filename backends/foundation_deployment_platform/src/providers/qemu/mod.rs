//! QEMU provider — Linux host, QEMU/KVM backend.
//!
//! Wraps the existing `crate::qemu::QemuConfig` into the Provider trait.

use std::path::PathBuf;

use crate::config::{DisplayMode, Result, VmProfile};
use crate::providers::{ProviderId, ResolvedPorts, VmHandle, VmProvider};

/// QEMU provider — launches and manages QEMU/KVM VMs on Linux.
pub struct QemuProvider;

impl QemuProvider {
    pub fn new() -> Self {
        QemuProvider
    }
}

impl Default for QemuProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl VmProvider for QemuProvider {
    fn name(&self) -> &'static str {
        "qemu"
    }

    fn id(&self) -> ProviderId {
        ProviderId::Qemu
    }

    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle> {
        let mut config = crate::qemu::QemuConfig::new(profile.clone(), mode);

        // Apply per-VM mount configuration from testbed.toml
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some((host_path, guest_path, readonly, _methods)) =
            crate::config::get_mount_for_profile(profile.name, &cwd.to_string_lossy())
        {
            config = config.with_project_mount(PathBuf::from(&host_path), &guest_path, readonly);
        }

        let qemu_vm = config.launch()?;

        let handle = VmHandle {
            profile: profile.clone(),
            provider_id: ProviderId::Qemu,
            internal_id: qemu_vm.pid().to_string(),
            resolved_ports: ResolvedPorts {
                ssh_port: qemu_vm.resolved_ports.ssh_port,
                winrm_port: qemu_vm.resolved_ports.winrm_port,
                rdp_port: qemu_vm.resolved_ports.rdp_port,
                vnc_port: qemu_vm.resolved_ports.vnc_port,
            },
            display_mode: mode,
        };

        Ok(handle)
    }

    fn stop(&self, handle: &VmHandle) -> Result<()> {
        // Run shutdown scripts before stopping the VM
        run_shutdown_scripts(handle);

        if let Some(pid) = handle.pid() {
            // Try SIGTERM first, wait up to 10 seconds
            unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if unsafe { libc::kill(pid, 0) } != 0 {
                    return Ok(()); // Process exited
                }
            }
            // Force kill if still running
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        Ok(())
    }

    fn is_running(&self, handle: &VmHandle) -> bool {
        if let Some(pid) = handle.pid() {
            unsafe { libc::kill(pid, 0) == 0 }
        } else {
            false
        }
    }

    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts> {
        Ok(handle.resolved_ports.clone())
    }

    fn monitor_command(&self, _handle: &VmHandle, _cmd: &str) -> Result<String> {
        Err(crate::config::TestbedError::MonitorFailed {
            source: anyhow::anyhow!("monitor commands require a live QemuVm instance"),
        })
    }

    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf> {
        crate::import::ensure_image(profile)
    }

    fn host_health(&self) -> crate::doctor::HostHealth {
        crate::doctor::check_host()
    }
}

/// Run startup scripts after the VM is booted and bootstrapped.
///
/// This should be called by the caller after `launch()` + bootstrap
/// completes, before marking the VM as "ready".
pub fn run_startup_scripts_for(handle: &VmHandle) -> Result<()> {
    crate::init::run_startup_scripts(
        handle.profile.name,
        handle.profile.os,
        handle.resolved_ports.ssh_port,
        handle.profile.user,
    )
}

/// Run shutdown scripts via SSH before stopping the VM.
/// Best-effort: if SSH isn't available, scripts are skipped.
fn run_shutdown_scripts(handle: &VmHandle) {
    if let Err(e) = crate::init::run_shutdown_scripts(
        handle.profile.name,
        handle.profile.os,
        handle.resolved_ports.ssh_port,
        handle.profile.user,
    ) {
        eprintln!("  Warning: shutdown scripts failed (non-fatal): {e}");
    }
}
