//! Windows bootstrap via WinRM + SSH — step-by-step idempotent.
//!
//! Each step checks concrete artifacts before running. If bootstrap fails
//! mid-way, re-running picks up where it left off.

use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tracing::{debug, warn};

use crate::vms::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::vms::bootstrap::BootstrapLogger;
use crate::vms::config::{Result, TestbedError, VmProfile};
use crate::vms::ssh::VmSession;
use crate::vms::winrm::WinRM;
use crate::vms::winrm::elevated::{self, ProgressCallback};

const CHECK_VIRTIO_PS1: &str = include_str!("../../../scripts/windows/check_virtio.ps1");
const INSTALL_VIRTIO_PS1: &str = include_str!("../../../scripts/windows/install_virtio.ps1");
const REGISTER_VIRTIOFS_STARTUP_PS1: &str = include_str!("../../../scripts/windows/register_virtiofs_startup.ps1");
const INSTALL_OPENSSH_PS1: &str = include_str!("../../../scripts/windows/install_openssh.ps1");
const CONFIGURE_SSHD_PS1: &str = include_str!("../../../scripts/windows/configure_sshd.ps1");
const SETUP_SSH_KEYS_PS1: &str = include_str!("../../../scripts/windows/setup_ssh_keys.ps1");
const SET_LOCAL_ACCOUNT_TOKEN_FILTER_PS1: &str = include_str!("../../../scripts/windows/set_local_account_token_filter.ps1");
const SET_AUTOLOGIN_PS1: &str = include_str!("../../../scripts/windows/set_autologin.ps1");
const INSTALL_MISE_PS1: &str = include_str!("../../../scripts/windows/install_mise.ps1");
const INSTALL_CARGO_BINSTALL_PS1: &str = include_str!("../../../scripts/windows/install_cargo_binstall.ps1");
const CONFIGURE_MISE_CARGO_BINSTALL_PS1: &str = include_str!("../../../scripts/windows/configure_mise_cargo_binstall.ps1");
const CHECK_VS_BUILD_TOOLS_PS1: &str = include_str!("../../../scripts/windows/check_vs_build_tools.ps1");
const INSTALL_VS_BUILD_TOOLS_PS1: &str = include_str!("../../../scripts/windows/install_vs_build_tools.ps1");
const INSTALL_WEBVIEW2_PS1: &str = include_str!("../../../scripts/windows/install_webview2.ps1");
const CHECK_WEBVIEW2_PS1: &str = include_str!("../../../scripts/windows/check_webview2.ps1");
const DEBLOAT_PS1: &str = include_str!("../../../scripts/windows/debloat.ps1");
const DEBLOAT_ACTION_PS1: &str = include_str!("../../../scripts/windows/debloat_action.ps1");
const SET_DEFENDER_EXCLUSIONS_PS1: &str = include_str!("../../../scripts/windows/set_defender_exclusions.ps1");
const CLEAN_MAIN_PS1: &str = include_str!("../../../scripts/windows/clean/main.ps1");
const CLEAN_ACTION_PS1: &str = include_str!("../../../scripts/windows/clean/action.ps1");
const CLEAN_DEEP_PS1: &str = include_str!("../../../scripts/windows/clean/deep.ps1");
const CLEAN_COMPACTOS_PS1: &str = include_str!("../../../scripts/windows/clean/aggressive_compactos.ps1");
const CLEAN_HIBERNATION_PS1: &str = include_str!("../../../scripts/windows/clean/aggressive_hibernation.ps1");
const CLEAN_VSS_PS1: &str = include_str!("../../../scripts/windows/clean/aggressive_vss.ps1");
const CLEAN_EVENT_LOGS_PS1: &str = include_str!("../../../scripts/windows/clean/aggressive_event_logs.ps1");
const CLEAN_PAGEFILE_PS1: &str = include_str!("../../../scripts/windows/clean/aggressive_pagefile.ps1");
const CONFIGURE_RUSTUP_ARM64_PS1: &str = include_str!("../../../scripts/windows/configure_rustup_arm64.ps1");
const INSTALL_TOOLS_MISE_PS1: &str = include_str!("../../../scripts/windows/install_tools_mise.ps1");
const SET_NUSHELL_DEFAULT_SHELL_PS1: &str = include_str!("../../../scripts/windows/set_nushell_default_shell.ps1");
const WRITE_BOOTSTRAP_MARKER_PS1: &str = include_str!("../../../scripts/windows/write_bootstrap_marker.ps1");
const INSTALL_WINFSP_PS1: &str = include_str!("../../../scripts/windows/install_winfsp.ps1");
const CHECK_PROJECT_MOUNT_PS1: &str = include_str!("../../../scripts/windows/check_project_mount.ps1");
const INTERACTIVE_LAUNCH_PS1: &str = include_str!("../../../scripts/windows/interactive_launch.ps1");
const SCREENSHOT_WINDOWS_PS1: &str = include_str!("../../../scripts/windows/screenshot_windows.ps1");
const SETUP_SMB_MOUNT_PS1: &str = include_str!("../../../scripts/windows/setup_smb_mount.ps1");

