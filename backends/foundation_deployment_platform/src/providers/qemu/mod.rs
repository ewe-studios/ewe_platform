//! QEMU provider — Linux host, QEMU/KVM backend.

use std::path::PathBuf;

use crate::config::{DisplayMode, Result, TestbedError, VmProfile};
use crate::providers::{Provider, ProviderId, ResolvedPorts, VmHandle, VmProvider};

/// QEMU provider — launches and manages QEMU/KVM VMs on Linux.
pub struct QemuProvider {
    display: DisplayMode,
}

impl QemuProvider {
    pub fn new() -> Self {
        QemuProvider { display: DisplayMode::Headless }
    }

    #[must_use]
    pub fn with_display(mut self, mode: DisplayMode) -> Self {
        self.display = mode;
        self
    }

    // ── Shared helpers ──

    fn launch_vm(profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle> {
        let mut config = crate::qemu::QemuConfig::new(profile.clone(), mode);

        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if let Some((host_path, guest_path, readonly, _methods)) =
            crate::config::get_mount_for_profile(profile.name, &cwd.to_string_lossy())
        {
            config = config.with_project_mount(PathBuf::from(&host_path), &guest_path, readonly);
        }

        let qemu_vm = config.launch()?;

        Ok(VmHandle {
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
        })
    }

    fn stop_vm(handle: &VmHandle) -> Result<()> {
        run_shutdown_scripts(handle);
        if let Some(pid) = handle.pid() {
            unsafe { libc::kill(pid, libc::SIGTERM) };
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if unsafe { libc::kill(pid, 0) } != 0 {
                    return Ok(());
                }
            }
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        Ok(())
    }

    fn is_vm_running(handle: &VmHandle) -> bool {
        handle.pid().map_or(false, |pid| unsafe { libc::kill(pid, 0) == 0 })
    }
}

impl Default for QemuProvider {
    fn default() -> Self {
        Self::new()
    }
}

// ── Associated-types Provider (canonical impl) ─────────────────────────

impl Provider for QemuProvider {
    type Handle = VmHandle;
    type Config = VmProfile;
    type Error = TestbedError;

    fn name(&self) -> &'static str { "qemu" }
    fn id(&self) -> ProviderId { ProviderId::Qemu }

    fn launch(&self, config: &VmProfile) -> Result<VmHandle> {
        Self::launch_vm(config, self.display)
    }

    fn stop(&self, handle: &VmHandle) -> Result<()> {
        Self::stop_vm(handle)
    }

    fn is_running(&self, handle: &VmHandle) -> bool {
        Self::is_vm_running(handle)
    }

    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts> {
        Ok(handle.resolved_ports.clone())
    }

    fn host_health(&self) -> Vec<(String, bool, String)> {
        crate::doctor::check_host()
            .checks
            .iter()
            .map(|c| (c.name.to_string(), c.ok, c.message.clone()))
            .collect()
    }
}

// ── VmProvider (compatibility — delegates to Provider) ──────────────────

impl VmProvider for QemuProvider {
    fn name(&self) -> &'static str { <Self as Provider>::name(self) }
    fn id(&self) -> ProviderId { <Self as Provider>::id(self) }
    fn launch(&self, profile: &VmProfile, _mode: DisplayMode) -> Result<VmHandle> {
        // mode is ignored — caller should use QemuProvider::with_display() instead
        <Self as Provider>::launch(self, profile)
    }
    fn stop(&self, handle: &VmHandle) -> Result<()> { <Self as Provider>::stop(self, handle) }
    fn is_running(&self, handle: &VmHandle) -> bool { <Self as Provider>::is_running(self, handle) }
    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts> { <Self as Provider>::resolved_ports(self, handle) }
    fn monitor_command(&self, _handle: &VmHandle, _cmd: &str) -> Result<String> {
        Err(TestbedError::MonitorFailed {
            source: anyhow::anyhow!("monitor commands require a live QemuVm instance"),
        })
    }
    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf> {
        crate::import::ensure_image(profile)
    }
    fn host_health(&self) -> crate::doctor::HostHealth { crate::doctor::check_host() }
}

// ── Public API ──────────────────────────────────────────────────────────

/// Run startup scripts after the VM is booted and bootstrapped.
pub fn run_startup_scripts_for(handle: &VmHandle) -> Result<()> {
    crate::init::run_startup_scripts(
        handle.profile.name,
        handle.profile.os,
        handle.resolved_ports.ssh_port,
        handle.profile.user,
    )
}

/// Run shutdown scripts via SSH before stopping the VM.
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
