//! QEMU process management — launch, monitor, and shutdown VMs.
//!
//! QEMU runs as a direct child process (not libvirt). All configuration
//! is passed via CLI arguments. User-mode networking provides port
//! forwarding without root privileges.

pub mod disk;
pub mod display;
pub mod download;
pub mod mount;
pub mod net;
pub mod snapshot;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::{ensure_dirs, monitor_dir, DisplayMode, GuestOs, Result, TestbedError, VmProfile};

/// A running QEMU VM instance.
///
/// Owns the child process and the monitor socket. When dropped, the VM
/// continues running — call [`QemuVm::shutdown`] for graceful cleanup.
pub struct QemuVm {
    /// The QEMU child process (None if adopted from existing state).
    process: Option<Child>,
    /// Monitor socket for QEMU commands (savevm, loadvm, system_powerdown).
    monitor: UnixStream,
    /// The profile this VM was launched with.
    pub profile: VmProfile,
    /// Path to the qcow2 disk image.
    pub disk_path: std::path::PathBuf,
    /// PID of the QEMU process (for state persistence).
    pub pid: u32,
    /// Resolved ports (may differ from profile defaults if ports were in use).
    pub resolved_ports: ResolvedPorts,
}

/// Actual port numbers the VM is listening on.
#[derive(Debug, Clone)]
pub struct ResolvedPorts {
    pub ssh_port: u16,
    pub winrm_port: Option<u16>,
    pub rdp_port: Option<u16>,
    pub vnc_port: u16,
}

/// Configuration for launching a QEMU VM.
#[derive(Debug, Clone)]
pub struct QemuConfig {
    profile: VmProfile,
    display_mode: DisplayMode,
    extra_args: Vec<String>,
    project_mount: Option<std::path::PathBuf>,
    project_mount_guest_path: Option<String>,
    project_mount_readonly: bool,
    cdrom_path: Option<std::path::PathBuf>,
}

impl QemuConfig {
    pub fn new(profile: VmProfile, display_mode: DisplayMode) -> Self {
        Self {
            profile,
            display_mode,
            extra_args: Vec::new(),
            project_mount: None,
            project_mount_guest_path: None,
            project_mount_readonly: false,
            cdrom_path: None,
        }
    }

    /// Add extra QEMU arguments (for advanced use cases).
    pub fn with_extra_arg(mut self, arg: String) -> Self {
        self.extra_args.push(arg);
        self
    }

    /// Set the host directory to mount into the guest via 9p.
    /// `guest_path` is where it will be mounted inside the VM.
    /// `readonly` controls whether the guest has write access.
    pub fn with_project_mount(
        mut self,
        host_path: std::path::PathBuf,
        guest_path: &str,
        readonly: bool,
    ) -> Self {
        self.project_mount = Some(host_path);
        self.project_mount_guest_path = Some(guest_path.to_string());
        self.project_mount_readonly = readonly;
        self
    }

    /// Get the configured guest mount path for this VM.
    pub fn project_mount_guest_path(&self) -> Option<&str> {
        self.project_mount_guest_path.as_deref()
    }

    /// Attach an ISO as a CD-ROM drive in the guest.
    ///
    /// Used for Windows guests to attach the virtio-win driver ISO.
    pub fn with_cdrom(mut self, path: std::path::PathBuf) -> Self {
        self.cdrom_path = Some(path);
        self
    }