/// Result from running PowerShell — mirrors WinRM's `CmdResult`.
struct PsResult {
    stdout: String,
    #[allow(dead_code)]
    stderr: String,
    #[allow(dead_code)]
    exit_code: i32,
}

/// Run PowerShell over SSH, falling back to WinRM if SSH fails.
///
/// Tries `ssh::exec_ps_windows` first (proper UTF-16LE + Base64 encoding
/// with CLIXML stripping). If SSH fails for any reason, falls back to
/// `winrm.run_ps_quiet`. This allows the bootstrap SSH phase to be
/// resilient — WinRM is available as a safety net.
fn ssh_ps(session: &mut VmSession, winrm: &WinRM, script: &str) -> Result<PsResult> {
    if let Ok((stdout, exit_code)) = crate::vms::ssh::exec_ps_windows(session, script) {
        return Ok(PsResult { stdout, stderr: String::new(), exit_code });
    }
    debug!("SSH exec failed, falling back to WinRM for PowerShell");
    let cmd = winrm.run_ps_quiet(script)?;
    Ok(PsResult {
        stdout: cmd.stdout,
        stderr: cmd.stderr,
        exit_code: cmd.exit_code,
    })
}

/// Bootstrap a Windows VM with development tools.
///
/// Phase 1 (WinRM): install OpenSSH, setup keys, autologin.
/// Phase 2 (SSH): install mise, build tools, runtimes.
pub fn bootstrap_windows(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession, logger: &BootstrapLogger, progress: ProgressCallback<'_>, force_vsbuild: bool) -> Result<()> {
    // Phase 1: WinRM-only steps (no SSH required)
    logger::step(logger, "install OpenSSH Server", || {
        install_openssh_server(winrm, progress)
    })?;

    // Phase 2: SSH key authorization (now via WinRM)
    logger::step(logger, "authorise host SSH key", || {
        setup_ssh_keys(winrm)
    })?;

    // Phase 3: LocalAccountTokenFilterPolicy
    logger::step(logger, "set LocalAccountTokenFilterPolicy", || {
        set_local_account_token_filter(winrm)
    })?;

    // Phase 4: Configure autologin (Winlogon registry keys)
    logger.step_start("configure autologin");
    let autologin_start = std::time::Instant::now();
    let needs_reboot = set_autologin(winrm)?;
    logger.step_done("configure autologin", autologin_start.elapsed());

    // Autologin keys only take effect after reboot
    if needs_reboot {
        reboot_and_wait(winrm, logger)?;
        thread::sleep(std::time::Duration::from_secs(5));
    }

    // Give SSH time to reconfigure after changes
    thread::sleep(std::time::Duration::from_secs(5));

    // Phase 5+: SSH-required steps
    logger::step(logger, "install mise", || {
        install_mise(session)
    })?;

    logger::step(logger, "install cargo-binstall", || {
        install_cargo_binstall(session, winrm)
    })?;

    logger::step(logger, "configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    logger::step(logger, "install VS Build Tools", || {
        install_vs_build_tools(session, winrm, progress, force_vsbuild)
    })?;

    logger::step(logger, "install WinFsp", || {
        install_winfsp(winrm)
    })?;

    logger::step(logger, "install virtio drivers", || {
        install_virtio_drivers(session, winrm)
    })?;

    logger::step(logger, "set up project mount", || {
        setup_project_mount(session, winrm, &["virtiofs", "smb"])
    })?;

    logger::step(logger, "install WebView2 Runtime", || {
        install_webview2(profile, session, winrm)
    })?;

    logger::step(logger, "set Windows Defender exclusions", || {
        set_defender_exclusions(session)
    })?;

    logger::step(logger, "configure rustup for x86_64 (ARM64 workaround)", || {
        configure_rustup_arm64(session)
    })?;

    logger::step(logger, "install tools via mise", || {
        install_tools(session)
    })?;

    logger::step(logger, "set nushell as default shell", || {
        set_nushell_default_shell(session)
    })?;

    logger::step(logger, "write bootstrap marker", || {
        write_bootstrap_marker(session)
    })?;

    let _ = profile;
    Ok(())
}

