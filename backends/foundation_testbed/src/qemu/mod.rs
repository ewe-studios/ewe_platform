//! QEMU process management — launch, monitor, and shutdown VMs.
//!
//! QEMU runs as a direct child process (not libvirt). All configuration
//! is passed via CLI arguments. User-mode networking provides port
//! forwarding without root privileges.

pub mod disk;
pub mod display;
pub mod download;
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
    /// The QEMU child process.
    process: Child,
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
#[derive(Debug)]
pub struct QemuConfig {
    profile: VmProfile,
    display_mode: DisplayMode,
    extra_args: Vec<String>,
}

impl QemuConfig {
    pub fn new(profile: VmProfile, display_mode: DisplayMode) -> Self {
        Self {
            profile,
            display_mode,
            extra_args: Vec::new(),
        }
    }

    /// Add extra QEMU arguments (for advanced use cases).
    pub fn with_extra_arg(mut self, arg: String) -> Self {
        self.extra_args.push(arg);
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

        // Build disk path
        let disk_path = self.profile.image_cache_path();

        // Build command
        let mut cmd = Command::new(&qemu_bin);
        cmd.args(build_qemu_args(
            &self.profile,
            &disk_path,
            &resolved,
            &monitor_path,
            self.display_mode,
        ));
        cmd.args(&self.extra_args);

        // Detach stdio so the child doesn't inherit the terminal's stdin
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

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

        // Auto-launch VNC viewer for headful mode
        if self.display_mode == DisplayMode::Headful {
            display::launch_viewer(DisplayMode::Headful, resolved.vnc_port);
        }

        Ok(QemuVm {
            process,
            monitor,
            profile: self.profile,
            disk_path,
            pid,
            resolved_ports: resolved,
        })
    }
}

impl QemuVm {
    /// Check if the QEMU process is still running.
    pub fn is_running(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
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

        // Hard kill
        self.process.kill().map_err(|e| TestbedError::Qcow2Error {
            message: format!("failed to kill QEMU process: {e}"),
        })?;
        self.process.wait().ok();

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
    pub fn wait(mut self) -> Result<Option<i32>> {
        let status = self.process.wait().map_err(|e| TestbedError::Qcow2Error {
            message: format!("waiting for QEMU process: {e}"),
        })?;
        Ok(status.code())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build the full QEMU command-line argument list.
fn build_qemu_args(
    profile: &VmProfile,
    disk_path: &std::path::Path,
    ports: &ResolvedPorts,
    monitor_path: &std::path::Path,
    _display: DisplayMode,
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
    args.push("-cpu".to_string());
    args.push("host".to_string());

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

    // Disk
    args.push("-drive".to_string());
    args.push(format!(
        "file={},format=qcow2,if=virtio",
        disk_path.display()
    ));

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

    // GPU
    args.push("-vga".to_string());
    args.push("std".to_string());

    // Monitor
    args.push("-monitor".to_string());
    args.push(format!(
        "unix:{},server,nowait",
        monitor_path.display()
    ));

    // Display (both modes use VNC; headful auto-launches viewer)
    args.push("-display".to_string());
    args.push(format!("vnc=:{}", ports.vnc_port - 5900));

    // No reboot on panic (for headless CI)
    args.push("-no-reboot".to_string());

    args
}

/// Check if KVM is available by testing `/dev/kvm`.
fn kvm_available() -> bool {
    let path = std::path::Path::new("/dev/kvm");
    path.exists()
        && std::fs::metadata(path)
            .map(|m| m.permissions().readonly() == false) // best-effort check
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

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, DisplayMode::Headless);

        assert!(args.contains(&"-enable-kvm".to_string()) || args.iter().any(|a| a.contains("kvm")));
        assert!(args.iter().any(|a| a.contains("12288"))); // RAM
        assert!(args.iter().any(|a| a.contains("virtio-net-pci,netdev=net")));
        assert!(args.iter().any(|a| a.contains("vnc=:0")));
        assert!(args.iter().any(|a| a.contains("hostfwd=tcp:127.0.0.1:2222-:22")));
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

        let args = build_qemu_args(&profile, &disk, &ports, &monitor, DisplayMode::Headful);

        assert!(args.iter().any(|a| a.contains("spice-app")));
        assert!(!args.iter().any(|a| a.contains("vnc")));
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
