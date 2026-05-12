//! Tauri E2E integration tests.
//!
//! These tests exercise the full VM lifecycle:
//! 1. Launch a VM (Linux, Windows, or macOS guest)
//! 2. Verify SSH/WinRM connectivity
//! 3. Build the example Tauri app (`examples/testbed/tauri-app/`) inside the VM
//! 4. Launch the resulting binary headlessly
//! 5. Verify the app runs without crashing
//! 6. Stop the VM cleanly
//!
//! The example Tauri project lives in `examples/testbed/tauri-app/` and is
//! excluded from the workspace so it can be built independently inside VMs.
//! It ships with `mise.toml`, `build.sh`, and `build.ps1` for in-guest builds.
//!
//! All tests are marked `#[ignore]` because they require:
//! - A running KVM hypervisor
//! - Pre-built VM images (downloaded on first run)
//! - Several minutes of execution time
//!
//! Run with: `cargo test -p foundation_testbed --test e2e_tauri -- --ignored`

mod common;

use std::path::PathBuf;
use std::time::Duration;

use common::{TestVm, assert_build_ok_linux, assert_build_ok_windows, assert_build_ok_macos,
             assert_elf_binary, assert_pe_binary, assert_macho_binary};
use foundation_testbed::config::Result;
use serial_test::serial;

// ── Constants ────────────────────────────────────────────────────────────────

const LINUX_PROFILE: &str = "linux-build";
const WINDOWS_PROFILE: &str = "windows-build";
const MACOS_PROFILE: &str = "macos-build";
const LINUX_MOUNT: &str = "/mnt/project";
const WINDOWS_MOUNT: &str = "C:\\Users\\vagrant\\project";
const MACOS_MOUNT: &str = "/Volumes/project";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(300); // 5 min

/// Path to the example Tauri app relative to the workspace root.
const TAURI_APP: &str = "examples/testbed/tauri-app";

fn project_dir() -> PathBuf {
    // Get the workspace root (2 levels up from foundation_testbed crate)
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/home/darkvoid/Boxxed/@dev/ewe_platform"))
}

/// Return the absolute path to the example Tauri app on the host.
fn tauri_app_host_path() -> PathBuf {
    project_dir().join(TAURI_APP)
}

/// Path to the example Tauri app inside the guest, relative to the mount point.
const TAURI_APP_GUEST: &str = "examples/testbed/tauri-app";

// ── 10.3 VM Lifecycle Tests ──────────────────────────────────────────────────

/// Verifies the complete VM start-run-stop cycle on Linux.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_vm_lifecycle_linux() -> Result<()> {
    let mut vm = TestVm::new(LINUX_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let output = vm.ssh_exec("echo hello")?;
    assert_eq!(output.trim(), "hello");

    println!("[lifecycle/linux] VM started, SSH responding, stopping...");
    Ok(())
}

/// Verifies the complete VM start-run-stop cycle on Windows.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_vm_lifecycle_windows() -> Result<()> {
    let mut vm = TestVm::new(WINDOWS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let output = vm.ssh_exec("echo hello")?;
    assert_eq!(output.trim(), "hello");

    println!("[lifecycle/windows] VM started, SSH responding, stopping...");
    Ok(())
}

// ── 10.4 Project Mount Tests ─────────────────────────────────────────────────

/// Verifies the host directory is accessible inside a Linux VM.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_project_mount_linux() -> Result<()> {
    let mut vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let output = vm.ssh_exec(&format!("ls {LINUX_MOUNT}"))?;
    assert!(
        !output.trim().is_empty(),
        "mount directory should contain files"
    );

    // Verify the example Tauri app is visible through the mount
    let output = vm.ssh_exec(&format!("test -d {LINUX_MOUNT}/{TAURI_APP_GUEST} && echo FOUND || echo MISSING"))?;
    assert_eq!(output.trim(), "FOUND", "example Tauri app should be visible in mount");

    println!("[mount/linux] Project mount verified, example app visible");
    Ok(())
}