/// Phase 1: WinRM-only bootstrap (installs and configures OpenSSH).
///
/// Runs before SSH is available. After this completes, callers should
/// wait for SSH to become reachable, then call `bootstrap_windows_ssh_phase`.
pub fn bootstrap_windows_winrm_phase(profile: &VmProfile, winrm: &WinRM, logger: &BootstrapLogger, progress: ProgressCallback<'_>) -> Result<()> {
    logger::step(logger, "install OpenSSH Server", || {
        install_openssh_server(winrm, progress)
    })?;

    logger::step(logger, "authorise host SSH key", || {
        setup_ssh_keys(winrm)
    })?;

    logger::step(logger, "set LocalAccountTokenFilterPolicy", || {
        set_local_account_token_filter(winrm)
    })?;

    // Autologin registry keys only take effect after a reboot.
    logger.step_start("configure autologin");
    let start = std::time::Instant::now();
    let needs_reboot = set_autologin(winrm)?;
    let elapsed = start.elapsed();
    logger.step_done("configure autologin", elapsed);

    if needs_reboot {
        reboot_and_wait(winrm, logger)?;
        // After reboot WinRM may be momentarily unavailable.
        // Give it a moment to stabilize before caller continues.
        thread::sleep(Duration::from_secs(5));
    }

    let _ = profile;
    Ok(())
}

/// Phase 2: SSH-required bootstrap (installs dev tools).
///
/// Call after `bootstrap_windows_winrm_phase` and waiting for SSH.
pub fn bootstrap_windows_ssh_phase(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession, logger: &BootstrapLogger, progress: ProgressCallback<'_>, force_vsbuild: bool) -> Result<()> {
    logger::step(logger, "install mise", || {
        install_mise(session)
    })?;

    logger::step(logger, "install cargo-binstall", || {
        install_cargo_binstall(session, winrm)
    })?;

    logger::step(logger, "configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    logger::step(logger, "install VS Build Tools", || {
        install_vs_build_tools(session, winrm, progress, force_vsbuild)
    })?;

    logger::step(logger, "install WinFsp", || {
        install_winfsp(winrm)
    })?;

    logger::step(logger, "install virtio drivers", || {
        install_virtio_drivers(session, winrm)
    })?;

    logger::step(logger, "set up project mount", || {
        setup_project_mount(session, winrm, &["virtiofs", "smb"])
    })?;

    logger::step(logger, "install WebView2 Runtime", || {
        install_webview2(profile, session, winrm)
    })?;

    logger::step(logger, "set Windows Defender exclusions", || {
        set_defender_exclusions(session)
    })?;

    logger::step(logger, "configure rustup for x86_64 (ARM64 workaround)", || {
        configure_rustup_arm64(session)
    })?;

    logger::step(logger, "install tools via mise", || {
        install_tools(session)
    })?;

    logger::step(logger, "set nushell as default shell", || {
        set_nushell_default_shell(session)
    })?;

    logger::step(logger, "write bootstrap marker", || {
        write_bootstrap_marker(session)
    })?;

    let _ = profile;
    Ok(())
}

/// Install/configure OpenSSH Server.
///
/// 1. Checks `C:\Program Files\OpenSSH-Win64\sshd.exe` first (pre-installed on Vagrant images).
/// 2. If found: skips Windows Capability entirely, configures service directly.
/// 3. If not found: installs via `Add-WindowsCapability`, waits for readiness.
/// 4. Always: adds sshd directory to Machine PATH, registers service if missing.
fn install_openssh_server(winrm: &WinRM, progress: ProgressCallback<'_>) -> Result<()> {
    let state = winrm.run_ps_quiet(
        "Get-Service sshd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Status",
    );
    if let Ok(s) = state && s.stdout.contains("Running") {
        return Ok(());
    }

    elevated::run_elevated(winrm, INSTALL_OPENSSH_PS1, 1800, progress)?;
    // sshd_config is handled separately (commenting out Match Group administrators, etc.)
    elevated::run_elevated(winrm, CONFIGURE_SSHD_PS1, 60, None)?;
    Ok(())
}

