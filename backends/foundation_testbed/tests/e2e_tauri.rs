//! Tauri E2E integration tests.
//!
//! These tests exercise the full VM lifecycle:
//! 1. Launch a VM (Linux or Windows guest)
//! 2. Verify SSH/WinRM connectivity
//! 3. Create a minimal Tauri project on the mounted filesystem
//! 4. Build the Tauri app inside the VM
//! 5. Launch the resulting binary headlessly
//! 6. Verify the app runs without crashing
//! 7. Stop the VM cleanly
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

use common::{TestVm, assert_build_ok_linux, assert_build_ok_windows,
             assert_elf_binary, assert_pe_binary, create_tauri_project};
use foundation_testbed::config::Result;
use serial_test::serial;

// ── Constants ────────────────────────────────────────────────────────────────

const LINUX_PROFILE: &str = "linux-build";
const WINDOWS_PROFILE: &str = "windows-build";
const LINUX_MOUNT: &str = "/mnt/project";
const WINDOWS_MOUNT: &str = "C:\\Users\\vagrant\\project";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(300); // 5 min

fn project_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

// ── 10.3 VM Lifecycle Tests ──────────────────────────────────────────────────

/// Verifies the complete VM start-run-stop cycle on Linux.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_vm_lifecycle_linux() -> Result<()> {
    let vm = TestVm::new(LINUX_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Verify connectivity
    let output = vm.ssh_exec("echo hello")?;
    assert_eq!(output.trim(), "hello");

    println!("[lifecycle/linux] VM started, SSH responding, stopping...");
    // VM stops on drop
    Ok(())
}

/// Verifies the complete VM start-run-stop cycle on Windows.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_vm_lifecycle_windows() -> Result<()> {
    let vm = TestVm::new(WINDOWS_PROFILE)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Verify connectivity
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
    let vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Verify mount is accessible
    let output = vm.ssh_exec(&format!("ls {LINUX_MOUNT}"))?;
    assert!(
        !output.trim().is_empty(),
        "mount directory should contain files"
    );

    // Round-trip: create a file in the mount, verify on host
    let test_file = format!("{LINUX_MOUNT}/.mount-test-{pid}", pid = std::process::id());
    vm.ssh_exec(&format!("touch {test_file}"))?;
    let exists = vm.ssh_exec(&format!("test -f {test_file} && echo YES || echo NO"))?;
    assert_eq!(exists.trim(), "YES");

    // Verify on host side
    let host_path = project_dir().join(format!(".mount-test-{}", std::process::id()));
    assert!(host_path.exists(), "file created in mount should be visible on host");
    std::fs::remove_file(host_path).ok(); // cleanup

    // Clean up guest side too
    vm.ssh_exec(&format!("rm -f {test_file}")).ok();

    println!("[mount/linux] Project mount verified, round-trip OK");
    Ok(())
}

/// Verifies the host directory is accessible inside a Windows VM.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_project_mount_windows() -> Result<()> {
    let vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Verify mount is accessible
    let (output, _) = vm.ps_exec(&format!(
        "Get-ChildItem -Path '{WINDOWS_MOUNT}' -Name | Select-Object -First 3"
    ))?;
    assert!(
        !output.trim().is_empty(),
        "mount directory should contain files"
    );

    // Round-trip: create a file in the mount
    let test_file = format!("{WINDOWS_MOUNT}\\.mount-test-{}", std::process::id());
    let (_, exit) = vm.ps_exec(&format!(
        "New-Item -Path '{test_file}' -ItemType File -Force"
    ))?;
    assert_eq!(exit, 0, "file creation should succeed");

    // Verify file exists
    let (exists, _) = vm.ps_exec(&format!(
        "if (Test-Path '{test_file}') {{ 'YES' }} else {{ 'NO' }}"
    ))?;
    assert_eq!(exists.trim(), "YES");

    // Verify on host side
    let host_path = project_dir().join(format!(".mount-test-{}", std::process::id()));
    assert!(host_path.exists(), "file created in mount should be visible on host");
    std::fs::remove_file(host_path).ok();

    // Clean up
    vm.ps_exec(&format!("Remove-Item -Path '{test_file}' -Force")).ok();

    println!("[mount/windows] Project mount verified, round-trip OK");
    Ok(())
}

