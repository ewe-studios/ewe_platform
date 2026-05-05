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
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Return the absolute path to the example Tauri app on the host.
fn tauri_app_host_path() -> PathBuf {
    project_dir().join(TAURI_APP)
}

/// Path to the example Tauri app inside the guest, relative to the mount point.
const TAURI_APP_GUEST: &str = "tauri-app";

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

    // Verify the mount directory and example Tauri app are visible
    let (output, exit) = vm.ps_exec(&format!(
        "if (Test-Path '{WINDOWS_MOUNT}\\{TAURI_APP_GUEST}\\Cargo.toml') {{ 'FOUND' }} else {{ 'MISSING' }}"
    ))?;
    assert_eq!(exit, 0, "path check should succeed");
    assert!(
        output.contains("FOUND"),
        "example Tauri app should be visible through project mount"
    );

    println!("[mount/windows] Project mount verified, example app visible");
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

    println!("[build/linux] Building Tauri app in VM...");
    let build_cmd = format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && \
         export DISPLAY=:99 && \
         cargo tauri build 2>&1 | tail -20"
    );
    let output = vm.ssh_exec(&build_cmd)?;
    println!("  Build output:\n{}", output);

    let artifact_on_host = tauri_app_host_path()
        .join("target/x86_64-unknown-linux-gnu/release/tauri-e2e-test");

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

    println!("[headless/linux] Building Tauri app...");
    let build_output = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/{TAURI_APP_GUEST} && cargo tauri build 2>&1 | tail -5"
    ))?;
    println!("  Build output:\n{}", build_output);

    println!("[headless/linux] Launching app on Xvfb :99...");
    let launch_script = format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/{TAURI_APP_GUEST}/target/x86_64-unknown-linux-gnu/release/tauri-e2e-test & \
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
        .join("target/x86_64-unknown-linux-gnu/release/tauri-e2e-test");
    let built = assert_build_ok_linux(&vm, &format!("{LINUX_MOUNT}/{TAURI_APP_GUEST}"))?;
    assert!(built, "build artifact should exist");
    let is_elf = assert_elf_binary(&artifact_on_host)?;
    assert!(is_elf, "artifact should be ELF binary");

    println!("[e2e/linux] Launching app on Xvfb :99...");
    let launch_output = vm.ssh_exec(&format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/{TAURI_APP_GUEST}/target/x86_64-unknown-linux-gnu/release/tauri-e2e-test & \
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