/// Set up SSH key authorization on Windows (both user + admin paths) via WinRM.
fn setup_ssh_keys(winrm: &WinRM) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
    let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
    let mut pub_key = String::new();

    for key_name in key_names {
        let path = home.join(".ssh").join(key_name);
        if path.exists() && let Ok(key) = std::fs::read_to_string(&path) {
            pub_key = key.trim().to_string();
            break;
        }
    }

    if pub_key.is_empty() {
        return Ok(());
    }

    let script = SETUP_SSH_KEYS_PS1.replace("{{KEY}}", &pub_key);

    let result = winrm.run_ps(&script);
    if let Err(e) = result {
        warn!("[bootstrap] SSH key setup via WinRM had issues: {e:?}");
    }
    Ok(())
}

/// Set LocalAccountTokenFilterPolicy registry key.
fn set_local_account_token_filter(winrm: &WinRM) -> Result<()> {
    elevated::run_elevated(winrm, SET_LOCAL_ACCOUNT_TOKEN_FILTER_PS1, 30, None)?;
    Ok(())
}

/// Configure Windows autologin via Winlogon registry keys.
/// Returns `true` if keys were changed (caller should reboot for them to take effect).
fn set_autologin(winrm: &WinRM) -> Result<bool> {
    // Check if already configured
    let check = winrm.run_ps_quiet(
        "Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name AutoAdminLogon -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AutoAdminLogon",
    )?;
    if check.stdout.contains("1") {
        return Ok(false);
    }

    elevated::run_elevated(winrm, SET_AUTOLOGIN_PS1, 60, None)?;
    Ok(true)
}

/// Reboot the Windows VM and wait for WinRM to come back.
pub fn reboot_and_wait(winrm: &WinRM, logger: &BootstrapLogger) -> Result<()> {
    logger.message("rebooting VM for autologin to take effect...");

    // Trigger reboot
    let _ = winrm.run_ps("Restart-Computer -Force");

    // Wait for WinRM to go down (reboot in progress)
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_secs(2));
        if winrm.run_ps_quiet("echo 1").is_err() {
            break;
        }
    }

    // Wait for WinRM to come back (autologin should be active now)
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    loop {
        if std::time::Instant::now() > deadline {
            return Err(crate::vms::config::TestbedError::BootstrapFailed {
                step: "wait for VM after reboot".to_string(),
                message: "timed out waiting for WinRM after autologin reboot".to_string(),
            });
        }
        if winrm.run_ps_quiet("echo 1").is_ok() {
            debug!("WinRM back up after reboot");
            return Ok(());
        }
        thread::sleep(Duration::from_secs(5));
    }
}

/// Install mise on Windows — downloads binary on host, SCPs to VM.
fn install_mise(session: &mut VmSession) -> Result<()> {
    let (check, _) = crate::vms::ssh::exec_ps_windows(session, "Get-Command mise -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name")?;
    if !check.trim().is_empty() {
        return Ok(());
    }

    let mise_zip = cache_or_download(
        "mise-windows-x64.zip",
        "https://github.com/jdx/mise/releases/latest/download/mise-v2026.5.1-windows-x64.zip",
    )?;

    debug!("scp'ing mise to VM...");
    crate::vms::ssh::upload(session, &mise_zip, "C:/mise.zip")?;

    crate::vms::ssh::exec_ps_windows(session, INSTALL_MISE_PS1)?;
    Ok(())
}

/// Install cargo-binstall — downloads on VM directly.
fn install_cargo_binstall(session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    let present = crate::vms::ssh::exec(
        session,
        "if (Test-Path \"$env:USERPROFILE\\.cargo\\bin\\cargo-binstall.exe\") { 'present' } else { 'missing' }",
    )?;
    if present.contains("present") {
        return Ok(());
    }

    elevated::run_elevated(winrm, INSTALL_CARGO_BINSTALL_PS1, 120, None)?;
    Ok(())
}