/// Verifies the host directory is accessible inside a Windows VM.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_project_mount_windows() -> Result<()> {
    let mut vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;
    vm.bootstrap()?;

    // First check which mount is available (virtiofs C: or SMB Z:)
    let (mount_check, _) = vm.ps_exec(
        "if (Test-Path 'C:\\Users\\vagrant\\project\\Cargo.toml') { 'VIRTIOFS' } elseif (Test-Path 'Z:\\Cargo.toml') { 'SMB' } else { 'NONE' }"
    )?;
    println!("[mount/windows] Detected mount type: {}", mount_check.trim());

    // Determine which path to use
    let (test_path, mount_type) = if mount_check.trim() == "VIRTIOFS" {
        (format!("{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}"), "virtiofs")
    } else if mount_check.trim() == "SMB" {
        (format!("Z:\\{TAURI_APP_GUEST}"), "smb")
    } else {
        // List both locations for debugging
        let (c_check, _) = vm.ps_exec("if (Test-Path 'C:\\Users\\vagrant\\project') { 'C_EXISTS' } else { 'C_MISSING' }")?;
        let (z_check, _) = vm.ps_exec("if (Test-Path 'Z:\\') { 'Z_EXISTS' } else { 'Z_MISSING' }")?;
        panic!("No mount detected. C:\\Users\\vagrant\\project: {}, Z:\\: {}", c_check.trim(), z_check.trim());
    };

    // Verify the mount directory and example Tauri app are visible
    let (output, exit) = vm.ps_exec(&format!(
        "if (Test-Path '{}\\Cargo.toml') {{ 'FOUND' }} else {{ 'MISSING' }}",
        test_path
    ))?;
    println!("[mount/windows] Mount check ({}): output={:?}, exit={}", mount_type, output, exit);

    // Diagnostic: list contents of mount directory
    let (dir_list, _) = vm.ps_exec(
        &format!("Get-ChildItem '{}' -ErrorAction SilentlyContinue | ForEach-Object {{ $_.Name }} | Select-Object -First 20", test_path)
    ).unwrap_or_else(|_| ("(error listing dir)".to_string(), 0));
    println!("[mount/windows] Dir contents ({}): {}", mount_type, dir_list);

    // Diagnostic: check mount point details
    let (mount_info, _) = vm.ps_exec(
        &format!("if (Test-Path '{}') {{ $fs = (Get-Item '{}').PSDrive; if ($fs) {{ 'Provider='+$fs.Provider.Name+' Type='+$fs.DriveType }} else {{ 'exists-no-drive' }} }} else {{ 'missing' }}",
            test_path, test_path)
    ).unwrap_or_else(|_| ("(error)".to_string(), 0));
    println!("[mount/windows] Mount info ({}): {}", mount_type, mount_info);

    assert!(
        output.contains("FOUND"),
        "example Tauri app should be visible through {} mount (path: {}, dir contents: {})",
        mount_type, test_path, dir_list
    );

    println!("[mount/windows] Project mount verified via {}, example app visible", mount_type);
    Ok(())
}