    /// Launch the VM.
    ///
    /// This spawns `qemu-system-x86_64` and connects to the monitor socket.
    /// The guest OS boot is **not** waited for here — use the bootstrap
    /// module to wait for SSH/WinRM after launch.
    pub fn launch(self) -> Result<QemuVm> {
        ensure_dirs().map_err(|e| TestbedError::Qcow2Error {
            message: format!("failed to create cache dirs: {e}"),
        })?;

        // Resolve QEMU binary
        let qemu_bin = find_qemu_system()?;

        // Allocate ports (scan for free ones if defaults are taken)
        let resolved = net::allocate_ports(&self.profile)?;

        // Build monitor socket path
        let monitor_path = monitor_dir().join(format!("{}.monitor", self.profile.name));
        let _ = std::fs::remove_file(&monitor_path); // clean stale socket

        // Build disk path — goes through full resolution chain (env → cache → stores → Vagrant)
        let disk_path = crate::import::ensure_image(&self.profile)?;

        // For Windows guests, attach virtio-win ISO if available (for driver installation)
        let mut config = self;
        if config.profile.os == GuestOs::Windows {
            if let Ok(iso_path) = crate::import::ensure_virtio_iso() {
                config = config.with_cdrom(iso_path);
            }
        }

        // For macOS guests, ensure the full image set exists (BaseSystem + OpenCore + data disk)
        let (macos_boot_disk, macos_efi_disk) = if config.profile.os == GuestOs::MacOS {
            let boot = crate::import::macos::ensure_macos_image(config.profile.name)?;
            let base_dir = boot.parent().unwrap().to_path_buf();
            let efi = base_dir.join(crate::import::macos::OPENCORE_EFI_FILENAME);
            (Some(boot), Some(efi))
        } else {
            (None, None)
        };

        // Build command
        let mut cmd = Command::new(&qemu_bin);
        let (display_args, has_native_window) = if config.display_mode == DisplayMode::Headful {
            let backend = display::detect_backend();
            let vnc_offset = (resolved.vnc_port - 5900) as u32;
            let args = backend.qemu_args(vnc_offset);
            let has_native = matches!(backend, display::DisplayBackend::Spice | display::DisplayBackend::Gtk);
            (args, has_native)
        } else {
            let vnc_offset = (resolved.vnc_port - 5900) as u32;
            (display::DisplayBackend::Vnc.qemu_args(vnc_offset), false)
        };
        let actual_disk = macos_boot_disk.as_ref().unwrap_or(&disk_path);
        cmd.args(build_qemu_args(
            &config.profile,
            actual_disk,
            &resolved,
            &monitor_path,
            display_args,
            has_native_window,
            config.project_mount.as_deref(),
            config.project_mount_readonly,
            config.cdrom_path.as_deref(),
            macos_efi_disk.as_deref(),
        ));
        cmd.args(&config.extra_args);

        // Capture QEMU stderr to the VM work directory: .testbed/[vm-name]/qemu.log
        let work_dir = crate::config::work_dir(&config.profile.name);
        let _ = std::fs::create_dir_all(&work_dir);
        let qemu_log = work_dir.join("qemu.log");
        let qemu_log_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&qemu_log)
            .ok();

        // Detach stdio so the child doesn't inherit the terminal's stdin
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        if let Some(log) = &qemu_log_file {
            cmd.stderr(log.try_clone().unwrap());
        } else {
            cmd.stderr(Stdio::piped());
        }

        let mut process = cmd.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => TestbedError::QemuNotFound {
                install_cmd: "mise install qemu".to_string(),
            },
            _ => TestbedError::Qcow2Error {
                message: format!("failed to spawn QEMU: {e}"),
            },
        })?;

        // Wait for monitor socket to appear (up to 5s)
        let deadline = Instant::now() + Duration::from_secs(5);
        let monitor = loop {
            if Instant::now() > deadline {
                process.kill().ok();
                return Err(TestbedError::QemuExited {
                    code: process.try_wait().ok().flatten().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1),
                });
            }
            match UnixStream::connect(&monitor_path) {
                Ok(s) => break s,
                Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        };

        // Set monitor to non-blocking for read (we write commands, read responses)
        monitor.set_nonblocking(true).ok();

        let pid = process.id();

        // Auto-launch external viewer for headful VNC mode
        if config.display_mode == DisplayMode::Headful && !has_native_window {
            if let Some(name) = display::launch_viewer(display::DisplayBackend::Vnc, resolved.vnc_port) {
                eprintln!("  Viewer: {name} on 127.0.0.1:{}", resolved.vnc_port);
            } else {
                eprintln!("  No VNC viewer found. Connect manually to 127.0.0.1:{}", resolved.vnc_port);
            }
        }

        Ok(QemuVm {
            process: Some(process),
            monitor,
            profile: config.profile,
            disk_path,
            pid,
            resolved_ports: resolved,
        })
    }
}