// ── 10.5 Tauri Build Tests ───────────────────────────────────────────────────

/// Full build pipeline for Linux guest.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_tauri_build_linux() -> Result<()> {
    let vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Create Tauri project in the mounted directory
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[build/linux] Building Tauri project in VM...");
    let build_cmd = format!(
        "cd {LINUX_MOUNT}/tauri-e2e-test && \
         export DISPLAY=:99 && \
         cargo tauri build 2>&1 | tail -20"
    );
    let output = vm.ssh_exec(&build_cmd)?;
    println!("  Build output:\n{}", output);

    // Verify artifact exists
    let artifact_on_host = project_path
        .join("target/x86_64-unknown-linux-gnu/release/tauri-e2e-test");

    let built = assert_build_ok_linux(&vm, &format!("{LINUX_MOUNT}/tauri-e2e-test"))?;
    assert!(built, "Tauri build artifact should exist");

    // Verify it's a valid ELF binary
    let is_elf = assert_elf_binary(&artifact_on_host)?;
    assert!(is_elf, "build artifact should be a valid ELF binary");

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[build/linux] Build OK, ELF binary verified");
    Ok(())
}

/// Full build pipeline for Windows guest.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_tauri_build_windows() -> Result<()> {
    let vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Create Tauri project in the mounted directory
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[build/windows] Building Tauri project in VM...");
    let build_cmd = format!(
        "cd {WINDOWS_MOUNT}\\tauri-e2e-test && cargo tauri build 2>&1"
    );
    let (output, _) = vm.ps_exec(&build_cmd)?;
    println!("  Build output:\n{}", output);

    // Verify artifact exists
    let artifact_on_host = project_path
        .join("target/x86_64-pc-windows-msvc/release/tauri-e2e-test.exe");

    let built = assert_build_ok_windows(&vm, &format!("{WINDOWS_MOUNT}\\tauri-e2e-test"))?;
    assert!(built, "Tauri build artifact should exist");

    // Verify it's a valid PE binary (MZ header)
    let is_pe = assert_pe_binary(&artifact_on_host)?;
    assert!(is_pe, "build artifact should be a valid PE (.exe) binary");

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[build/windows] Build OK, PE binary verified");
    Ok(())
}

// ── 10.7/10.8 Headless App Launch Tests ──────────────────────────────────────

/// Validates the built binary can launch without crashing (Linux).
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_headless_app_linux() -> Result<()> {
    let vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Create and build the Tauri project
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[headless/linux] Building Tauri project...");
    let build_output = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/tauri-e2e-test && cargo tauri build 2>&1 | tail -5"
    ))?;
    println!("  Build output:\n{}", build_output);

    // Launch the app on Xvfb display :99
    println!("[headless/linux] Launching app on Xvfb :99...");
    let launch_script = format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/tauri-e2e-test/target/x86_64-unknown-linux-gnu/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true"
    );
    let output = vm.ssh_exec(&launch_script)?;
    assert!(
        output.contains("ALIVE"),
        "Tauri app should be alive after 5 seconds. Output: {output}"
    );

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[headless/linux] App launched and survived 5s");
    Ok(())
}