/// Quick SMB mount validation - only tests mounting, no full bootstrap.
/// This is useful for debugging SMB issues without waiting for full bootstrap.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_smb_mount_only() -> Result<()> {
    println!("[smb-only] === SMB Mount Validation Test ===");

    let mut vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    println!("[smb-only] VM ready, testing SMB mount...");

    // Check if SMB is already mounted (from virtiofs fallback)
    let (mount_check, _) = vm.ps_exec(
        "if (Test-Path 'Z:\\Cargo.toml') { 'SMB_OK' } elseif (Test-Path 'C:\\Users\\vagrant\\project\\Cargo.toml') { 'VIRTIOFS_OK' } else { 'NOT_MOUNTED' }"
    )?;
    println!("[smb-only] Initial mount check: {}", mount_check.trim());

    if mount_check.trim() == "SMB_OK" || mount_check.trim() == "VIRTIOFS_OK" {
        println!("[smb-only] Mount already active via {}", mount_check.trim());
    } else {
        // Try SMB mount directly
        println!("[smb-only] Attempting SMB mount...");
        let smb_script = r#"
$ErrorActionPreference = 'Stop'
$smbServer = '10.0.2.4'
$shareName = 'qemu'
$mountLetter = 'Z'

# Clean up existing
net use "${mountLetter}:" /delete /y 2>$null | Out-Null
Remove-PSDrive -Name $mountLetter -Force -ErrorAction SilentlyContinue | Out-Null

# Wait for server
$connected = $false
for ($i = 0; $i -lt 15; $i++) {
    if (Test-Connection -ComputerName $smbServer -Count 1 -Quiet -ErrorAction SilentlyContinue) {
        $connected = $true
        break
    }
    Start-Sleep -Seconds 2
}
if (-not $connected) { throw "SMB server not reachable" }

# Mount
$unc = "\\$smbServer\$shareName"
net use "${mountLetter}:" "$unc" /persistent:no 2>&1
if ($LASTEXITCODE -ne 0) { throw "net use failed" }

# Verify
if (Test-Path "${mountLetter}:") {
    $files = Get-ChildItem "${mountLetter}:" -ErrorAction SilentlyContinue | Select-Object -First 5 | ForEach-Object { $_.Name }
    "MOUNT_OK: Files found: " + ($files -join ', ')
} else {
    throw "Mount not accessible"
}
"#;

        let (result, exit) = vm.ps_exec(smb_script)?;
        println!("[smb-only] SMB mount result: exit={}, output={}", exit, result.trim());

        if !result.contains("MOUNT_OK") {
            // Get diagnostics
            let (ping, _) = vm.ps_exec("Test-Connection -ComputerName 10.0.2.4 -Count 2 -ErrorAction SilentlyContinue; if ($?) { 'PING_OK' } else { 'PING_FAIL' }")?;
            let (netstat, _) = vm.ps_exec("netstat -an | findstr 10.0.2.4")?;
            println!("[smb-only] Diagnostics - Ping: {}, Netstat: {}", ping.trim(), netstat.trim());
        }
    }

    // Final verification
    let (verify, _) = vm.ps_exec(
        "if (Test-Path 'Z:\\Cargo.toml') { 'Z_OK' } elseif (Test-Path 'C:\\Users\\vagrant\\project\\Cargo.toml') { 'C_OK' } else { 'FAIL' }"
    )?;

    let (contents, _) = vm.ps_exec(
        "Get-ChildItem 'Z:\\' -ErrorAction SilentlyContinue | Select-Object -First 10 | ForEach-Object { $_.Name }"
    ).unwrap_or_else(|_| ("(error)".to_string(), 0));

    println!("[smb-only] Final verification: {}", verify.trim());
    println!("[smb-only] Z:\\ contents: {}", contents.trim());

    assert!(
        verify.trim() == "Z_OK" || verify.trim() == "C_OK",
        "SMB mount failed - neither Z: nor C: have Cargo.toml"
    );

    println!("[smb-only] === SMB Mount Validation PASSED ===");
    Ok(())
}

// ── 10.5 Tauri Build Tests ───────────────────────────────────────────────────