/// Download a file to `$HOME/.testbed/` if not already cached, return the local path.
fn cache_or_download(filename: &str, url: &str) -> Result<PathBuf> {
    let cache_dir = dirs::home_dir()
        .map(|h| h.join(".testbed/windows"))
        .unwrap_or_else(|| PathBuf::from("/tmp/.testbed/windows"));
    std::fs::create_dir_all(&cache_dir).map_err(|e| TestbedError::BootstrapFailed {
        step: format!("create cache dir {cache_dir:?}"),
        message: e.to_string(),
    })?;

    let dest = cache_dir.join(filename);
    if dest.exists() {
        debug!("using cached: {} ({} bytes)", filename, dest.metadata().unwrap().len());
        return Ok(dest);
    }

    debug!("downloading {filename} from {url} to {cache_dir:?}...");
    let status = Command::new("curl")
        .args([
            "-fSL", "--retry", "3", "--retry-delay", "5", "-C", "-",
            "-o", dest.to_str().ok_or_else(|| TestbedError::BootstrapFailed {
                step: format!("download {filename}"),
                message: "cache path is not valid UTF-8".to_string(),
            })?,
            url,
        ])
        .status()
        .map_err(|e| TestbedError::BootstrapFailed {
            step: format!("download {filename}"),
            message: format!("failed to spawn curl: {e}"),
        })?;

    if !status.success() {
        let _ = std::fs::remove_file(&dest);
        return Err(TestbedError::BootstrapFailed {
            step: format!("download {filename}"),
            message: format!("curl exited with status {status:?}"),
        });
    }

    if let Ok(meta) = dest.metadata() {
        debug!("downloaded {filename} ({} bytes)", meta.len());
    }
    Ok(dest)
}

/// Persist mise's cargo_binstall = true setting.
fn configure_mise_cargo_binstall(session: &mut VmSession) -> Result<()> {
    crate::vms::ssh::exec_ps_windows(session, CONFIGURE_MISE_CARGO_BINSTALL_PS1)?;
    Ok(())
}

/// Install VS Build Tools with C++ workload.
fn install_vs_build_tools(session: &mut VmSession, winrm: &WinRM, progress: ProgressCallback<'_>, force: bool) -> Result<()> {
    if !force {
        let vc_check = ssh_ps(session, winrm, CHECK_VS_BUILD_TOOLS_PS1)?;
        if vc_check.stdout.contains("present") {
            debug!("VS Build Tools already installed — link.exe and SDK verified");
            return Ok(());
        }
    } else {
        debug!("--force-vsbuild-install: skipping precheck, running installer");
    }

    elevated::run_elevated(winrm, INSTALL_VS_BUILD_TOOLS_PS1, 2000, progress)?;
    Ok(())
}

