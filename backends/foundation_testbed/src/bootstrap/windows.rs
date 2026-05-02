//! Windows bootstrap via WinRM + SSH.
//!
//! Steps: OpenSSH server → SSH keys → registry → mise → tools → nushell → marker.

use std::thread;
use std::time::Duration;

use crate::bootstrap::BOOTSTRAP_MISE_TOML;
use crate::config::{Result, VmProfile};
use crate::ssh::VmSession;
use crate::winrm::WinRM;
use crate::winrm::elevated;

/// Bootstrap a Windows VM with development tools.
pub fn bootstrap_windows(profile: &VmProfile, winrm: &WinRM, session: &mut VmSession) -> Result<()> {
    // Step 1: Install OpenSSH Server via WinRM elevated
    install_openssh_server(winrm)?;

    // Step 2: Configure SSH key authorization
    setup_ssh_keys(profile, session, winrm)?;

    // Step 3: Set LocalAccountTokenFilterPolicy
    set_local_account_token_filter(winrm)?;

    // Step 4: Give SSH time to reconfigure after changes
    thread::sleep(Duration::from_secs(5));

    // Step 5: Install mise via PowerShell
    install_mise(session)?;

    // Step 6: Write bootstrap mise.toml and run mise install
    install_tools(session)?;

    // Step 7: Set nushell as default shell
    set_nushell_default_shell(session)?;

    // Step 8: Write bootstrap marker
    write_bootstrap_marker(session)?;

    Ok(())
}

/// Install OpenSSH Server via Windows Capability.
fn install_openssh_server(winrm: &WinRM) -> Result<()> {
    let script = r#"
$cap = Get-WindowsCapability -Online | Where-Object { $_.Name -like 'OpenSSH.Server*' }
if ($cap.State -ne 'Installed') {
    Add-WindowsCapability -Online -Name $cap.Name
}

# Configure sshd for password authentication
$sshd_config = 'C:\ProgramData\ssh\sshd_config'
$config = Get-Content $sshd_config
$config = $config | ForEach-Object {
    if ($_ -match '^#?PasswordAuthentication') { 'PasswordAuthentication yes' }
    elseif ($_ -match '^#?PubkeyAuthentication') { 'PubkeyAuthentication yes' }
    else { $_ }
}
Set-Content $sshd_config $config -Encoding UTF8

# Restart sshd if running
Restart-Service sshd -ErrorAction SilentlyContinue
Start-Service sshd -ErrorAction SilentlyContinue
"#;

    elevated::run_elevated(winrm, script, 120)?;
    Ok(())
}

/// Set up SSH key authorization on Windows.
fn setup_ssh_keys(_profile: &VmProfile, session: &mut VmSession, _winrm: &WinRM) -> Result<()> {
    // Get host public key
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
    let pub_key_path = home.join(".ssh").join("id_ed25519.pub");

    let pub_key = if pub_key_path.exists() {
        std::fs::read_to_string(&pub_key_path).unwrap_or_default()
    } else {
        // Try rsa
        let rsa_pub = home.join(".ssh").join("id_rsa.pub");
        if rsa_pub.exists() {
            std::fs::read_to_string(&rsa_pub).unwrap_or_default()
        } else {
            String::new()
        }
    };

    if !pub_key.is_empty() {
        let key_escaped = pub_key.replace("'", "''");
        let script = format!(
            r#"
$ssh_dir = "$env:USERPROFILE\.ssh"
if (-not (Test-Path $ssh_dir)) {{ mkdir $ssh_dir -Force }}
$auth_file = "$ssh_dir\authorized_keys"
$key = '{key_escaped}'
if (-not (Select-String -Path $auth_file -Pattern $key -Quiet -ErrorAction SilentlyContinue)) {{
    Add-Content $auth_file $key
}}
# Also add to administrators_authorized_keys for admin users
$admin_keys = 'C:\ProgramData\ssh\administrators_authorized_keys'
if (-not (Select-String -Path $admin_keys -Pattern $key -Quiet -ErrorAction SilentlyContinue)) {{
    Add-Content $admin_keys $key
}}
"#
        );
        crate::ssh::exec(session, &script)?;
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

    elevated::run_elevated(winrm, script, 30)?;
    Ok(())
}

/// Install mise on Windows via PowerShell installer.
fn install_mise(session: &mut VmSession) -> Result<()> {
    let script = r#"
if (-not (Get-Command mise -ErrorAction SilentlyContinue)) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    iwr -useb https://mise.run | iex
    $mise_bin = "$env:USERPROFILE\.local\bin\mise.exe"
    if (Test-Path $mise_bin) {
        $env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
        [Environment]::SetEnvironmentVariable('PATH', "$env:USERPROFILE\.local\bin;$env:PATH", 'User')
    }
}
"#;
    crate::ssh::exec(session, script)?;
    Ok(())
}

/// Install development tools via bootstrap mise.toml.
fn install_tools(session: &mut VmSession) -> Result<()> {
    // Write bootstrap mise.toml to a temp location
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

    // Verify key tools are installed
    crate::ssh::exec(session, "nu -c 'mise exec -- rustc --version'")?;

    Ok(())
}

/// Set nushell as the default interactive shell on Windows.
fn set_nushell_default_shell(session: &mut VmSession) -> Result<()> {
    let script = r#"
$nu_path = (Get-Command nu -ErrorAction SilentlyContinue).Source
if ($nuPath) {
    # Set as default shell for this user via registry
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