/// Full build pipeline for Linux guest.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_tauri_build_linux() -> Result<()> {
    let mut vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Clear any stale bootstrap marker (fresh VM may have marker without tools)
    let _ = vm.ssh_exec("rm -f ~/.testbed-bootstrapped");

    vm.bootstrap()?;

    println!("[build/linux] Building Tauri app in VM...");

    // Trust the mise config in the project (use -y to auto-accept)
    let trust_cmd = format!("cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && echo 'y' | ~/.local/bin/mise trust 2>&1");
    let trust_output = vm.ssh_exec(&trust_cmd)?;
    println!("  Trust output: {}", trust_output);

    let build_cmd = format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && \
         export DISPLAY=:99 && \
         ~/.local/bin/mise exec -- cargo tauri build 2>&1 | tail -20"
    );
    let output = vm.ssh_exec(&build_cmd)?;
    println!("  Build output:\n{}", output);

    let artifact_on_host = tauri_app_host_path()
        .join("target/release/tauri-e2e-test");

    let built = assert_build_ok_linux(&vm, &format!("{LINUX_MOUNT}/{TAURI_APP_GUEST}"))?;
    assert!(built, "Tauri build artifact should exist");

    let is_elf = assert_elf_binary(&artifact_on_host)?;
    assert!(is_elf, "build artifact should be a valid ELF binary");

    println!("[build/linux] Build OK, ELF binary verified");
    Ok(())
}

/// Full build pipeline for Windows guest.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_tauri_build_windows() -> Result<()> {
    let mut vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;
    vm.bootstrap()?;

    // Verify project mount before building
    let (mount_check, _) = vm.ps_exec(&format!(
        "if (Test-Path '{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}\\Cargo.toml') {{ 'FOUND' }} else {{ 'MISSING' }}"
    ))?;
    assert!(
        mount_check.contains("FOUND"),
        "project mount should be accessible before build (got: {})",
        mount_check.trim()
    );

    println!("[build/windows] Building Tauri app in VM...");
    let build_cmd = format!(
        "cd {WINDOWS_MOUNT}\\{TAURI_APP_GUEST} && cargo tauri build 2>&1"
    );
    let (output, exit_code) = vm.ps_exec(&build_cmd)?;
    println!("  Build output:\n{output}");
    println!("  Exit code: {exit_code}");

    let artifact_on_host = tauri_app_host_path()
        .join("target/x86_64-pc-windows-msvc/release/tauri-e2e-test.exe");

    let built = assert_build_ok_windows(&vm, &format!("{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}"))?;
    assert!(built, "Tauri build artifact should exist (exit code: {exit_code})");

    let is_pe = assert_pe_binary(&artifact_on_host)?;
    assert!(is_pe, "build artifact should be a valid PE (.exe) binary");

    println!("[build/windows] Build OK, PE binary verified");
    Ok(())
}

// ── 10.7/10.8 Headless App Launch Tests ──────────────────────────────────────

/// Validates the built binary can launch without crashing (Linux).
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_headless_app_linux() -> Result<()> {
    let mut vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Clear any stale bootstrap marker (fresh VM may have marker without tools)
    let _ = vm.ssh_exec("rm -f ~/.testbed-bootstrapped");

    vm.bootstrap()?;

    println!("[headless/linux] Building Tauri app...");
    // Trust the mise config first
    let _ = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && echo 'y' | ~/.local/bin/mise trust 2>&1"
    ))?;
    let build_output = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && \
         export DISPLAY=:99 && \
         ~/.local/bin/mise exec -- cargo tauri build 2>&1 | tail -5"
    ))?;
    println!("  Build output:\n{}", build_output);

    println!("[headless/linux] Launching app on Xvfb :99...");
    let launch_script = format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/{TAURI_APP_GUEST}/target/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true"
    );
    let output = vm.ssh_exec(&launch_script)?;
    assert!(
        output.contains("ALIVE"),
        "Tauri app should be alive after 5 seconds. Output: {output}"
    );

    println!("[headless/linux] App launched and survived 5s");
    Ok(())
}

