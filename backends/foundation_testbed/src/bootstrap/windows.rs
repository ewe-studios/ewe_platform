//! Windows bootstrap via WinRM + SSH — step-by-step idempotent.
//!
//! Each step checks concrete artifacts before running. If bootstrap fails
//! mid-way, re-running picks up where it left off.

use std::thread;
use std::time::Instant;

use crate::bootstrap::BOOTSTRAP_MISE_TOML;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;
use crate::winrm::WinRM;
use crate::winrm::elevated;

/// Bootstrap a Windows VM with development tools.
pub fn bootstrap_windows(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession) -> Result<()> {
    // Step 1: OpenSSH Server
    step("install OpenSSH Server", || {
        install_openssh_server(winrm)
    })?;

    // Step 2: SSH key authorization (both user + admin paths)
    step("authorise host SSH key", || {
        setup_ssh_keys(profile, session, winrm)
    })?;

    // Step 3: LocalAccountTokenFilterPolicy
    step("set LocalAccountTokenFilterPolicy", || {
        set_local_account_token_filter(winrm)
    })?;

    // Give SSH time to reconfigure after changes
    thread::sleep(std::time::Duration::from_secs(5));

    // Step 4: mise
    step("install mise", || {
        install_mise(session)
    })?;

    // Step 5: cargo-binstall (before tools, so mise can use it)
    step("install cargo-binstall", || {
        install_cargo_binstall(session)
    })?;

    // Step 6: configure mise cargo_binstall setting
    step("configure mise cargo_binstall", || {
        configure_mise_cargo_binstall(session)
    })?;

    // Step 7: VS Build Tools with C++ workload
    step("install VS Build Tools", || {
        install_vs_build_tools(session, winrm)
    })?;

    // Step 8: WebView2 Runtime
    step("install WebView2 Runtime", || {
        install_webview2(session)
    })?;

    // Step 9: Windows Defender exclusions for dev directories
    step("set Windows Defender exclusions", || {
        set_defender_exclusions(session)
    })?;

    // Step 10: rustup ARM64 default-host workaround (only on ARM64 hosts)
    step("configure rustup for x86_64 (ARM64 workaround)", || {
        configure_rustup_arm64(session)
    })?;

    // Step 11: install tools via bootstrap mise.toml
    step("install tools via mise", || {
        install_tools(session)
    })?;

    // Step 12: set nushell as default shell
    step("set nushell as default shell", || {
        set_nushell_default_shell(session)
    })?;

    // Step 13: write bootstrap marker
    step("write bootstrap marker", || {
        write_bootstrap_marker(session)
    })?;

    // Silence unused warnings for profile (used by callers for routing)
    let _ = profile;
    Ok(())
}

/// Run a named bootstrap step, tracking elapsed time.
fn step<F>(label: &str, f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    let start = Instant::now();
    f()?;
    let elapsed = start.elapsed();
    let _ = (label, elapsed);
    Ok(())
}

/// Install OpenSSH Server via Windows Capability.
fn install_openssh_server(winrm: &WinRM) -> Result<()> {
    // Check if sshd is already running
    let state = winrm.run_ps(
        "Get-Service sshd -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Status",
    )?;
    if state.stdout.trim().eq_ignore_ascii_case("Running") {
        return Ok(());
    }

    let script = r#"
$cap = Get-WindowsCapability -Online | Where-Object { $_.Name -like 'OpenSSH.Server*' }
if ($cap.State -ne 'Installed') {
    Add-WindowsCapability -Online -Name $cap.Name
}

# Configure sshd for password + pubkey auth
$sshd_config = 'C:\ProgramData\ssh\sshd_config'
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

Start-Service sshd -ErrorAction SilentlyContinue
Set-Service -Name sshd -StartupType Automatic
Restart-Service sshd -ErrorAction SilentlyContinue
"#;

    elevated::run_elevated(winrm, script, 360)?;
    Ok(())
}

/// Set up SSH key authorization on Windows (both user + admin paths).
fn setup_ssh_keys(_profile: &VmProfile, session: &mut VmSession, _winrm: &WinRM) -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
    let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
    let mut pub_key = String::new();

    for key_name in key_names {
        let path = home.join(".ssh").join(key_name);
        if path.exists() {
            if let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key.trim().to_string();
                break;
            }
        }
    }

    if pub_key.is_empty() {
        return Ok(());
    }

    let key_escaped = pub_key.replace("'", "''");
    let script = format!(
        r#"
$key = '{key_escaped}'

# User-level path
$dir = "$env:USERPROFILE\.ssh"
if (-not (Test-Path $dir)) {{ New-Item -ItemType Directory -Path $dir -Force | Out-Null }}
$f = "$dir\authorized_keys"
if (-not (Test-Path $f) -or ((Get-Content $f -ErrorAction SilentlyContinue) -notcontains $key)) {{
    Add-Content $f $key -Encoding ASCII
}}

# Admin-level path (for Match Group administrators users)
$adm = 'C:\ProgramData\ssh\administrators_authorized_keys'
if (-not (Test-Path $adm) -or ((Get-Content $adm -ErrorAction SilentlyContinue) -notcontains $key)) {{
    Add-Content $adm $key -Encoding ASCII
}}
icacls $adm /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F' | Out-Null

Restart-Service sshd -ErrorAction SilentlyContinue
"#
    );
    crate::ssh::exec(session, &script)?;
    Ok(())
}

/// Set LocalAccountTokenFilterPolicy registry key.
fn set_local_account_token_filter(winrm: &WinRM) -> Result<()> {
    let script = r#"
$reg_path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
Set-ItemProperty -Path $reg_path -Name 'LocalAccountTokenFilterPolicy' -Value 1 -Type DWord -Force
"#;
    elevated::run_elevated(winrm, script, 30)?;
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
$env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
[Environment]::SetEnvironmentVariable('PATH', "$env:USERPROFILE\.local\bin;$env:PATH", 'User')
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
# Ensure ~/.cargo/bin is on PATH for future sessions
$path = [Environment]::GetEnvironmentVariable('PATH','User')
if ($path -notmatch [regex]::Escape($dest)) {
    [Environment]::SetEnvironmentVariable('PATH', $dest + ';' + $path, 'User')
}
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
fn install_vs_build_tools(session: &mut VmSession, winrm: &WinRM) -> Result<()> {
    // Check for the actual binary we need: Hostarm64\x64\link.exe
    let vc_check = winrm.run_ps(
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
    $reg_path = 'HKCU:\Software\Microsoft\Command Processor'
    if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
    # AutoRun makes nu start for every cmd session (including SSH)
    Set-ItemProperty -Path $reg_path -Name 'AutoRun' -Value "nu" -Force
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
