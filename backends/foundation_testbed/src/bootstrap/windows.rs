//! Windows bootstrap via WinRM + SSH — step-by-step idempotent.
//!
//! Each step checks concrete artifacts before running. If bootstrap fails
//! mid-way, re-running picks up where it left off.

use std::thread;
use tracing::warn;

use crate::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::bootstrap::BootstrapLogger;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;
use crate::winrm::WinRM;
use crate::winrm::elevated::{self, ProgressCallback};

/// Bootstrap a Windows VM with development tools.
///
/// Phase 1 (WinRM): install OpenSSH, setup keys, autologin.
/// Phase 2 (SSH): install mise, build tools, runtimes.
pub fn bootstrap_windows(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession, logger: &BootstrapLogger, progress: ProgressCallback<'_>) -> Result<()> {
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
    logger::step(logger, "configure autologin", || {
        set_autologin(winrm)
    })?;

    // Give SSH time to reconfigure after changes
    thread::sleep(std::time::Duration::from_secs(5));

    // Phase 5+: SSH-required steps
    logger::step(logger, "install mise", || {
        install_mise(session)
    })?;

    // Step 5: cargo-binstall (before tools, so mise can use it)
    logger::step(logger, "install cargo-binstall", || {
        install_cargo_binstall(session)
    })?;

    // Step 6: configure mise cargo_binstall setting
    logger::step(logger, "configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    // Step 7: VS Build Tools with C++ workload
    logger::step(logger, "install VS Build Tools", || {
        install_vs_build_tools(session, winrm, progress)
    })?;

    // Step 7b: Install virtio drivers from CD-ROM (virtio-win ISO)
    logger::step(logger, "install virtio drivers", || {
        install_virtio_drivers(session, winrm)
    })?;

    // Step 7c: Set up viofs project mount service
    logger::step(logger, "set up project mount", || {
        setup_project_mount(session, winrm)
    })?;

    // Step 8: WebView2 Runtime
    logger::step(logger, "install WebView2 Runtime", || {
        install_webview2(session)
    })?;

    // Step 9: Windows Defender exclusions for dev directories
    logger::step(logger, "set Windows Defender exclusions", || {
        set_defender_exclusions(session)
    })?;

    // Step 10: rustup ARM64 default-host workaround (only on ARM64 hosts)
    logger::step(logger, "configure rustup for x86_64 (ARM64 workaround)", || {
        configure_rustup_arm64(session)
    })?;

    // Step 11: install tools via bootstrap mise.toml
    logger::step(logger, "install tools via mise", || {
        install_tools(session)
    })?;

    // Step 12: set nushell as default shell
    logger::step(logger, "set nushell as default shell", || {
        set_nushell_default_shell(session)
    })?;

    // Step 13: write bootstrap marker
    logger::step(logger, "write bootstrap marker", || {
        write_bootstrap_marker(session)
    })?;

    // Silence unused warnings for profile (used by callers for routing)
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

    logger::step(logger, "configure autologin", || {
        set_autologin(winrm)
    })?;

    let _ = profile;
    Ok(())
}

/// Phase 2: SSH-required bootstrap (installs dev tools).
///
/// Call after `bootstrap_windows_winrm_phase` and waiting for SSH.
pub fn bootstrap_windows_ssh_phase(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession, logger: &BootstrapLogger, progress: ProgressCallback<'_>) -> Result<()> {
    logger::step(logger, "install mise", || {
        install_mise(session)
    })?;

    logger::step(logger, "install cargo-binstall", || {
        install_cargo_binstall(session)
    })?;

    logger::step(logger, "configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    logger::step(logger, "install VS Build Tools", || {
        install_vs_build_tools(session, winrm, progress)
    })?;

    logger::step(logger, "install virtio drivers", || {
        install_virtio_drivers(session, winrm)
    })?;

    logger::step(logger, "set up project mount", || {
        setup_project_mount(session, winrm)
    })?;

    logger::step(logger, "install WebView2 Runtime", || {
        install_webview2(session)
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

/// Install/configure OpenSSH Server — resilient two-path approach.
///
/// 1. Checks `C:\Program Files\OpenSSH-Win64\sshd.exe` first (pre-installed on Vagrant images).
/// 2. If found: skips Windows Capability entirely, configures service directly.
/// 3. If not found: installs via `Add-WindowsCapability`, waits for readiness.
/// 4. Always: adds sshd directory to Machine PATH, registers service if missing
///    via `sc.exe`, configures sshd_config, starts and verifies sshd.
fn install_openssh_server(winrm: &WinRM, progress: ProgressCallback<'_>) -> Result<()> {
    // Quick exit if sshd is already running
    let state = winrm.run_ps_quiet(
        "Get-Service sshd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Status",
    );
    if let Ok(s) = state
        && s.stdout.trim().eq_ignore_ascii_case("Running")
    {
        return Ok(());
    }

    let script = r#"
$ErrorActionPreference = 'Continue'
$Error.Clear()
$progressLog = 'C:\bootstrap-step-progress.log'

function Log-Progress {
    param([string]$msg)
    $ts = Get-Date -Format 'o'
    Add-Content -Path $progressLog -Value "[$ts] $msg" -Encoding UTF8
    Write-Output $msg
}

Log-Progress "Phase 1: locating sshd.exe"

# ── Phase 1: Locate or install sshd.exe ──────────────────────────
$sshdExe = $null
$sshdDir = $null

Log-Progress "[openssh] Phase 1: locating sshd.exe"
Log-Progress "[openssh]   checking C:\Program Files\OpenSSH-Win64\sshd.exe"
$progFiles = Test-Path 'C:\Program Files\OpenSSH-Win64\sshd.exe'
Log-Progress "[openssh]     result: $progFiles"
if ($progFiles) {
    $sshdExe = 'C:\Program Files\OpenSSH-Win64\sshd.exe'
    $sshdDir = 'C:\Program Files\OpenSSH-Win64'
    $files = (Get-ChildItem 'C:\Program Files\OpenSSH-Win64' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name) -join ', '
    Log-Progress "[openssh]   FOUND pre-installed at $sshdExe"
    Log-Progress "[openssh]   directory contents: $files"
}
else {
    Log-Progress "[openssh]   checking C:\Windows\System32\OpenSSH\sshd.exe"
    $sysDir = Test-Path 'C:\Windows\System32\OpenSSH\sshd.exe'
    Log-Progress "[openssh]     result: $sysDir"
    if ($sysDir) {
        $sshdExe = 'C:\Windows\System32\OpenSSH\sshd.exe'
        $sshdDir = 'C:\Windows\System32\OpenSSH'
        $files = (Get-ChildItem 'C:\Windows\System32\OpenSSH' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name) -join ', '
        Log-Progress "[openssh]   FOUND Windows Capability at $sshdExe"
        Log-Progress "[openssh]   directory contents: $files"
    }
    else {
        Log-Progress "[openssh]   NOT FOUND in either location — installing via Windows Capability"
        $cap = Get-WindowsCapability -Online | Where-Object { $_.Name -like 'OpenSSH.Server*' }
        if (-not $cap) { throw "OpenSSH.Server capability not available" }
        Log-Progress "[openssh]   capability state: $($cap.State), installing..."
        Add-WindowsCapability -Online -Name $cap.Name
        Log-Progress "[openssh]   capability install complete, polling for sshd.exe..."

        # Poll for sshd.exe to appear (capability install can be async)
        $found = $false
        for ($i = 0; $i -lt 60; $i++) {
            Start-Sleep -Seconds 5
            if (($i + 1) % 6 -eq 0) {
                Log-Progress "[openssh]   still waiting... ($(($i + 1) * 5)s elapsed)"
            }
            if (Test-Path 'C:\Program Files\OpenSSH-Win64\sshd.exe') {
                $sshdExe = 'C:\Program Files\OpenSSH-Win64\sshd.exe'
                $sshdDir = 'C:\Program Files\OpenSSH-Win64'
                $found = $true
                Log-Progress "[openssh]   FOUND at Program Files after $(( $i + 1) * 5)s"
                break
            }
            if (Test-Path 'C:\Windows\System32\OpenSSH\sshd.exe') {
                $sshdExe = 'C:\Windows\System32\OpenSSH\sshd.exe'
                $sshdDir = 'C:\Windows\System32\OpenSSH'
                $found = $true
                Log-Progress "[openssh]   FOUND at System32 after $(( $i + 1) * 5)s"
                break
            }
        }
        if (-not $found) {
            # List what DID appear
            Log-Progress "[openssh]   listing C:\Program Files\OpenSSH-Win64:"
            Get-ChildItem 'C:\Program Files\OpenSSH-Win64' -ErrorAction SilentlyContinue | ForEach-Object { Log-Progress "[openssh]     $($_.Name)" }
            Log-Progress "[openssh]   listing C:\Windows\System32\OpenSSH:"
            Get-ChildItem 'C:\Windows\System32\OpenSSH' -ErrorAction SilentlyContinue | ForEach-Object { Log-Progress "[openssh]     $($_.Name)" }
            throw "sshd.exe did not appear after Windows Capability install (5 min timeout)"
        }
    }
}

# ── Phase 2: Add sshd directory to Machine PATH ──────────────────
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($sshdDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$sshdDir;$machinePath", 'Machine')
    Log-Progress "[openssh] added $sshdDir to Machine PATH"
}

# ── Phase 3: Ensure sshd service exists ──────────────────────────
$svc = Get-Service sshd -ErrorAction SilentlyContinue
if (-not $svc) {
    # Service not registered — create it manually
    Log-Progress "[openssh] sshd service not found, registering via sc.exe"
    sc.exe create sshd binPath= "`"$sshdExe`"" start= auto DisplayName= "OpenSSH SSH Server" 2>&1 | Out-Null
    Start-Sleep -Seconds 2
    $svc = Get-Service sshd -ErrorAction SilentlyContinue
    if (-not $svc) {
        # Fallback: try New-Service
        Log-Progress "[openssh] sc.exe failed, trying New-Service"
        New-Service -Name sshd -BinaryPathName $sshdExe -DisplayName "OpenSSH SSH Server" -StartupType Automatic 2>&1 | Out-Null
        Start-Sleep -Seconds 2
        $svc = Get-Service sshd -ErrorAction SilentlyContinue
    }
    if (-not $svc) {
        throw "Could not register sshd service via sc.exe or New-Service"
    }
    Log-Progress "[openssh] sshd service registered"
}

# ── Phase 4: Configure sshd_config ──────────────────────────────
$sshd_config = 'C:\ProgramData\ssh\sshd_config'
if (Test-Path $sshd_config) {
    $c = Get-Content $sshd_config
    $o = @()
    foreach ($l in $c) {
        if ($l -match '^#?PasswordAuthentication')              { $o += 'PasswordAuthentication yes' }
        elseif ($l -match '^#?PubkeyAuthentication')            { $o += 'PubkeyAuthentication yes' }
        elseif ($l -match '^Match Group administrators')        { $o += '#Match Group administrators' }
        elseif ($l -match 'AuthorizedKeysFile __PROGRAMDATA__') { $o += '#AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys' }
        else                                                     { $o += $l }
    }
    $o | Set-Content $sshd_config -Force -Encoding UTF8
    Log-Progress "[openssh] sshd_config configured"
}

# ── Phase 5: Start sshd and verify ───────────────────────────────
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd -ErrorAction SilentlyContinue

# Verify it's running (poll briefly — service can take a moment)
for ($i = 0; $i -lt 12; $i++) {
    $st = (Get-Service sshd -ErrorAction SilentlyContinue).Status
    if ($st -eq 'Running') {
        Log-Progress "[openssh] sshd is Running"
        break
    }
    Start-Sleep -Seconds 2
}

$final = (Get-Service sshd -ErrorAction SilentlyContinue).Status
if ($final -ne 'Running') {
    throw "sshd service is not running (state: $final). Check C:\ProgramData\ssh\logs\sshd.log"
}
"#;

    elevated::run_elevated(winrm, script, 1800, progress)?;
    Ok(())
}

/// Set up SSH key authorization on Windows (both user + admin paths) via WinRM.
/// Uses direct WinRM calls (already elevated) instead of scheduled tasks.
fn setup_ssh_keys(winrm: &WinRM) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
    let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
    let mut pub_key = String::new();

    for key_name in key_names {
        let path = home.join(".ssh").join(key_name);
        if path.exists()
            && let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key.trim().to_string();
                break;
            }
    }

    if pub_key.is_empty() {
        return Ok(());
    }

    // Escape for PowerShell double-quoted string
    let key_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        pub_key.as_bytes(),
    );

    let script = format!(
        r#"
$bytes = [System.Convert]::FromBase64String('{key_b64}')
$key = [System.Text.Encoding]::UTF8.GetString($bytes)

# User-level authorized_keys
$dir = "$env:USERPROFILE\.ssh"
if (-not (Test-Path $dir)) {{ New-Item -ItemType Directory -Path $dir -Force | Out-Null }}
$f = "$dir\authorized_keys"
if (-not (Test-Path $f) -or ((Get-Content $f -Raw -ErrorAction SilentlyContinue) -notcontains $key)) {{
    Add-Content $f $key -Encoding UTF8
}}

# Admin-level authorized_keys
$adm = 'C:\ProgramData\ssh\administrators_authorized_keys'
if (-not (Test-Path $adm) -or ((Get-Content $adm -Raw -ErrorAction SilentlyContinue) -notcontains $key)) {{
    Add-Content $adm $key -Encoding UTF8
}}
icacls $adm /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F' | Out-Null

# Restart sshd to pick up changes
Restart-Service sshd -ErrorAction SilentlyContinue
Start-Service sshd -ErrorAction SilentlyContinue
"#
    );

    // Direct WinRM call — already runs as admin, no scheduled task needed
    let result = winrm.run_ps(&script);
    if let Err(e) = result {
        warn!("[bootstrap] SSH key setup via WinRM had issues: {e:?}");
    }
    Ok(())
}

/// Set LocalAccountTokenFilterPolicy registry key.
fn set_local_account_token_filter(winrm: &WinRM) -> Result<()> {
    let script = r#"
$reg_path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
Set-ItemProperty -Path $reg_path -Name 'LocalAccountTokenFilterPolicy' -Value 1 -Type DWord -Force
"#;
    elevated::run_elevated(winrm, script, 30, None)?;
    Ok(())
}

/// Configure Windows autologin via Winlogon registry keys.
///
/// Sets AutoAdminLogon=1, DefaultUsername, and DefaultPassword so the VM
/// logs in automatically after boot, enabling SSH access without manual
/// VNC login.
fn set_autologin(winrm: &WinRM) -> Result<()> {
    // Check if already configured
    let check = winrm.run_ps_quiet(
        "Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Winlogon' -Name AutoAdminLogon -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AutoAdminLogon",
    )?;
    if check.stdout.trim() == "1" {
        return Ok(());
    }

    // Use reg.exe — AutoAdminLogon must be REG_DWORD for Windows to honor it
    let script = r#"
reg add "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon" /v AutoAdminLogon /t REG_DWORD /d 1 /f
reg add "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon" /v DefaultUsername /t REG_SZ /d vagrant /f
reg add "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon" /v DefaultPassword /t REG_SZ /d vagrant /f
reg delete "HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon" /v AutoLogonCount /f 2>nul || true
"#;
    elevated::run_elevated(winrm, script, 30, None)?;
    Ok(())
}

/// Install mise on Windows via PowerShell installer.
fn install_mise(session: &mut VmSession) -> Result<()> {
    // Check if mise is already available
    let check = crate::ssh::exec(session, "Get-Command mise -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name")?;
    if !check.trim().is_empty() {
        return Ok(());
    }

    let script = r#"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
iwr -useb https://mise.run | iex

$miseDir = "$env:USERPROFILE\.local\bin"

# Set User-level PATH
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notmatch [regex]::Escape($miseDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseDir;$userPath", 'User')
}

# Also set Machine-level PATH so OpenSSH sessions always see it
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($miseDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseDir;$machinePath", 'Machine')
}

# Update current session PATH
$env:PATH = "$miseDir;$env:PATH"
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Install cargo-binstall directly (binary download, not cargo install).
fn install_cargo_binstall(session: &mut VmSession) -> Result<()> {
    let present = crate::ssh::exec(
        session,
        "if (Test-Path \"$env:USERPROFILE\\.cargo\\bin\\cargo-binstall.exe\") { 'present' } else { 'missing' }",
    )?;
    if present.contains("present") {
        return Ok(());
    }

    let script = r#"
$dest = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path $dest)) { New-Item -ItemType Directory -Path $dest | Out-Null }
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$zip = "$env:TEMP\cargo-binstall.zip"
Invoke-WebRequest -Uri 'https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-pc-windows-msvc.zip' -OutFile $zip -UseBasicParsing
Expand-Archive -Force $zip $dest
Remove-Item $zip -Force -ErrorAction SilentlyContinue

# Set both User and Machine PATH for OpenSSH session visibility
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($userPath -notmatch [regex]::Escape($dest)) {
    [Environment]::SetEnvironmentVariable('PATH', $dest + ';' + $userPath, 'User')
}
if ($machinePath -notmatch [regex]::Escape($dest)) {
    [Environment]::SetEnvironmentVariable('PATH', $dest + ';' + $machinePath, 'Machine')
}
$env:PATH = $dest + ';' + $env:PATH
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Persist mise's cargo_binstall = true setting.
fn configure_mise_cargo_binstall(session: &mut VmSession) -> Result<()> {
    let script = r#"
$cfg = "$env:USERPROFILE\AppData\Roaming\mise\config.toml"
$dir = Split-Path $cfg
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
if (-not (Test-Path $cfg)) {
    Set-Content $cfg "[settings]`ncargo_binstall = true`n" -Encoding UTF8
} elseif ((Get-Content $cfg -Raw) -notmatch 'cargo_binstall') {
    Add-Content $cfg "`n[settings]`ncargo_binstall = true`n" -Encoding UTF8
}
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Install VS Build Tools with C++ workload, checking for actual binary.
fn install_vs_build_tools(session: &mut VmSession, winrm: &WinRM, progress: ProgressCallback<'_>) -> Result<()> {
    // Check for the actual binary we need: Hostarm64\x64\link.exe
    let vc_check = winrm.run_ps_quiet(
        r#"if (Get-ChildItem 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostarm64\x64\link.exe' -ErrorAction SilentlyContinue) { 'present' } else { 'missing' }"#
    )?;
    if vc_check.stdout.trim() == "present" {
        return Ok(());
    }

    // Download bootstrapper
    crate::ssh::exec(
        session,
        r#"[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_buildtools.exe' -OutFile 'C:\vs_buildtools.exe' -UseBasicParsing"#,
    )?;

    // Install elevated with C++ workload + ARM64 cross-tools
    elevated::run_elevated(
        winrm,
        r#"
Start-Process -FilePath 'C:\vs_buildtools.exe' -ArgumentList @(
    '--add', 'Microsoft.VisualStudio.Workload.VCTools',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
    '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621',
    '--includeRecommended', '--quiet', '--norestart', '--wait'
) -Wait -NoNewWindow -PassThru | Out-Null
"#,
        1800,
        progress,
    )?;
    Ok(())
}

/// Install WebView2 Runtime (required by Tauri).
fn install_webview2(session: &mut VmSession) -> Result<()> {
    let present = crate::ssh::exec(
        session,
        r#"if (Test-Path 'C:\Program Files (x86)\Microsoft\EdgeWebView') { 'installed' } else { 'missing' }"#,
    )?;
    if present.contains("installed") {
        return Ok(());
    }

    let script = r#"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile 'C:\webview2_setup.exe' -UseBasicParsing
Start-Process 'C:\webview2_setup.exe' -ArgumentList '/silent','/install' -Wait -NoNewWindow
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Set Windows Defender exclusions for development directories.
fn set_defender_exclusions(session: &mut VmSession) -> Result<()> {
    let script = r#"
Add-MpPreference -ExclusionPath "C:\Users\vagrant\project" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\Users\vagrant\.cargo" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\Users\vagrant\.rustup" -ErrorAction SilentlyContinue
Add-MpPreference -ExclusionPath "C:\Users\vagrant\.local" -ErrorAction SilentlyContinue
"#;
    crate::ssh::exec_ps_windows(session, script)?;
    Ok(())
}

/// Set up the viofs-based project mount for Windows guests.
///
/// The virtio-win ISO provides the viofs driver (installed separately) and
/// `virtiofs.exe` mount utility. This function:
/// 1. Copies virtiofs.exe from the ISO to the VM
/// 2. Creates the mount point directory
/// 3. Sets up a startup scheduled task to auto-mount on boot
/// 4. Triggers an immediate mount
fn setup_project_mount(_session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    // Check if already set up — verify virtiofs.exe + mount is functional
    let check = winrm.run_ps_quiet(
        r#"
        $exeOk = Test-Path 'C:\Program Files\virtiofs\virtiofs.exe'
        $mountOk = Test-Path 'C:\Users\vagrant\project\Cargo.toml'
        if ($exeOk -and $mountOk) { 'configured' } else { 'missing' }
        "#
    )?;
    if check.stdout.trim() == "configured" {
        return Ok(());
    }

    // Find the CD-ROM with virtio-win
    let script = r#"
# Kill any stale virtiofs processes
Get-Process -Name 'virtiofs' -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue

# Find CD-ROM with virtio-win
$cd = (Get-Volume | Where-Object { $_.FileSystemLabel -like 'virtio*' }).DriveLetter
if (-not $cd) {
    $cds = Get-CimInstance Win32_CDROMDrive | ForEach-Object { $_.Drive }
    foreach ($d in $cds) {
        if (Test-Path "${d}:\w11\amd64") {
            $cd = $d
            break
        }
    }
}
if (-not $cd) { throw "virtio-win CD-ROM not found" }

# Create virtiofs install directory
$installDir = 'C:\Program Files\virtiofs'
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

# Copy virtiofs tools from ISO (viofs directory on the ISO)
$viofsDir = "${cd}:\viofs"
if (Test-Path $viofsDir) {
    Copy-Item -Path "$viofsDir\*" -Destination $installDir -Recurse -Force
}

# Also copy from the architecture-specific directory
$toolsDir = "${cd}:\vioserial\w11\amd64"
if (Test-Path $toolsDir) {
    Copy-Item -Path "$toolsDir\*" -Destination $installDir -Recurse -Force -ErrorAction SilentlyContinue
}

# Create the mount point
$mountPoint = 'C:\Users\vagrant\project'
if (-not (Test-Path $mountPoint)) {
    New-Item -ItemType Directory -Path $mountPoint -Force | Out-Null
}

# Create the mount script for boot-time use
$mountScript = @"
# Mount virtiofs project drive
Start-Process -FilePath 'virtiofs.exe' -ArgumentList '-t project', '-m C:\Users\vagrant\project' -WindowStyle Hidden
"@
Set-Content -Path "$installDir\mount-project.ps1" -Value $mountScript -Encoding UTF8

# Create a scheduled task to mount on every boot
$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-WindowStyle Hidden -ExecutionPolicy Bypass -File `"$installDir\mount-project.ps1`""
$trigger = New-ScheduledTaskTrigger -AtStartup
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest
Register-ScheduledTask -TaskName 'MountVirtiofsProject' -Action $action -Trigger $trigger -Principal $principal -Force | Out-Null

# Mount now: start virtiofs.exe directly as the current user (elevated admin)
$virtiofsExe = "$installDir\virtiofs.exe"
if (-not (Test-Path $virtiofsExe)) {
    throw "virtiofs.exe not found at $virtiofsExe"
}

Start-Process -FilePath $virtiofsExe -ArgumentList '-t project', '-m C:\Users\vagrant\project' -WindowStyle Hidden

# Wait for mount to become accessible
$mounted = $false
for ($i = 0; $i -lt 15; $i++) {
    Start-Sleep -Seconds 2
    if (Test-Path "$mountPoint\Cargo.toml") {
        $mounted = $true
        break
    }
}

if (-not $mounted) {
    $files = Get-ChildItem $mountPoint -ErrorAction SilentlyContinue
    Log-Progress "Mount check failed. Contents of $mountPoint : $($files | Select-Object -ExpandProperty Name -ErrorAction SilentlyContinue)"
    throw "virtiofs mount did not become accessible after 30s"
}

Log-Progress "Project mount verified at $mountPoint"
"#;
    elevated::run_elevated(winrm, script, 120, None)?;
    Ok(())
}

/// Install virtio-win drivers from the CD-ROM ISO.
///
/// The ISO must be attached as a CD-ROM drive to QEMU before launching.
/// Drivers are installed via `pnputil` which stages them in the driver
/// store and makes them available for the virtual hardware.
///
/// Idempotent: skips if virtio drivers are already installed.
fn install_virtio_drivers(_session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    // Check if virtio drivers are already installed
    let check = winrm.run_ps_quiet(
        r#"$devices = Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.FriendlyName -like '*VirtIO*' -or $_.FriendlyName -like '*Red Hat*' }; if ($null -ne $devices -and $devices.Count -gt 0) { 'installed' } else { 'missing' }"#
    )?;
    if check.stdout.trim() == "installed" {
        return Ok(());
    }

    // Find CD-ROM with virtio label and install all drivers
    let script = r#"
$cd = (Get-Volume | Where-Object { $_.FileSystemLabel -like 'virtio*' }).DriveLetter
if (-not $cd) {
    # Fallback: try to find any CD-ROM with virtio files
    $cds = Get-CimInstance Win32_CDROMDrive | ForEach-Object { $_.Drive }
    foreach ($d in $cds) {
        if (Test-Path "${d}:\w11\amd64") {
            $cd = $d
            break
        }
    }
}
if (-not $cd) { throw "virtio-win CD-ROM not found — attach virtio-win.iso as CD-ROM" }

# Install all drivers from w11/amd64
$driverDir = "${cd}:\w11\amd64"
if (-not (Test-Path $driverDir)) {
    # Try recursive search for w11/amd64
    $found = Get-ChildItem -Path "${cd}:\" -Recurse -Directory -Filter "amd64" | Where-Object { $_.Parent.Name -eq 'w11' } | Select-Object -First 1
    if ($found) { $driverDir = $found.FullName }
    else { throw "w11\amd64 not found on virtio-win CD-ROM" }
}

$installed = 0
$failed = 0
Get-ChildItem -Path $driverDir -Filter "*.inf" -Recurse | ForEach-Object {
    $result = pnputil -a $_.FullName 2>&1
    if ($LASTEXITCODE -eq 0 -or ($result -join ' ') -match 'successfully') {
        $installed++
    } else {
        $failed++
    }
}
Log-Progress "virtio drivers: $installed installed, $failed failed"
"#;
    elevated::run_elevated(winrm, script, 300, None)?;
    Ok(())
}

/// Configure rustup default-host to x86_64 on ARM64 Windows hosts.
fn configure_rustup_arm64(session: &mut VmSession) -> Result<()> {
    // Only needed on ARM64 hosts
    let arch = crate::ssh::exec(session, "$env:PROCESSOR_ARCHITECTURE")?;
    if !arch.contains("ARM64") {
        return Ok(());
    }

    let script = r#"
$candidates = @(
  'D:\mise\installs\rust\stable\rustup.exe',
  "$env:USERPROFILE\.local\share\mise\installs\rust\stable\rustup.exe"
)
$rustup = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($rustup) {
  & $rustup set default-host x86_64-pc-windows-msvc
  & $rustup default --force-non-host stable-x86_64-pc-windows-msvc
}
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Install development tools via bootstrap mise.toml.
fn install_tools(session: &mut VmSession) -> Result<()> {
    let mise_toml_content = BOOTSTRAP_MISE_TOML.replace("'", "''");
    let script = format!(
        r#"
$mise_toml = @"
{mise_toml_content}
"@
Set-Content -Path "$env:TEMP\bootstrap-mise.toml" -Value $mise_toml -Encoding UTF8
$env:MISE_CONFIG_FILE = "$env:TEMP\bootstrap-mise.toml"
mise install
Remove-Item "$env:TEMP\bootstrap-mise.toml" -ErrorAction SilentlyContinue
"#
    );
    crate::ssh::exec(session, &script)?;

    // Verify rust is available
    crate::ssh::exec(session, "mise exec -- rustc --version")?;
    Ok(())
}

/// Set nushell as the default interactive shell on Windows.
fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    let script = r#"
$nu_path = (Get-Command nu -ErrorAction SilentlyContinue).Source
if ($nu_path) {
    # AutoRun makes nu start for every cmd session (including SSH)
    $reg_path = 'HKCU:\Software\Microsoft\Command Processor'
    if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
    Set-ItemProperty -Path $reg_path -Name 'AutoRun' -Value "nu" -Force

    # Also set PSProfile to auto-start nu for PowerShell sessions
    $psProfile = "$env:USERPROFILE\Documents\PowerShell\Microsoft.PowerShell_profile.ps1"
    $psDir = Split-Path $psProfile
    if (-not (Test-Path $psDir)) { New-Item -ItemType Directory -Path $psDir -Force }
    # Only write if it doesn't already have the mise PATH injection
    $content = Get-Content $psProfile -ErrorAction SilentlyContinue
    if ($content -notmatch 'mise.*PATH') {
        Add-Content $psProfile "`n`$env:PATH = `"$env:USERPROFILE\.local\bin;$env:PATH`"" -Encoding UTF8
    }
}
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Write the bootstrap completion marker.
fn write_bootstrap_marker(session: &mut VmSession) -> Result<()> {
    crate::ssh::exec(
        session,
        r#"New-Item -Path "$env:USERPROFILE\.testbed-bootstrapped" -ItemType File -Force"#,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bootstrap_mise_toml_has_required_tools() {
        let toml = crate::bootstrap::BOOTSTRAP_MISE_TOML;
        assert!(toml.contains("rust"));
        assert!(toml.contains("nu"));
        assert!(toml.contains("cargo:cargo-binstall"));
        assert!(toml.contains("cargo:sccache"));
        assert!(toml.contains("cargo:tauri-cli"));
    }
}