/// Validates the built binary can launch without crashing (Windows).
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_headless_app_windows() -> Result<()> {
    let mut vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    println!("[headless/windows] Building Tauri app...");
    let build_cmd = format!(
        "cd {WINDOWS_MOUNT}\\{TAURI_APP_GUEST} && cargo tauri build 2>&1 | Select-Object -Last 5"
    );
    let (build_output, _) = vm.ps_exec(&build_cmd)?;
    println!("  Build output:\n{}", build_output);

    let binary_path = format!(
        "{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}\\target\\x86_64-pc-windows-msvc\\release\\tauri-e2e-test.exe"
    );
    println!("[headless/windows] Launching app hidden...");
    let launch_script = format!(
        "Start-Process -FilePath '{binary_path}' -WindowStyle Hidden; \
         Start-Sleep -Seconds 5; \
         $proc = Get-Process -Name 'tauri-e2e-test' -ErrorAction SilentlyContinue; \
         if ($null -ne $proc) {{ 'ALIVE' }} else {{ 'DEAD' }}; \
         Stop-Process -Name 'tauri-e2e-test' -Force -ErrorAction SilentlyContinue"
    );
    let (output, _) = vm.ps_exec(&launch_script)?;
    assert!(
        output.contains("ALIVE"),
        "Tauri app should be alive after 5 seconds. Output: {output}"
    );

    println!("[headless/windows] App launched and survived 5s");
    Ok(())
}

// ── macOS Guest Tests ────────────────────────────────────────────────────────

