//! Shared test fixtures for foundation_testbed integration tests.
//!
//! Provides `TestVm` — an RAII guard that starts a VM on construction
//! and stops it on drop, ensuring cleanup even on test panic.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use foundation_testbed::config::{DisplayMode, GuestOs, Result, VmProfile, get_profile};
use foundation_testbed::qemu::{QemuConfig, QemuVm};
use foundation_testbed::qemu::mount;
use foundation_testbed::ssh;
use foundation_testbed::state;

/// RAII guard: starts a VM on creation, stops it on drop.
///
/// ```ignore
/// let vm = TestVm::new("linux-build")?;
/// vm.ssh_exec("echo hello")?;
/// // VM is automatically stopped when vm goes out of scope
/// ```
pub struct TestVm {
    profile_name: String,
    profile: VmProfile,
    qemu: Option<QemuVm>,
    ssh_port: u16,
    mount_project: bool,
}

impl TestVm {
    /// Start a VM by profile name and wait for it to be ready.
    pub fn new(profile_name: &str) -> Result<Self> {
        Self::new_with_mount(profile_name, true)
    }

    /// Start a VM, optionally with a project mount.
    pub fn new_with_mount(profile_name: &str, mount_project: bool) -> Result<Self> {
        let profile = get_profile(profile_name).map(|p| p.clone())?;
        println!("[{}] Starting VM '{}'...", std::any::type_name::<Self>(), profile_name);

        // Ensure image is available
        let _disk = foundation_testbed::import::ensure_image(&profile)?;

        let mut config = QemuConfig::new(profile.clone(), DisplayMode::Headless);
        if mount_project {
            config = config.with_project_mount(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        }

        let qemu = config.launch()?;
        let ssh_port = qemu.resolved_ports.ssh_port;

        // Save state
        let monitor_path = foundation_testbed::config::monitor_dir()
            .join(format!("{}.monitor", profile.name));
        let vm_state = state::from_qemu(
            &profile.name,
            &qemu.disk_path,
            qemu.pid,
            &monitor_path,
            qemu.resolved_ports.ssh_port,
            qemu.resolved_ports.winrm_port,
            qemu.resolved_ports.rdp_port,
            qemu.resolved_ports.vnc_port,
            false,
        );
        state::save(&vm_state).ok();

        Ok(Self {
            profile_name: profile_name.to_string(),
            profile,
            qemu: Some(qemu),
            ssh_port,
            mount_project,
        })
    }

    /// Wait for SSH (Linux) or WinRM (Windows) to become reachable.
    /// Mounts the project directory inside the guest if requested.
    pub fn wait_for_ready(&mut self, timeout: Duration) -> Result<()> {
        let start = Instant::now();

        println!("[{}] Waiting for {} connectivity on port {} (timeout: {:?})...",
            &self.profile_name,
            if self.profile.os == GuestOs::Linux || self.profile.os == GuestOs::MacOS { "SSH" } else { "WinRM" },
            self.ssh_port,
            timeout);

        loop {
            if start.elapsed() > timeout {
                return Err(foundation_testbed::config::TestbedError::SshFailed {
                    port: self.ssh_port,
                    source: anyhow::anyhow!("timed out waiting for VM connectivity after {:?}", timeout),
                });
            }

            match self.profile.os {
                GuestOs::Linux | GuestOs::MacOS => {
                    if ssh::connect_from_port(self.ssh_port, self.profile.user, self.profile.os).is_ok() {
                        println!("[{}] SSH ready", &self.profile_name);
                        if self.mount_project {
                            self.mount_project_in_guest()?;
                        }
                        return Ok(());
                    }
                }
                GuestOs::Windows => {
                    // For Windows, try SSH (OpenSSH should be available after bootstrap)
                    if ssh::connect_from_port(self.ssh_port, self.profile.user, self.profile.os).is_ok() {
                        println!("[{}] SSH ready (Windows)", &self.profile_name);
                        if self.mount_project {
                            self.mount_project_in_guest()?;
                        }
                        return Ok(());
                    }
                }
            }

            std::thread::sleep(Duration::from_secs(5));
        }
    }

    /// Mount the project directory inside the guest via 9p/virtio.
    /// Linux only — Windows guests need virtio-win drivers for 9p support.
    fn mount_project_in_guest(&mut self) -> Result<()> {
        if self.profile.os == GuestOs::Windows {
            println!("[{}] Skipping 9p mount (Windows requires virtio drivers)", &self.profile_name);
            return Ok(());
        }
        println!("[{}] Mounting project directory in guest...", &self.profile_name);
        let mount_cmd = mount::guest_mount_command(mount::DEFAULT_TAG, mount::DEFAULT_GUEST_PATH);
        let output = self.ssh_exec(&mount_cmd)?;
        println!("[{}] Mount result: {}", &self.profile_name, output.trim());
        Ok(())
    }

    /// Execute a command via SSH on the VM.
    pub fn ssh_exec(&self, cmd: &str) -> Result<String> {
        let mut session = ssh::connect_from_port(self.ssh_port, self.profile.user, self.profile.os)?;
        ssh::exec(&mut session, cmd)
    }

    /// Execute a PowerShell script via SSH on the VM (CLIXML-stripped).
    pub fn ps_exec(&self, script: &str) -> Result<(String, i32)> {
        let mut session = ssh::connect_from_port(self.ssh_port, self.profile.user, self.profile.os)?;
        ssh::exec_ps_windows(&mut session, script)
    }

    /// Get a reference to the QemuVm for advanced operations.
    pub fn qemu(&self) -> Option<&QemuVm> {
        self.qemu.as_ref()
    }

    /// Get the profile name.
    pub fn profile_name(&self) -> &str {
        &self.profile_name
    }

    /// Get the resolved SSH port.
    pub fn ssh_port(&self) -> u16 {
        self.ssh_port
    }
}

impl Drop for TestVm {
    fn drop(&mut self) {
        if let Some(mut q) = self.qemu.take() {
            println!("[{}] Stopping VM...", self.profile_name);
            let _ = q.shutdown();
        }
        // Clean up state file
        let _ = state::delete(&self.profile_name);
        println!("[{}] VM stopped", self.profile_name);
    }
}

/// Ensure a VM image is cached (downloads if needed).
pub fn ensure_image(profile_name: &str) -> Result<PathBuf> {
    let profile = get_profile(profile_name).map(|p| p.clone())?;
    foundation_testbed::import::ensure_image(&profile)
}

/// Create a minimal Tauri project at the given directory.
/// No network access required — all files are written inline.
pub fn create_tauri_project(dir: &Path) -> Result<()> {
    use foundation_testbed::config::TestbedError;
    let io_err = |msg: &str| -> TestbedError { TestbedError::Qcow2Error { message: msg.to_string() } };

    std::fs::create_dir_all(dir.join("src")).map_err(|e| io_err(&format!("creating src: {e}")))?;
    std::fs::create_dir_all(dir.join("dist")).map_err(|e| io_err(&format!("creating dist: {e}")))?;

    std::fs::write(dir.join("Cargo.toml"), r#"[package]
name = "tauri-e2e-test"
version = "0.1.0"
edition = "2021"

[dependencies]
tauri = "2"

[build-dependencies]
tauri-build = "2"
"#).map_err(|e| io_err(&format!("writing Cargo.toml: {e}")))?;

    std::fs::write(dir.join("build.rs"), "fn main() { tauri_build::build() }")
        .map_err(|e| io_err(&format!("writing build.rs: {e}")))?;

    std::fs::write(dir.join("src/main.rs"), r#"#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
"#).map_err(|e| io_err(&format!("writing main.rs: {e}")))?;

    std::fs::write(dir.join("tauri.conf.json"), r#"{
  "productName": "tauri-e2e-test",
  "version": "0.1.0",
  "identifier": "com.testbed.e2e",
  "build": {
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": false
  },
  "bundle": {}
}
"#).map_err(|e| io_err(&format!("writing tauri.conf.json: {e}")))?;

    std::fs::write(dir.join("dist/index.html"), r#"<!DOCTYPE html>
<html>
<head><title>E2E Test</title></head>
<body><h1>Tauri E2E Test App</h1></body>
</html>
"#).map_err(|e| io_err(&format!("writing index.html: {e}")))?;

    Ok(())
}

/// Assert that a build completed successfully (exit code 0, artifact exists).
pub fn assert_build_ok_linux(vm: &TestVm, project_path: &str) -> Result<bool> {
    let output = vm.ssh_exec(&format!(
        "test -f {project_path}/target/x86_64-unknown-linux-gnu/release/tauri-e2e-test && echo OK || echo MISSING"
    ))?;
    Ok(output.trim() == "OK")
}

/// Assert that a Windows build completed successfully.
pub fn assert_build_ok_windows(vm: &TestVm, project_path: &str) -> Result<bool> {
    let (output, _) = vm.ps_exec(&format!(
        "if (Test-Path '{project_path}\\target\\x86_64-pc-windows-msvc\\release\\tauri-e2e-test.exe') {{ 'OK' }} else {{ 'MISSING' }}"
    ))?;
    Ok(output.trim() == "OK")
}

/// Validate that a file is an ELF binary (Linux executable).
pub fn assert_elf_binary(path_on_host: &Path) -> Result<bool> {
    use foundation_testbed::config::TestbedError;
    if !path_on_host.exists() {
        return Ok(false);
    }
    let mut magic = [0u8; 4];
    let mut file = std::fs::File::open(path_on_host)
        .map_err(|e| TestbedError::Qcow2Error { message: format!("opening file: {e}") })?;
    std::io::Read::read_exact(&mut file, &mut magic)
        .map_err(|e| TestbedError::Qcow2Error { message: format!("reading file: {e}") })?;
    Ok(&magic == b"\x7fELF")
}

/// Validate that a file is a PE binary (Windows .exe with MZ header).
pub fn assert_pe_binary(path_on_host: &Path) -> Result<bool> {
    use foundation_testbed::config::TestbedError;
    if !path_on_host.exists() {
        return Ok(false);
    }
    let mut magic = [0u8; 2];
    let mut file = std::fs::File::open(path_on_host)
        .map_err(|e| TestbedError::Qcow2Error { message: format!("opening file: {e}") })?;
    std::io::Read::read_exact(&mut file, &mut magic)
        .map_err(|e| TestbedError::Qcow2Error { message: format!("reading file: {e}") })?;
    Ok(&magic == b"MZ")
}
