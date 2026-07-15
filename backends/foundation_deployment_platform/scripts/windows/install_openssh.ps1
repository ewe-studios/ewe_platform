# Install OpenSSH Server — find or install, then register service.
# sshd_config is handled separately by configure_sshd.ps1.
$ErrorActionPreference = 'Continue'

$sshdExe = $null
$sshdDir = $null

# 1. Pre-installed on Vagrant images
if (Test-Path 'C:\Program Files\OpenSSH-Win64\sshd.exe') {
    $sshdExe = 'C:\Program Files\OpenSSH-Win64\sshd.exe'
    $sshdDir = 'C:\Program Files\OpenSSH-Win64'
}
elseif (Test-Path 'C:\Windows\System32\OpenSSH\sshd.exe') {
    $sshdExe = 'C:\Windows\System32\OpenSSH\sshd.exe'
    $sshdDir = 'C:\Windows\System32\OpenSSH'
}
else {
    # 2. Install via Windows Capability
    $cap = Get-WindowsCapability -Online | Where-Object { $_.Name -like 'OpenSSH.Server*' }
    if (-not $cap) { throw "OpenSSH.Server capability not available" }
    Add-WindowsCapability -Online -Name $cap.Name

    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Seconds 5
        if (Test-Path 'C:\Program Files\OpenSSH-Win64\sshd.exe') {
            $sshdExe = 'C:\Program Files\OpenSSH-Win64\sshd.exe'
            $sshdDir = 'C:\Program Files\OpenSSH-Win64'
            break
        }
        if (Test-Path 'C:\Windows\System32\OpenSSH\sshd.exe') {
            $sshdExe = 'C:\Windows\System32\OpenSSH\sshd.exe'
            $sshdDir = 'C:\Windows\System32\OpenSSH'
            break
        }
    }
    if (-not $sshdExe) {
        throw "sshd.exe did not appear after Windows Capability install (5 min timeout)"
    }
}

# 3. Add sshd directory to Machine PATH
$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($sshdDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$sshdDir;$machinePath", 'Machine')
}

# 4. Register sshd service if missing
$svc = Get-Service sshd -ErrorAction SilentlyContinue
if (-not $svc) {
    sc.exe create sshd binPath= "`"$sshdExe`"" start= auto DisplayName= "OpenSSH SSH Server" 2>&1 | Out-Null
    Start-Sleep -Seconds 2
    $svc = Get-Service sshd -ErrorAction SilentlyContinue
    if (-not $svc) {
        New-Service -Name sshd -BinaryPathName $sshdExe -DisplayName "OpenSSH SSH Server" -StartupType Automatic 2>&1 | Out-Null
        Start-Sleep -Seconds 2
        $svc = Get-Service sshd -ErrorAction SilentlyContinue
    }
    if (-not $svc) {
        throw "Could not register sshd service via sc.exe or New-Service"
    }
}

Set-Service -Name sshd -StartupType Automatic
Start-Service sshd -ErrorAction SilentlyContinue