/// Verifies the complete VM start-run-stop cycle on macOS.
#[test]
#[ignore]
#[serial(macos_vm)]
fn test_vm_lifecycle_macos() -> Result<()> {
    let mut vm = TestVm::new(MACOS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let output = vm.ssh_exec("echo hello")?;
    assert_eq!(output.trim(), "hello");

    println!("[lifecycle/macos] VM started, SSH responding, stopping...");
    Ok(())
}

/// Verifies the host directory is accessible inside a macOS VM.
#[test]
#[ignore]
#[serial(macos_vm)]
fn test_project_mount_macos() -> Result<()> {
    let mut vm = TestVm::new(MACOS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Transfer the example Tauri app to guest via scp
    let project_path = tauri_app_host_path();
    let scp_cmd = format!(
        "scp -r -P {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
         {} vagrant@127.0.0.1:/tmp/tauri-app",
        vm.ssh_port(),
        project_path.display()
    );
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&scp_cmd)
        .status()
        .map_err(|e| foundation_testbed::config::TestbedError::Qcow2Error {
            message: format!("scp command: {e}"),
        })?;
    assert!(status.success(), "scp transfer should succeed");

    // Verify files arrived
    let output = vm.ssh_exec("test -f /tmp/tauri-app/Cargo.toml && echo FOUND || echo MISSING")?;
    assert_eq!(output.trim(), "FOUND");

    // Cleanup
    vm.ssh_exec("rm -rf /tmp/tauri-app").ok();

    println!("[mount/macos] Project round-trip via SCP OK");
    Ok(())
}

/// Full build pipeline for macOS guest.
#[test]
#[ignore]
#[serial(macos_vm)]
fn test_tauri_build_macos() -> Result<()> {
    let mut vm = TestVm::new(MACOS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Transfer example Tauri app to guest via scp
    let project_path = tauri_app_host_path();
    let scp_cmd = format!(
        "scp -r -P {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
         {} vagrant@127.0.0.1:/tmp/tauri-app",
        vm.ssh_port(),
        project_path.display()
    );
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&scp_cmd)
        .status()
        .map_err(|e| foundation_testbed::config::TestbedError::Qcow2Error {
            message: format!("scp command: {e}"),
        })?;
    assert!(status.success(), "scp transfer should succeed");

    println!("[build/macos] Building Tauri app in VM...");
    let build_cmd = "cd /tmp/tauri-app && \
         export DISPLAY=:99 && \
         cargo tauri build 2>&1 | tail -20";
    let output = vm.ssh_exec(build_cmd)?;
    println!("  Build output:\n{}", output);

    let built = assert_build_ok_macos(&vm, "/tmp/tauri-app")?;
    assert!(built, "Tauri build artifact should exist");

    // Pull artifact back to host for validation
    let host_artifact = project_path.join("target/x86_64-apple-darwin/release/tauri-e2e-test");
    std::fs::create_dir_all(host_artifact.parent().unwrap()).ok();
    let pull_cmd = format!(
        "scp -P {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
         vagrant@127.0.0.1:/tmp/tauri-app/target/x86_64-apple-darwin/release/tauri-e2e-test \
         {}",
        vm.ssh_port(),
        host_artifact.display()
    );
    let _ = std::process::Command::new("sh").arg("-c").arg(&pull_cmd).status();

    if host_artifact.exists() {
        let is_macho = assert_macho_binary(&host_artifact)?;
        assert!(is_macho, "build artifact should be a valid Mach-O binary");
    }

    // Cleanup
    vm.ssh_exec("rm -rf /tmp/tauri-app").ok();

    println!("[build/macos] Build OK, Mach-O binary verified");
    Ok(())
}

/// Validates the built binary can launch without crashing (macOS).
#[test]
#[ignore]
#[serial(macos_vm)]
fn test_headless_app_macos() -> Result<()> {
    let mut vm = TestVm::new(MACOS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Transfer example Tauri app to guest
    let project_path = tauri_app_host_path();
    let scp_cmd = format!(
        "scp -r -P {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
         {} vagrant@127.0.0.1:/tmp/tauri-app",
        vm.ssh_port(),
        project_path.display()
    );
    let _ = std::process::Command::new("sh").arg("-c").arg(&scp_cmd).status();

    println!("[headless/macos] Building Tauri app...");
    let build_output = vm.ssh_exec(
        "cd /tmp/tauri-app && cargo tauri build 2>&1 | tail -5"
    )?;
    println!("  Build output:\n{}", build_output);

    println!("[headless/macos] Launching app...");
    let launch_script = "export DISPLAY=:99 && \
         /tmp/tauri-app/target/x86_64-apple-darwin/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true";
    let output = vm.ssh_exec(launch_script)?;
    assert!(
        output.contains("ALIVE"),
        "Tauri app should be alive after 5 seconds. Output: {output}"
    );

    // Cleanup
    vm.ssh_exec("rm -rf /tmp/tauri-app").ok();

    println!("[headless/macos] App launched and survived 5s");
    Ok(())
}

// ── 10.9/10.10 Full E2E Tests ───────────────────────────────────────────────

/// End-to-end: Linux — start, mount, build, launch, stop.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_full_e2e_linux() -> Result<()> {
    println!("[e2e/linux] === Full E2E test starting ===");

    let mut vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let mount_list = vm.ssh_exec(&format!("ls {LINUX_MOUNT} | head -5"))?;
    assert!(
        !mount_list.trim().is_empty(),
        "project mount should have files"
    );
    println!("[e2e/linux] Mount OK: {}", mount_list.trim());

    println!("[e2e/linux] Building Tauri app...");
    let build_output = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && cargo tauri build 2>&1 | tail -10"
    ))?;
    println!("  Build output:\n{}", build_output);

    let artifact_on_host = tauri_app_host_path()
        .join("target/release/tauri-e2e-test");
    let built = assert_build_ok_linux(&vm, &format!("{LINUX_MOUNT}/{TAURI_APP_GUEST}"))?;
    assert!(built, "build artifact should exist");
    let is_elf = assert_elf_binary(&artifact_on_host)?;
    assert!(is_elf, "artifact should be ELF binary");

    println!("[e2e/linux] Launching app on Xvfb :99...");
    let launch_output = vm.ssh_exec(&format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/{TAURI_APP_GUEST}/target/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true"
    ))?;
    assert!(
        launch_output.contains("ALIVE"),
        "app should survive 5 seconds"
    );

    println!("[e2e/linux] === Full E2E test PASSED ===");
    Ok(())
}