/// Validates the built binary can launch without crashing (Windows).
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_headless_app_windows() -> Result<()> {
    let vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // Create and build the Tauri project
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[headless/windows] Building Tauri project...");
    let build_cmd = format!(
        "cd {WINDOWS_MOUNT}\\tauri-e2e-test && cargo tauri build 2>&1 | Select-Object -Last 5"
    );
    let (build_output, _) = vm.ps_exec(&build_cmd)?;
    println!("  Build output:\n{}", build_output);

    // Launch the app hidden
    let binary_path = format!(
        "{WINDOWS_MOUNT}\\tauri-e2e-test\\target\\x86_64-pc-windows-msvc\\release\\tauri-e2e-test.exe"
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

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[headless/windows] App launched and survived 5s");
    Ok(())
}

// ── 10.9/10.10 Full E2E Tests ───────────────────────────────────────────────

/// End-to-end: Linux — start, mount, build, launch, stop.
#[test]
#[ignore]
#[serial(linux_vm)]
fn test_full_e2e_linux() -> Result<()> {
    println!("[e2e/linux] === Full E2E test starting ===");

    // 1. Start Linux VM
    let vm = TestVm::new_with_mount(LINUX_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // 2. Verify SSH reachable (already done by wait_for_ready)

    // 3. Verify /mnt/project mount accessible
    let mount_list = vm.ssh_exec(&format!("ls {LINUX_MOUNT} | head -5"))?;
    assert!(
        !mount_list.trim().is_empty(),
        "project mount should have files"
    );
    println!("[e2e/linux] Mount OK: {}", mount_list.trim());

    // 4. Create and build Tauri project
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[e2e/linux] Building Tauri project...");
    let build_output = vm.ssh_exec(&format!(
        "cd {LINUX_MOUNT}/tauri-e2e-test && cargo tauri build 2>&1 | tail -10"
    ))?;
    println!("  Build output:\n{}", build_output);

    // 5. Verify build artifact
    let artifact_on_host = project_path
        .join("target/x86_64-unknown-linux-gnu/release/tauri-e2e-test");
    let built = assert_build_ok_linux(&vm, &format!("{LINUX_MOUNT}/tauri-e2e-test"))?;
    assert!(built, "build artifact should exist");
    let is_elf = assert_elf_binary(&artifact_on_host)?;
    assert!(is_elf, "artifact should be ELF binary");

    // 6. Launch binary headlessly
    println!("[e2e/linux] Launching app on Xvfb :99...");
    let launch_output = vm.ssh_exec(&format!(
        "export DISPLAY=:99 && \
         {LINUX_MOUNT}/tauri-e2e-test/target/x86_64-unknown-linux-gnu/release/tauri-e2e-test & \
         APP_PID=$! && sleep 5 && kill -0 $APP_PID 2>/dev/null && echo ALIVE || echo DEAD && \
         kill $APP_PID 2>/dev/null; true"
    ))?;
    assert!(
        launch_output.contains("ALIVE"),
        "app should survive 5 seconds"
    );

    // VM stops on drop (step 7)

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[e2e/linux] === Full E2E test PASSED ===");
    Ok(())
}

/// End-to-end: Windows — start, mount, build, launch, stop.
#[test]
#[ignore]
#[serial(windows_vm)]
fn test_full_e2e_windows() -> Result<()> {
    println!("[e2e/windows] === Full E2E test starting ===");

    // 1. Start Windows VM
    let vm = TestVm::new_with_mount(WINDOWS_PROFILE, true)?;
    vm.wait_for_ready(CONNECT_TIMEOUT)?;

    // 2. Verify SSH reachable (already done)

    // 3. Verify project mount accessible
    let (mount_list, _) = vm.ps_exec(&format!(
        "Get-ChildItem -Path '{WINDOWS_MOUNT}' -Name | Select-Object -First 5"
    ))?;
    assert!(
        !mount_list.trim().is_empty(),
        "project mount should have files"
    );
    println!("[e2e/windows] Mount OK: {}", mount_list.trim());

    // 4. Create and build Tauri project
    let project_path = project_dir().join("tauri-e2e-test");
    create_tauri_project(&project_path)?;

    println!("[e2e/windows] Building Tauri project...");
    let (build_output, _) = vm.ps_exec(&format!(
        "cd {WINDOWS_MOUNT}\\tauri-e2e-test; cargo tauri build 2>&1 | Select-Object -Last 10"
    ))?;
    println!("  Build output:\n{}", build_output);

    // 5. Verify build artifact
    let artifact_on_host = project_path
        .join("target/x86_64-pc-windows-msvc/release/tauri-e2e-test.exe");
    let built = assert_build_ok_windows(&vm, &format!("{WINDOWS_MOUNT}\\tauri-e2e-test"))?;
    assert!(built, "build artifact should exist");
    let is_pe = assert_pe_binary(&artifact_on_host)?;
    assert!(is_pe, "artifact should be PE (.exe) binary");

    // 6. Launch binary headlessly
    let binary_path = format!(
        "{WINDOWS_MOUNT}\\tauri-e2e-test\\target\\x86_64-pc-windows-msvc\\release\\tauri-e2e-test.exe"
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

    // VM stops on drop (step 7)

    // Cleanup
    std::fs::remove_dir_all(&project_path).ok();

    println!("[e2e/windows] === Full E2E test PASSED ===");
    Ok(())
}