/// Install WebView2 Runtime (required by Tauri).
///
/// Includes retry logic with WinRM reconnection — after long-running operations
/// (like project mount setup), WinRM may become temporarily unavailable.
fn install_webview2(profile: &VmProfile, session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    // Retry loop: check if already installed
    let mut last_error = None;
    for attempt in 0..3 {
        if attempt > 0 {
            tracing::debug!("WebView2 check attempt {}/3", attempt + 1);
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
        match winrm.run_ps(CHECK_WEBVIEW2_PS1) {
            Ok(check) => {
                if check.stdout.trim() == "installed" {
                    return Ok(());
                }
                break; // Not installed, proceed to installation
            }
            Err(e) => {
                tracing::warn!("WebView2 check failed (attempt {}): {}", attempt + 1, e);
                last_error = Some(e);
            }
        }
    }

    // If all checks failed, try to restart WinRM service via SSH
    if last_error.is_some() {
        tracing::info!("WinRM unreachable, attempting recovery via SSH...");

        // Try to restart WinRM service via SSH PowerShell
        let restart_script = r#"
            try {
                Restart-Service -Name 'WinRM' -Force -ErrorAction Stop
                Start-Sleep -Seconds 3
                $svc = Get-Service -Name 'WinRM'
                if ($svc.Status -eq 'Running') { 'RESTARTED' } else { 'FAILED' }
            } catch { 'FAILED: ' + $_.Exception.Message }
        "#;

        match crate::vms::ssh::exec_ps_windows(session, restart_script) {
            Ok((output, _)) => {
                tracing::info!("WinRM restart result: {}", output.trim());
                std::thread::sleep(std::time::Duration::from_secs(10));
            }
            Err(e) => {
                tracing::warn!("SSH WinRM restart failed: {}", e);
            }
        }
    }

    // Retry installation with fresh WinRM connection
    for attempt in 0..3 {
        if attempt > 0 {
            tracing::debug!("WebView2 install attempt {}/3", attempt + 1);
            std::thread::sleep(std::time::Duration::from_secs(10));
        }

        // Use fresh WinRM connection for each attempt
        let fresh_winrm = crate::vms::winrm::WinRM::from_profile(profile)?;
        match fresh_winrm.run_ps(INSTALL_WEBVIEW2_PS1) {
            Ok(_) => return Ok(()),
            Err(e) => {
                tracing::warn!("WebView2 install failed (attempt {}): {}", attempt + 1, e);
                if attempt == 2 {
                    // Last attempt - try via SSH PowerShell directly
                    tracing::info!("Trying WebView2 install via SSH as fallback...");
                    return crate::vms::ssh::exec_ps_windows(session, INSTALL_WEBVIEW2_PS1)
                        .map(|_| ())
                        .map_err(|e| TestbedError::BootstrapFailed {
                            step: "install WebView2 Runtime".to_string(),
                            message: format!("All WinRM and SSH attempts failed: {}", e),
                        });
                }
            }
        }
    }

    Ok(())
}

/// Set Windows Defender exclusions for development directories.
fn set_defender_exclusions(session: &mut VmSession) -> Result<()> {
    crate::vms::ssh::exec_ps_windows(session, SET_DEFENDER_EXCLUSIONS_PS1)?;
    Ok(())
}

/// Remove pre-installed bloatware packages from Windows.
pub fn debloat_windows(winrm: &WinRM) -> Result<()> {
    const DEBLOAT_PREFIXES: &[&str] = &[
        "Microsoft.BingWeather", "Microsoft.GetHelp", "Microsoft.Getstarted",
        "Microsoft.MicrosoftSolitaireCollection", "Microsoft.MicrosoftStickyNotes",
        "Microsoft.MixedReality.Portal", "Microsoft.Office.OneNote",
        "Microsoft.OneDriveSync", "Microsoft.People", "Microsoft.PowerAutomateDesktop",
        "Microsoft.ScreenSketch", "Microsoft.SkypeApp", "Microsoft.Todos",
        "Microsoft.WindowsAlarms", "Microsoft.WindowsCamera", "Microsoft.WindowsFeedbackHub",
        "Microsoft.WindowsMaps", "Microsoft.WindowsSoundRecorder", "Microsoft.Xbox.TCUI",
        "Microsoft.XboxApp", "Microsoft.XboxGameOverlay", "Microsoft.XboxGamingOverlay",
        "Microsoft.XboxIdentityProvider", "Microsoft.XboxSpeechToTextOverlay",
        "Microsoft.YourPhone", "Microsoft.ZuneMusic", "Microsoft.ZuneVideo",
        "Clipchamp.Clipchamp", "MicrosoftTeams", "SpotifyAB.SpotifyMusic",
    ];
    let prefixes_ps = DEBLOAT_PREFIXES.iter().map(|p| format!("'{p}'")).collect::<Vec<_>>().join(", ");
    let script = DEBLOAT_PS1
        .replace("__PREFIXES__", &prefixes_ps)
        .replace("__ACTION__", DEBLOAT_ACTION_PS1);
    elevated::run_elevated(winrm, &script, 1200, None)?; // 20 min for AppxPackage removal
    Ok(())
}

/// Set up the project mount for Windows guests.
///
/// Registers a persistent Windows Scheduled Task that auto-mounts virtiofs
/// on every boot (runs as 'vagrant' in the interactive session so WinFsp
/// can create a user-visible mount at `C:\Users\vagrant\project`).
/// Then starts the task immediately and waits for the mount to appear.
fn setup_project_mount(session: &mut VmSession, winrm: &WinRM, _methods: &[&str]) -> Result<()> {
    // Check if already mounted with content verification
    let check = ssh_ps(session, winrm, CHECK_PROJECT_MOUNT_PS1)?;
    let stdout = check.stdout.trim();

    // Parse the result - now returns "mounted|METHOD" or "missing" etc
    if stdout.starts_with("mounted|") {
        let method = stdout.split('|').nth(1).unwrap_or("unknown");
        debug!("project mount already active via {}", method);
        return Ok(());
    }

    if stdout.starts_with("starting|") {
        debug!("virtiofs process starting, waiting for completion...");
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if std::time::Instant::now() > deadline { break; }
            thread::sleep(Duration::from_secs(3));
            let check = ssh_ps(session, winrm, CHECK_PROJECT_MOUNT_PS1)?;
            if check.stdout.starts_with("mounted|") {
                let method = check.stdout.split('|').nth(1).unwrap_or("unknown");
                debug!("project mount now active via {}", method);
                return Ok(());
            }
        }
    }

    // Register startup task and run mount immediately
    elevated::run_elevated(winrm, REGISTER_VIRTIOFS_STARTUP_PS1, 60, None)?;

    // Wait for virtiofs mount to appear
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    loop {
        if std::time::Instant::now() > deadline { break; }
        thread::sleep(Duration::from_secs(5));
        let check = ssh_ps(session, winrm, CHECK_PROJECT_MOUNT_PS1)?;
        let stdout = check.stdout.trim();
        debug!("mount check output: {}", stdout);
        if stdout.starts_with("mounted|virtiofs") {
            debug!("project mount verified via virtiofs");
            return Ok(());
        }
        if stdout.starts_with("mounted|") {
            let method = check.stdout.split('|').nth(1).unwrap_or("unknown");
            debug!("project mount active via {} (not virtiofs)", method);
            return Ok(());
        }
        if stdout.starts_with("exists|") {
            debug!("mount point exists but not accessible: {}", stdout);
        }
        if stdout.starts_with("starting|") {
            debug!("virtiofs process is starting...");
        }
        if stdout == "missing" {
            // Check if virtiofs.exe is installed
            let virtio_check = ssh_ps(session, winrm, "if (Test-Path 'C:\\Program Files\\Virtio-Win\\VioFS\\virtiofs.exe') { 'EXISTS' } else { 'MISSING' }")?;
            debug!("virtiofs.exe check: {}", virtio_check.stdout.trim());

            // Check VirtIO-FS driver status
            let driver_check = ssh_ps(session, winrm, "Get-PnpDevice -Class System -ErrorAction SilentlyContinue | Where-Object { $_.FriendlyName -like '*VirtIO*' } | Select-Object -First 3 | ForEach-Object { $_.FriendlyName }")?;
            debug!("VirtIO drivers found: {}", driver_check.stdout.trim());

            // Check WinFsp service
            let winfsp_check = ssh_ps(session, winrm, "Get-Service -Name 'WinFsp.Launcher' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Status")?;
            debug!("WinFsp.Launcher status: {}", winfsp_check.stdout.trim());
        }
    }

    // Fallback: try SMB mount directly
    debug!("virtiofs startup mount failed, falling back to SMB");
    elevated::run_interactive(winrm, SETUP_SMB_MOUNT_PS1, "vagrant")?;

    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        if std::time::Instant::now() > deadline { break; }
        thread::sleep(Duration::from_secs(5));
        let check = ssh_ps(session, winrm, CHECK_PROJECT_MOUNT_PS1)?;
        if check.stdout.starts_with("mounted|") {
            let method = check.stdout.split('|').nth(1).unwrap_or("unknown");
            debug!("project mount verified via {}", method);
            return Ok(());
        }
    }

    warn!("all mount methods failed");
    Ok(())
}

