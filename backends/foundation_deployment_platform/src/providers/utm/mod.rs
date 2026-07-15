//! UTM provider — macOS host, UTM/Hypervisor.framework backend.
//!
//! Full implementation using:
//! - `utmctl` CLI for VM lifecycle (list, start, stop, delete)
//! - AppleScript via `osascript` for network config, resource config, bundle import
//! - Vagrant Cloud UTM registry API client for image downloads
//! - `.utm` bundle config.plist generation and modification

use std::path::PathBuf;

use crate::config::{DisplayMode, Result, TestbedError, VmProfile};
use crate::providers::{Provider, ProviderId, ResolvedPorts, VmHandle, VmProvider};

pub mod applescript;
pub mod bundle;
pub mod import;
pub mod state;
pub mod utmctl;

/// Minimum tested UTM version.
pub const MIN_UTM_VERSION: &str = "4.6.5";

/// UTM provider — manages UTM VMs on macOS.
pub struct UtmProvider {
    display: DisplayMode,
}

impl UtmProvider {
    pub fn new() -> Self {
        UtmProvider { display: DisplayMode::Headless }
    }

    #[must_use]
    pub fn with_display(mut self, mode: DisplayMode) -> Self {
        self.display = mode;
        self
    }
}

impl Default for UtmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl VmProvider for UtmProvider {
    fn name(&self) -> &'static str {
        "utm"
    }

    fn id(&self) -> ProviderId {
        ProviderId::Utm
    }

    fn launch(&self, profile: &VmProfile, mode: DisplayMode) -> Result<VmHandle> {
        // Ensure UTM is installed and responsive
        let warnings = utmctl::ensure_utm()?;
        for w in &warnings {
            eprintln!("  Warning: {w}");
        }

        // Ensure the VM bundle is imported into UTM
        let bundle_path = import::ensure_utm_bundle(profile)?;

        // Find the VM in UTM to get its UUID
        let vm_entry = utmctl::find_vm_by_name(profile.name)?.ok_or_else(|| {
            crate::config::TestbedError::Qcow2Error {
                message: format!(
                    "VM '{}' not found in UTM after import — bundle at {}",
                    profile.name,
                    bundle_path.display()
                ),
            }
        })?;

        // Check if already running
        if utmctl::is_vm_running(profile.name)? {
            return Err(crate::config::TestbedError::Qcow2Error {
                message: format!("VM '{}' is already running. Stop it first.", profile.name),
            });
        }

        // Apply per-VM mount configuration from testbed.toml
        if let Some((host_path, _guest_path, readonly, _methods)) =
            crate::config::get_mount_for_profile(profile.name, ".")
        {
            if let Err(e) = applescript::configure_shared_directory(
                &vm_entry.uuid,
                &host_path,
                "project",
                readonly,
            ) {
                eprintln!("  Warning: could not configure shared directory (non-fatal): {e}");
            }
        }

        // Configure resources via AppleScript (CPU, memory)
        if profile.cpu_cores > 0 || profile.memory_mib > 0 {
            let cpus = if profile.cpu_cores > 0 { profile.cpu_cores } else { 4 };
            let mem = if profile.memory_mib > 0 { profile.memory_mib } else { 4096 };
            if let Err(e) = applescript::configure_resources(&vm_entry.uuid, mem, cpus) {
                eprintln!("  Warning: could not configure resources (non-fatal): {e}");
            }
        }

        // Start the VM (with built-in retry logic)
        utmctl::start_vm(profile.name)?;

        // Wait for VM to appear in utmctl list as "started"
        let vm_entry = wait_for_vm_started(profile.name, 120)?;

        // Configure network port forwards via AppleScript
        let mut extra_forwards = Vec::new();
        if let Some(rdp) = profile.rdp_port {
            extra_forwards.push((3389, rdp));
        }
        if let Some(winrm) = profile.winrm_port {
            extra_forwards.push((5985, winrm));
        }
        if let Err(e) = applescript::configure_port_forwards(
            &vm_entry.uuid,
            profile.ssh_port,
            &extra_forwards,
        ) {
            eprintln!("  Warning: could not configure port forwards (non-fatal): {e}");
        }

        // Resolve ports
        let resolved_ports = ResolvedPorts {
            ssh_port: profile.ssh_port,
            winrm_port: profile.winrm_port,
            rdp_port: profile.rdp_port,
            vnc_port: profile.vnc_port,
        };

        // Build handle
        let vm_uuid = vm_entry.uuid.clone();
        let handle = VmHandle {
            profile: profile.clone(),
            provider_id: ProviderId::Utm,
            internal_id: vm_uuid.clone(),
            resolved_ports: resolved_ports.clone(),
            display_mode: mode,
        };

        // Save state
        state::save_from_runtime(
            profile.name,
            bundle_path.to_str().unwrap(),
            &vm_uuid,
            resolved_ports.ssh_port,
            false, // not yet bootstrapped
        )?;

        Ok(handle)
    }

    fn stop(&self, handle: &VmHandle) -> Result<()> {
        // Run shutdown scripts before stopping the VM
        run_shutdown_scripts(handle);

        utmctl::stop_vm(handle.profile.name)?;

        // Clean up state
        let _ = state::delete(handle.profile.name);

        Ok(())
    }

    fn is_running(&self, handle: &VmHandle) -> bool {
        utmctl::is_vm_running(handle.profile.name).unwrap_or(false)
    }

    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts> {
        Ok(handle.resolved_ports.clone())
    }

    fn monitor_command(&self, _handle: &VmHandle, _cmd: &str) -> Result<String> {
        Err(crate::config::TestbedError::MonitorFailed {
            source: anyhow::anyhow!("UTM does not expose a QEMU monitor interface"),
        })
    }

    fn ensure_image(&self, profile: &VmProfile) -> Result<PathBuf> {
        import::ensure_utm_bundle(profile)
    }

    fn host_health(&self) -> crate::doctor::HostHealth {
        crate::doctor::check_host()
    }
}

impl Provider for UtmProvider {
    type Handle = VmHandle;
    type Config = VmProfile;
    type Error = TestbedError;

    fn name(&self) -> &'static str { "utm" }
    fn id(&self) -> ProviderId { ProviderId::Utm }

    fn launch(&self, config: &VmProfile) -> Result<VmHandle> {
        <Self as VmProvider>::launch(self, config, self.display)
    }

    fn stop(&self, handle: &VmHandle) -> Result<()> {
        <Self as VmProvider>::stop(self, handle)
    }

    fn is_running(&self, handle: &VmHandle) -> bool {
        <Self as VmProvider>::is_running(self, handle)
    }

    fn resolved_ports(&self, handle: &VmHandle) -> Result<ResolvedPorts> {
        <Self as VmProvider>::resolved_ports(self, handle)
    }

    fn host_health(&self) -> Vec<(String, bool, String)> {
        let h = <Self as VmProvider>::host_health(self);
        h.checks.iter().map(|c| (c.name.to_string(), c.ok, c.message.clone())).collect()
    }
}

/// Wait for a VM to appear in `utmctl list` with status "started".
fn wait_for_vm_started(name: &str, timeout_secs: u64) -> Result<utmctl::VmEntry> {
    let start = std::time::Instant::now();
    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err(crate::config::TestbedError::Qcow2Error {
                message: format!("VM '{name}' did not start within {timeout_secs}s"),
            });
        }
        if let Some(entry) = utmctl::find_vm_by_name(name)? {
            if entry.status == "started" {
                return Ok(entry);
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
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