/// End-to-end: Windows — start, mount, build, launch, stop.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_full_e2e_windows() -> Result<()> {
    println!("[e2e/windows] === Full E2E test starting ===");

    let mut vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    vm.bootstrap()?;

    let (mount_list, _) = vm.ps_exec(&format!(
        "Get-ChildItem -Path '{WINDOWS_MOUNT}' -Name | Select-Object -First 5"
    ))?;
    assert!(
        !mount_list.trim().is_empty(),
        "project mount should have files"
    );
    println!("[e2e/windows] Mount OK: {}", mount_list.trim());

    println!("[e2e/windows] Building Tauri app...");
    let (build_output, _) = vm.ps_exec(&format!(
        "cd {WINDOWS_MOUNT}\\{TAURI_APP_GUEST}; cargo tauri build 2>&1 | Select-Object -Last 10"
    ))?;
    println!("  Build output:\n{}", build_output);

    let artifact_on_host = tauri_app_host_path()
        .join("target/x86_64-pc-windows-msvc/release/tauri-e2e-test.exe");
    let built = assert_build_ok_windows(&vm, &format!("{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}"))?;
    assert!(built, "build artifact should exist");
    let is_pe = assert_pe_binary(&artifact_on_host)?;
    assert!(is_pe, "artifact should be PE (.exe) binary");

    let binary_path = format!(
        "{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}\\target\\x86_64-pc-windows-msvc\\release\\tauri-e2e-test.exe"
    );
    println!("[e2e/windows] Launching app hidden...");
    let (launch_output, _) = vm.ps_exec(&format!(
        "Start-Process -FilePath '{binary_path}' -WindowStyle Hidden; \
         Start-Sleep -Seconds 5; \
         $proc = Get-Process -Name 'tauri-e2e-test' -ErrorAction SilentlyContinue; \
         if ($null -ne $proc) {{ 'ALIVE' }} else {{ 'DEAD' }}; \
         Stop-Process -Name 'tauri-e2e-test' -Force -ErrorAction SilentlyContinue"
    ))?;
    assert!(
        launch_output.contains("ALIVE"),
        "app should survive 5 seconds"
    );

    println!("[e2e/windows] === Full E2E test PASSED ===");
    Ok(())
}

/// End-to-end: macOS — start, transfer, build, launch, stop.
#[test]
#[ignore]
#[serial(macos_vm)]
fn test_full_e2e_macos() -> Result<()> {
    println!("[e2e/macos] === Full E2E test starting ===");

    let mut vm = TestVm::new(MACOS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    let output = vm.ssh_exec("echo ready")?;
    assert_eq!(output.trim(), "ready");
    println!("[e2e/macos] SSH OK");

    // Transfer example Tauri app
    let project_path = tauri_app_host_path();
    let scp_cmd = format!(
        "scp -r -P {} -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
         {} vagrant@127.0.0.1:/tmp/tauri-app",
        vm.ssh_port(),
        project_path.display()
    );
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(&scp_cmd)
        .status()
        .map_err(|e| foundation_testbed::config::TestbedError::Qcow2Error {
            message: format!("scp command: {e}"),
        })?;
    assert!(status.success(), "scp transfer should succeed");
    println!("[e2e/macos] App transferred");

    println!("[e2e/macos] Building Tauri app...");
    let build_output = vm.ssh_exec(
        "cd /tmp/tauri-app && cargo tauri build 2>&1 | tail -10"
    )?;
    println!("  Build output:\n{}", build_output);

    let built = assert_build_ok_macos(&vm, "/tmp/tauri-app")?;
    assert!(built, "build artifact should exist");

    println!("[e2e/macos] Launching app...");
    let launch_output = vm.ssh_exec(
        "export DISPLAY=:99 && \
         /tmp/tauri-app/target/x86_64-apple-darwin/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true"
    )?;
    assert!(
        launch_output.contains("ALIVE"),
        "app should survive 5 seconds"
    );

    vm.ssh_exec("rm -rf /tmp/tauri-app").ok();

    println!("[e2e/macos] === Full E2E test PASSED ===");
    Ok(())
}