/// Install WinFsp — userspace filesystem framework required by virtiofs.exe.
///
/// virtiofs.exe on Windows uses WinFsp to mount the host filesystem. Without
/// WinFsp, the VirtIO-FS service fails with "failed to load WinFsp DLL".
fn install_winfsp(winrm: &WinRM) -> Result<()> {
    // Check if already installed
    let check = winrm.run_ps_quiet(
        "Get-Service -Name 'WinFsp.Launcher' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Status",
    );
    if let Ok(s) = check && s.stdout.contains("Running") {
        return Ok(());
    }

    elevated::run_elevated(winrm, INSTALL_WINFSP_PS1, 600, None)?;
    Ok(())
}

/// Install virtio-win drivers if not already present.
fn install_virtio_drivers(session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    let check = ssh_ps(session, winrm, CHECK_VIRTIO_PS1)?;
    let result = check.stdout.trim();

    if !result.starts_with("not_installed") {
        let parts: Vec<&str> = result.split('|').collect();
        if parts.len() >= 3 {
            debug!("virtio-win v{} already installed at {}", parts[0], parts[1]);
            return Ok(());
        }
    }

    debug!("virtio-win not found, installing from latest ISO");
    elevated::run_elevated(winrm, INSTALL_VIRTIO_PS1, 600, None)?;
    Ok(())
}

/// Configure rustup default-host to x86_64 on ARM64 Windows hosts.
fn configure_rustup_arm64(session: &mut VmSession) -> Result<()> {
    let (arch, _) = crate::vms::ssh::exec_ps_windows(session, "$env:PROCESSOR_ARCHITECTURE")?;
    if !arch.contains("ARM64") {
        return Ok(());
    }

    crate::vms::ssh::exec_ps_windows(session, CONFIGURE_RUSTUP_ARM64_PS1)?;
    Ok(())
}