impl QemuVm {
    /// Attach to an already-running QEMU process via its monitor socket.
    ///
    /// Used by CLI commands (snapshot, stop) that need to communicate with
    /// a VM that was started in a previous process.
    pub fn adopt_running(
        profile: VmProfile,
        disk_path: std::path::PathBuf,
        pid: u32,
        monitor_path: &std::path::Path,
        resolved_ports: ResolvedPorts,
    ) -> Result<Self> {
        let monitor = UnixStream::connect(monitor_path).map_err(|e| TestbedError::Qcow2Error {
            message: format!("connecting to monitor socket {monitor_path:?}: {e}"),
        })?;
        monitor.set_nonblocking(true).ok();

        Ok(QemuVm {
            process: None, // We don't own this process
            monitor,
            profile,
            disk_path,
            pid,
            resolved_ports,
        })
    }

    /// Check if the QEMU process is still running.
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            matches!(child.try_wait(), Ok(None))
        } else {
            // Adopted VM — check via /proc
            std::path::Path::new(&format!("/proc/{}", self.pid)).exists()
        }
    }

    /// Get the PID of the QEMU process.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Gracefully shut down the VM.
    ///
    /// Sends `system_powerdown` via the QEMU monitor, then waits up to
    /// 30 seconds for the process to exit. Falls back to `kill()` if
    /// the guest doesn't respond.
    pub fn shutdown(&mut self) -> Result<()> {
        if !self.is_running() {
            return Ok(()); // already exited
        }

        // Try graceful shutdown via monitor
        if self.monitor_command("system_powerdown").is_ok() {
            let deadline = Instant::now() + Duration::from_secs(30);
            while Instant::now() < deadline {
                thread::sleep(Duration::from_millis(500));
                if !self.is_running() {
                    return Ok(());
                }
            }
        }

        // Hard kill (only if we own the process)
        if let Some(ref mut child) = self.process {
            let _ = child.kill();
        } else {
            // Adopted VM — send SIGTERM via libc
            unsafe { libc::kill(self.pid as i32, libc::SIGTERM) };
            // Wait briefly
            for _ in 0..30 {
                thread::sleep(Duration::from_millis(500));
                if !std::path::Path::new(&format!("/proc/{}", self.pid)).exists() {
                    return Ok(());
                }
            }
            unsafe { libc::kill(self.pid as i32, libc::SIGKILL) };
        }
        if let Some(ref mut child) = self.process {
            child.wait().ok();
        }

        // Clean up monitor socket
        let monitor_path = monitor_dir().join(format!("{}.monitor", self.profile.name));
        let _ = std::fs::remove_file(&monitor_path);

        Ok(())
    }

    /// Send a command to the QEMU monitor and read the response.
    ///
    /// The monitor socket is a simple text protocol: write a command
    /// followed by `\n`, read the response until `(qemu)` prompt.
    pub fn monitor_command(&mut self, cmd: &str) -> Result<String> {
        // The socket is non-blocking for reads, so we need to handle
        // WouldBlock errors. Write the command first.
        self.monitor
            .write_all(format!("{cmd}\n").as_bytes())
            .map_err(|e| TestbedError::MonitorFailed {
                source: anyhow::anyhow!("writing monitor command: {e}"),
            })?;
        self.monitor
            .flush()
            .map_err(|e| TestbedError::MonitorFailed {
                source: anyhow::anyhow!("flushing monitor command: {e}"),
            })?;

        // Read response until (qemu) prompt
        let reader = BufReader::new(&self.monitor);
        let mut output = String::new();
        let deadline = Instant::now() + Duration::from_secs(5);

        for line in reader.lines() {
            if Instant::now() > deadline {
                return Err(TestbedError::MonitorFailed {
                    source: anyhow::anyhow!("monitor response timeout"),
                });
            }
            match line {
                Ok(l) => {
                    if l.contains("(qemu)") {
                        break;
                    }
                    if !l.is_empty() && l != cmd {
                        output.push_str(&l);
                        output.push('\n');
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
                Err(e) => {
                    return Err(TestbedError::MonitorFailed {
                        source: anyhow::anyhow!("reading monitor response: {e}"),
                    });
                }
            }
        }

        Ok(output.trim().to_string())
    }

    /// Wait for the process to exit, returning the exit code.
    /// Only works for VMs we launched (not adopted ones).
    pub fn wait(self) -> Result<Option<i32>> {
        let Some(mut child) = self.process else {
            return Ok(None); // Adopted VM — no child to wait on
        };
        let status = child.wait().map_err(|e| TestbedError::Qcow2Error {
            message: format!("waiting for QEMU process: {e}"),
        })?;
        Ok(status.code())
    }
}

/// Adopt an existing qcow2 image as a managed VM.
///
/// 1. Validates the qcow2 file exists and is non-empty
/// 2. Copies the disk into the image cache under a unique name
/// 3. Creates a state file with `bootstrapped: false`
/// 4. Returns the resolved profile + disk path so the caller can launch
pub fn adopt(
    profile: &VmProfile,
    disk_path: &std::path::Path,
) -> Result<(VmProfile, std::path::PathBuf)> {
    if !disk_path.exists() {
        return Err(TestbedError::Qcow2Error {
            message: format!("disk file not found: {}", disk_path.display()),
        });
    }
    let meta = std::fs::metadata(disk_path).map_err(|e| TestbedError::Qcow2Error {
        message: format!("stat {}: {e}", disk_path.display()),
    })?;
    if meta.len() == 0 {
        return Err(TestbedError::Qcow2Error {
            message: format!("disk file is empty: {}", disk_path.display()),
        });
    }

    // Copy into cache
    let cache_dir = crate::config::image_cache_dir();
    let dest = cache_dir.join(format!("adopted-{}.qcow2", profile.name));
    std::fs::copy(disk_path, &dest).map_err(|e| TestbedError::Qcow2Error {
        message: format!("copy {} → {}: {e}", disk_path.display(), dest.display()),
    })?;

    // Create state file (VM not yet started)
    crate::state::save(&crate::state::VmState {
        profile_name: profile.name.to_string(),
        disk_path: dest.to_string_lossy().to_string(),
        pid: None,
        provider_id: crate::providers::ProviderId::Qemu,
        provider_internal_id: String::new(),
        monitor_socket: String::new(),
        ssh_port: profile.ssh_port,
        winrm_port: profile.winrm_port,
        rdp_port: profile.rdp_port,
        vnc_port: profile.vnc_port,
        bootstrapped: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    })?;

    Ok((profile.clone(), dest))
}

/// Refresh network ports for a running or stopped VM.
///
/// 1. Stops the VM gracefully (or force-kills if unresponsive)
/// 2. Waits for the QEMU process to fully exit
/// 3. Reallocates ports (checks for conflicts)
/// 4. Relaunches the VM with fresh port forwarding
/// 5. Waits for SSH/WinRM to become reachable
pub fn refresh_network(
    profile: &VmProfile,
    display_mode: DisplayMode,
) -> Result<QemuVm> {
    // Stop if running
    if let Ok(state) = crate::state::load(profile.name)
        && let Some(pid) = state.pid {
            // Check if process is still alive via /proc
            let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
            if alive {
                // Try graceful stop via QEMU monitor
                if let Ok(mut vm) = QemuConfig::new(profile.clone(), display_mode).launch() {
                    vm.shutdown()?;
                }
                // Wait for process to fully exit
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
                while std::time::Instant::now() < deadline {
                    if !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }

    // Relaunch with fresh ports
    let vm = QemuConfig::new(profile.clone(), display_mode).launch()?;

    // Save updated state with new ports
    crate::state::save(&crate::state::VmState {
        profile_name: profile.name.to_string(),
        disk_path: vm.disk_path.to_string_lossy().to_string(),
        pid: Some(vm.pid),
        provider_id: crate::providers::ProviderId::Qemu,
        provider_internal_id: vm.pid.to_string(),
        monitor_socket: String::new(),
        ssh_port: vm.resolved_ports.ssh_port,
        winrm_port: vm.resolved_ports.winrm_port,
        rdp_port: vm.resolved_ports.rdp_port,
        vnc_port: vm.resolved_ports.vnc_port,
        bootstrapped: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    })?;

    Ok(vm)
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build the full QEMU command-line argument list.
fn build_qemu_args(
    profile: &VmProfile,
    disk_path: &std::path::Path,
    ports: &ResolvedPorts,
    monitor_path: &std::path::Path,
    display_args: Vec<String>,
    _has_native_window: bool,
    project_mount: Option<&std::path::Path>,
    project_mount_readonly: bool,
    cdrom_path: Option<&std::path::Path>,
    macos_efi_disk: Option<&std::path::Path>,
) -> Vec<String> {
    let mut args = Vec::new();

    // Acceleration
    if kvm_available() {
        args.push("-enable-kvm".to_string());
    }

    // Resources
    args.push("-m".to_string());
    args.push(profile.memory_mib.to_string());
    args.push("-smp".to_string());
    args.push(profile.cpu_cores.to_string());

    // CPU model
    if profile.os == GuestOs::MacOS {
        args.push("-cpu".to_string());
        args.push("Penryn,kvm=on,vendor=GenuineIntel,+invtsc,vmware-cpuid-freq=on".to_string());
        args.push("-machine".to_string());
        args.push("q35".to_string());
    } else {
        args.push("-cpu".to_string());
        args.push("host".to_string());
    }

    // UEFI/OVMF firmware for Windows (required for boot)
    if profile.os == GuestOs::Windows {
        let vars_path = crate::config::state_dir().join(format!("{}.nvram", profile.name));
        if !vars_path.exists() {
            let _ = std::fs::copy("/usr/share/edk2/x64/OVMF_VARS.4m.fd", &vars_path);
        }
        args.push("-drive".to_string());
        args.push("file=/usr/share/edk2/x64/OVMF_CODE.4m.fd,if=pflash,format=raw,readonly=on".to_string());
        args.push("-drive".to_string());
        args.push(format!("file={},if=pflash,format=raw", vars_path.display()));
    }

    // macOS-specific devices (SMC, USB, SATA, audio)
    if profile.os == GuestOs::MacOS {
        // Apple SMC (System Management Controller)
        args.push("-device".to_string());
        args.push("isa-applesmc,osk=\"ourhardworkbythesewordsguardedpleasedontsteal(c)AppleComputerInc\"".to_string());

        // USB controller + keyboard + tablet (XHCI, not legacy USB)
        args.push("-device".to_string());
        args.push("qemu-xhci".to_string());
        args.push("-device".to_string());
        args.push("usb-kbd".to_string());
        args.push("-device".to_string());
        args.push("usb-tablet".to_string());

        // SATA controller (macOS boots from AHCI, not virtio)
        args.push("-device".to_string());
        args.push("ich9-ahci,id=sata".to_string());

        // Audio
        args.push("-device".to_string());
        args.push("ich9-intel-hda".to_string());
        args.push("-device".to_string());
        args.push("hda-output".to_string());
    }

    // Disk
    if profile.os == GuestOs::MacOS {
        // OpenCore EFI disk (first SATA device, acts as bootloader)
        if let Some(efi_disk) = macos_efi_disk {
            args.push("-drive".to_string());
            args.push(format!(
                "file={},format=qcow2,if=none,id=OpenCore",
                efi_disk.display()
            ));
            args.push("-device".to_string());
            args.push("ide-hd,bus=sata.1,drive=OpenCore".to_string());
        }

        // BaseSystem / boot disk (second SATA device)
        args.push("-drive".to_string());
        args.push(format!(
            "file={},format=qcow2,if=none,id=macOS",
            disk_path.display()
        ));
        args.push("-device".to_string());
        args.push("ide-hd,bus=sata.2,drive=macOS".to_string());

        // Data disk (third SATA device — user space, resizable)
        if let Some(data_dir) = disk_path.parent() {
            let data_disk = data_dir.join("macOS-data.qcow2");
            if data_disk.exists() {
                args.push("-drive".to_string());
                args.push(format!(
                    "file={},format=qcow2,if=none,id=MacData",
                    data_disk.display()
                ));
                args.push("-device".to_string());
                args.push("ide-hd,bus=sata.3,drive=MacData".to_string());
            }
        }
    } else {
        args.push("-drive".to_string());
        args.push(format!(
            "file={},format=qcow2,if=virtio",
            disk_path.display()
        ));
    }

    // CD-ROM (optional — used for virtio-win ISO on Windows guests)
    if let Some(cdrom) = cdrom_path {
        args.push("-drive".to_string());
        args.push(format!("file={},media=cdrom", cdrom.display()));
    }

    // Project mount via 9p (if configured).
    // Windows guests need the viofs driver (installed during bootstrap) to
    // recognize this device. The virtfs arg is passed for all OS types.
    if let Some(host_path) = project_mount {
        args.extend(mount::mount_args(host_path, mount::DEFAULT_TAG, project_mount_readonly));
    }

    // Network (user-mode with port forwarding)
    let mut netdev = "user,id=net".to_string();
    netdev.push_str(&format!(",hostfwd=tcp:127.0.0.1:{}-:22", ports.ssh_port));
    if let Some(p) = ports.winrm_port {
        netdev.push_str(&format!(",hostfwd=tcp:127.0.0.1:{p}-:5985"));
    }
    if let Some(p) = ports.rdp_port {
        netdev.push_str(&format!(",hostfwd=tcp:127.0.0.1:{p}-:3389"));
    }
    args.push("-netdev".to_string());
    args.push(netdev);
    args.push("-device".to_string());
    args.push("virtio-net-pci,netdev=net".to_string());

    // GPU (macOS uses whatever's attached to the SATA bus; std VGA conflicts)
    if profile.os != GuestOs::MacOS {
        args.push("-vga".to_string());
        args.push("std".to_string());
    }

    // USB tablet (fixes mouse tracking in VNC viewers) — not for macOS (uses qemu-xhci above)
    if profile.os != GuestOs::MacOS {
        args.push("-usb".to_string());
        args.push("-device".to_string());
        args.push("usb-tablet".to_string());
    }

    // Monitor
    args.push("-monitor".to_string());
    args.push(format!(
        "unix:{},server,nowait",
        monitor_path.display()
    ));

    // Display (auto-detected: spice-app, gtk, or vnc)
    args.extend(display_args);

    // No reboot on panic (for headless CI)
    args.push("-no-reboot".to_string());

    args
}

/// Check if KVM is available by testing `/dev/kvm`.
fn kvm_available() -> bool {
    let path = std::path::Path::new("/dev/kvm");
    path.exists()
        && std::fs::metadata(path)
            .map(|m| !m.permissions().readonly()) // best-effort check
            .unwrap_or(false)
}

/// Find `qemu-system-x86_64` on PATH, trying common locations.
fn find_qemu_system() -> Result<std::path::PathBuf> {
    if let Ok(p) = which::which("qemu-system-x86_64") {
        return Ok(p);
    }
    // Common Homebrew / Nix locations
    let candidates = [
        "/opt/homebrew/bin/qemu-system-x86_64",
        "/usr/local/bin/qemu-system-x86_64",
        "/run/current-system/sw/bin/qemu-system-x86_64",
    ];
    for c in &candidates {
        let p = std::path::PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
    }
    Err(TestbedError::QemuNotFound {
        install_cmd: "mise install qemu".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::get_profile;

    #[test]
    fn test_build_qemu_args_headless() {
        let profile = get_profile("windows-build").unwrap();
        let ports = ResolvedPorts {
            ssh_port: 2222,
            winrm_port: Some(5985),
            rdp_port: Some(3389),
            vnc_port: 5900,
        };
        let monitor = std::path::PathBuf::from("/tmp/test.monitor");
        let disk = std::path::PathBuf::from("/tmp/test.qcow2");
        let display_args = display::DisplayBackend::Vnc.qemu_args(0);

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, None, None);

        assert!(args.contains(&"-enable-kvm".to_string()) || args.iter().any(|a| a.contains("kvm")));
        assert!(args.iter().any(|a| a.contains("12288"))); // RAM
        assert!(args.iter().any(|a| a.contains("virtio-net-pci,netdev=net")));
        assert!(args.iter().any(|a| a.contains("vnc=:0")));
        assert!(args.iter().any(|a| a.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
        assert!(args.iter().any(|a| a.contains("usb-tablet"))); // mouse tracking fix
    }

    #[test]
    fn test_build_qemu_args_headful() {
        let profile = get_profile("linux-build").unwrap();
        let ports = ResolvedPorts {
            ssh_port: 2422,
            winrm_port: None,
            rdp_port: None,
            vnc_port: 5902,
        };
        let monitor = std::path::PathBuf::from("/tmp/test.monitor");
        let disk = std::path::PathBuf::from("/tmp/test.qcow2");
        let display_args = display::DisplayBackend::Vnc.qemu_args(2);

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, None, None);

        assert!(args.iter().any(|a| a.contains("vnc=:2")));
        assert!(args.iter().any(|a| a.contains("usb-tablet")));
    }

    #[test]
    fn test_build_qemu_args_with_mount() {
        let profile = get_profile("linux-build").unwrap();
        let ports = ResolvedPorts {
            ssh_port: 2422,
            winrm_port: None,
            rdp_port: None,
            vnc_port: 5902,
        };
        let monitor = std::path::PathBuf::from("/tmp/test.monitor");
        let disk = std::path::PathBuf::from("/tmp/test.qcow2");
        let display_args = display::DisplayBackend::Vnc.qemu_args(2);
        let project_mount = std::path::PathBuf::from("/home/user/project");

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, Some(&project_mount), false, None, None);

        assert!(args.iter().any(|a| a.contains("-virtfs")));
        assert!(args.iter().any(|a| a.contains("path=/home/user/project")));
        assert!(args.iter().any(|a| a.contains("mount_tag=project")));
    }

    #[test]
    fn test_build_qemu_args_with_cdrom() {
        let profile = get_profile("windows-build").unwrap();
        let ports = ResolvedPorts {
            ssh_port: 2222,
            winrm_port: Some(5985),
            rdp_port: Some(3389),
            vnc_port: 5900,
        };
        let monitor = std::path::PathBuf::from("/tmp/test.monitor");
        let disk = std::path::PathBuf::from("/tmp/test.qcow2");
        let display_args = display::DisplayBackend::Vnc.qemu_args(0);
        let cdrom = std::path::PathBuf::from("/tmp/virtio-win.iso");

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, display_args, false, None, false, Some(&cdrom), None);

        assert!(args.iter().any(|a| a.contains("media=cdrom")));
        assert!(args.iter().any(|a| a.contains("/tmp/virtio-win.iso")));
    }

    #[test]
    fn test_all_profiles_parseable() {
        use crate::config::list_profiles;
        let profiles = list_profiles();
        assert!(!profiles.is_empty());
        for p in profiles {
            assert!(!p.name.is_empty());
            assert!(p.memory_mib > 0);
            assert!(p.cpu_cores > 0);
            assert!(p.disk_gb > 0);
        }
    }
}