/// Install development tools via bootstrap mise.toml.
fn install_tools(session: &mut VmSession) -> Result<()> {
    let script = INSTALL_TOOLS_MISE_PS1.replace("{{MISE_TOML}}", BOOTSTRAP_MISE_TOML);
    crate::vms::ssh::exec_ps_windows(session, &script)?;
    crate::vms::ssh::exec(session, "mise exec -- rustc --version")?;
    Ok(())
}

/// Set nushell as the default interactive shell on Windows.
fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    crate::vms::ssh::exec_ps_windows(session, SET_NUSHELL_DEFAULT_SHELL_PS1)?;
    Ok(())
}

/// Write the bootstrap completion marker.
fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::vms::ssh::exec_ps_windows(session, WRITE_BOOTSTRAP_MARKER_PS1)?;
    Ok(())
}

/// Clean transient files from the Windows guest.
pub fn clean_windows(winrm: &WinRM, deep: bool) -> crate::vms::config::Result<()> {
    let deep_block = if deep { CLEAN_DEEP_PS1 } else { "" };
    let script = CLEAN_MAIN_PS1
        .replace("__DEEP_BLOCK__", deep_block)
        .replace("__ACTION__", CLEAN_ACTION_PS1);
    elevated::run_elevated(winrm, &script, 600, None)?;
    Ok(())
}

/// Aggressive cleanup — CompactOS, hibernation, VSS, event logs.
pub fn aggressive_clean(winrm: &WinRM) -> crate::vms::config::Result<()> {
    for (_name, script) in [
        ("CompactOS", CLEAN_COMPACTOS_PS1),
        ("hibernation", CLEAN_HIBERNATION_PS1),
        ("VSS shadows", CLEAN_VSS_PS1),
        ("event logs", CLEAN_EVENT_LOGS_PS1),
        ("pagefile", CLEAN_PAGEFILE_PS1),
    ] {
        elevated::run_elevated(winrm, script, 300, None)?;
    }
    Ok(())
}

/// Launch a binary in the vagrant interactive desktop session via Scheduled Task.
///
/// Uses `/RU vagrant /IT` so GUI apps actually appear on screen (unlike
/// headless SSH/WinRM execution where Win32 GUI windows are silently exited).
pub fn run_interactive_windows(winrm: &WinRM, bin_path: &str, args: &[String]) -> Result<()> {
    let arglist = if args.is_empty() {
        String::new()
    } else {
        let quoted = args.iter().map(|a| format!("'{a}'")).collect::<Vec<_>>().join(",");
        format!(" -ArgumentList @({quoted})")
    };
    let script = INTERACTIVE_LAUNCH_PS1
        .replace("__BIN__", bin_path)
        .replace("__ARGLIST__", &arglist);
    elevated::run_interactive(winrm, &script, "vagrant")?;
    Ok(())
}

/// Capture a screenshot of the vagrant interactive desktop session.
///
/// Runs in a Scheduled Task with `/RU vagrant /IT` so `Graphics.CopyFromScreen`
/// sees the real desktop. The PNG is written to a VM path, then SCP'd back.
pub fn screenshot_windows(winrm: &WinRM, session: &mut VmSession, output: &std::path::Path) -> Result<()> {
    let vm_path = "$env:USERPROFILE\\.testbed-screenshot.png";
    let script = SCREENSHOT_WINDOWS_PS1.replace("__OUTPUT_PATH__", vm_path);

    // Launch in interactive session
    elevated::run_interactive(winrm, &script, "vagrant")?;

    // Give it a moment to capture
    thread::sleep(std::time::Duration::from_secs(3));

    // Download to host
    crate::vms::ssh::download(session, "C:\\Users\\vagrant\\.testbed-screenshot.png", output)?;

    // Clean up on VM
    crate::vms::ssh::exec_ps_windows(session, "Remove-Item 'C:\\Users\\vagrant\\.testbed-screenshot.png' -Force -ErrorAction SilentlyContinue")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bootstrap_mise_toml_has_required_tools() {
        let toml = crate::vms::bootstrap::BOOTSTRAP_MISE_TOML;
        assert!(toml.contains("rust"));
        assert!(toml.contains("nushell"));
        assert!(toml.contains("tauri-cli"));
    }
}
