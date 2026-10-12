# platform-images: provision a fresh Windows VM for CI (layer 1), then run
# every repository layer (C:\OEM\layers\<name>\install.ps1, in name order).
# Started by install.bat at the end of dockurr's unattended install, under
# Windows PowerShell 5.1. Any failure stops provisioning and is reported by
# install.bat (C:\OEM\status\provision.failed + provision.log).
#
# Installed (versions and checksums in components.json, tools in mise\):
#   OpenSSH Server (key from C:\OEM\ssh\authorized_keys, default shell pwsh)
#   PowerShell 7, Git for Windows (+ bash), Visual Studio 2022 Build Tools
#   (MSVC + Windows SDK), LLVM/clang (LIBCLANG_PATH), Vulkan SDK (VULKAN_SDK),
#   WebView2 runtime, mise + the shared tools (nushell, node, python, cmake,
#   ninja, protoc, sccache, ...), rustup without a toolchain
#   (RUSTUP_HOME/CARGO_HOME under C:\rust), the GitHub Actions runner
#   (C:\actions-runner, registered per job with a JIT config).

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
Set-StrictMode -Version Latest

$Oem = $PSScriptRoot
Import-Module (Join-Path $Oem 'PlatformImages.psm1') -Force
$C = Get-Content (Join-Path $Oem 'components.json') -Raw | ConvertFrom-Json
$Root = 'C:\platform-images'
New-Item -ItemType Directory -Force -Path $Root | Out-Null

Write-PiLog 'provisioning starts'

# --- OpenSSH first, so a failure later on can be inspected over SSH --------
Write-PiLog 'OpenSSH server'
$cap = Get-WindowsCapability -Online -Name 'OpenSSH.Server*'
if ($cap.State -ne 'Installed') { Add-WindowsCapability -Online -Name $cap.Name | Out-Null }
Set-Service -Name sshd -StartupType Automatic
Start-Service sshd
if (-not (Get-NetFirewallRule -Name 'platform-images-sshd' -ErrorAction SilentlyContinue)) {
    New-NetFirewallRule -Name 'platform-images-sshd' -DisplayName 'OpenSSH Server (platform-images)' `
        -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 | Out-Null
}
$keys = Join-Path $Oem 'ssh\authorized_keys'
if (Test-Path $keys) {
    # The VM user is an administrator: sshd reads administrators_authorized_keys,
    # which must be writable only by Administrators and SYSTEM.
    $dest = Join-Path $env:ProgramData 'ssh\administrators_authorized_keys'
    Copy-Item -Force $keys $dest
    Invoke-Native -FilePath 'icacls.exe' -ArgumentList @($dest, '/inheritance:r', '/grant', 'Administrators:F', '/grant', 'SYSTEM:F') | Out-Null
    Write-PiLog 'authorised the SSH key from C:\OEM\ssh\authorized_keys'
} else {
    Write-PiLog 'no C:\OEM\ssh\authorized_keys: SSH key login is not configured'
}

# --- CI tuning --------------------------------------------------------------
# Long paths (deep cargo target trees), and no Windows Update reboots in the
# middle of a job.
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\FileSystem' -Name LongPathsEnabled -Value 1 -Type DWord
Stop-Service -Name wuauserv -Force
Set-Service -Name wuauserv -StartupType Disabled
# Real-time scanning of build output costs a lot on Windows. Turning it off
# fails when Tamper Protection is on (client editions); that only costs
# speed, so it is reported, not fatal.
try {
    Set-MpPreference -DisableRealtimeMonitoring $true
    Add-MpPreference -ExclusionPath 'C:\'
    Write-PiLog 'Defender real-time scanning off for this CI VM'
} catch {
    Write-PiLog ("WARNING: could not turn off Defender real-time scanning ({0}); builds will be slower" -f $_.Exception.Message)
}

# --- PowerShell 7 -----------------------------------------------------------
$f = Get-VerifiedDownload -Name 'PowerShell 7' -Component $C.pwsh -FileName 'pwsh.msi'
Invoke-Native -FilePath 'msiexec.exe' -ArgumentList @('/i', $f, '/qn', '/norestart', 'ADD_PATH=1', 'ENABLE_PSREMOTING=0') -AllowedExitCodes @(0, 3010) | Out-Null
$Pwsh = 'C:\Program Files\PowerShell\7\pwsh.exe'
if (-not (Test-Path $Pwsh)) { throw "PowerShell 7 is not at $Pwsh" }
Add-MachinePath 'C:\Program Files\PowerShell\7'
New-ItemProperty -Path 'HKLM:\SOFTWARE\OpenSSH' -Name DefaultShell -Value $Pwsh -PropertyType String -Force | Out-Null

# --- Git for Windows (git, bash for `shell: bash` steps) --------------------
$f = Get-VerifiedDownload -Name 'Git for Windows' -Component $C.git -FileName 'git.exe'
Invoke-Native -FilePath $f -ArgumentList @('/VERYSILENT', '/NORESTART', '/NOCANCEL', '/SP-', '/SUPPRESSMSGBOXES', '/o:PathOption=Cmd') | Out-Null
Add-MachinePath 'C:\Program Files\Git\cmd'
Add-MachinePath 'C:\Program Files\Git\bin'
Invoke-Native -FilePath 'C:\Program Files\Git\cmd\git.exe' -ArgumentList @('config', '--system', 'core.longpaths', 'true') | Out-Null

# --- Visual Studio 2022 Build Tools (MSVC, Windows SDK) ---------------------
$f = Get-VerifiedDownload -Name 'VS Build Tools' -Component $C.vs_buildtools -FileName 'vs_buildtools.exe'
$vsArgs = @('--quiet', '--wait', '--norestart', '--nocache', '--includeRecommended')
foreach ($w in $C.vs_buildtools.workloads) { $vsArgs += @('--add', $w) }
Invoke-Native -FilePath $f -ArgumentList $vsArgs -AllowedExitCodes @(0, 3010) | Out-Null

# --- LLVM (clang, libclang for bindgen) -------------------------------------
$f = Get-VerifiedDownload -Name 'LLVM' -Component $C.llvm -FileName 'llvm.exe'
Invoke-Native -FilePath $f -ArgumentList @('/S') | Out-Null
if (-not (Test-Path 'C:\Program Files\LLVM\bin\libclang.dll')) { throw 'LLVM did not install libclang.dll' }
Add-MachinePath 'C:\Program Files\LLVM\bin'
Set-MachineEnv -Name LIBCLANG_PATH -Value 'C:\Program Files\LLVM\bin'

# --- Vulkan SDK (headers, loader lib, glslc: llama.cpp's Vulkan backend) ----
$f = Get-VerifiedDownload -Name 'Vulkan SDK' -Component $C.vulkan_sdk -FileName 'vulkansdk.exe'
$vkRoot = 'C:\VulkanSDK\' + $C.vulkan_sdk.version
Invoke-Native -FilePath $f -ArgumentList @('--root', $vkRoot, '--accept-licenses', '--default-answer', '--confirm-command', 'install') | Out-Null
if (-not (Test-Path (Join-Path $vkRoot 'Bin\glslc.exe'))) { throw "the Vulkan SDK has no glslc under $vkRoot" }
Set-MachineEnv -Name VULKAN_SDK -Value $vkRoot
Add-MachinePath (Join-Path $vkRoot 'Bin')

# --- WebView2 runtime (Tauri/wry) -------------------------------------------
$f = Get-VerifiedDownload -Name 'WebView2' -Component $C.webview2 -FileName 'webview2.exe'
Invoke-Native -FilePath $f -ArgumentList @('/silent', '/install') | Out-Null

# --- mise and the shared tools ----------------------------------------------
$f = Get-VerifiedDownload -Name 'mise' -Component $C.mise -FileName 'mise.zip'
Expand-Archive -Path $f -DestinationPath (Join-Path $Root 'mise-dist') -Force
$miseExe = Get-ChildItem -Path (Join-Path $Root 'mise-dist') -Recurse -Filter 'mise.exe' | Select-Object -First 1
if ($null -eq $miseExe) { throw 'mise.exe not found in the mise archive' }
Add-MachinePath $miseExe.DirectoryName
Set-MachineEnv -Name MISE_DATA_DIR -Value (Join-Path $Root 'mise-data')
Set-MachineEnv -Name MISE_CACHE_DIR -Value (Join-Path $Root 'mise-cache')
Set-MachineEnv -Name MISE_VERSION_CHECK -Value '0'
Install-MiseTools -ConfigDir (Join-Path $Oem 'mise')

# --- rustup (no toolchain: repository layers install theirs) ----------------
Set-MachineEnv -Name RUSTUP_HOME -Value 'C:\rust\rustup'
Set-MachineEnv -Name CARGO_HOME -Value 'C:\rust\cargo'
$f = Get-VerifiedDownload -Name 'rustup' -Component $C.rustup -FileName 'rustup-init.exe'
Invoke-Native -FilePath $f -ArgumentList @('-y', '--no-modify-path', '--profile', 'minimal', '--default-toolchain', 'none') | Out-Null
Add-MachinePath 'C:\rust\cargo\bin'

# --- GitHub Actions runner (registered per job; see vm.nu runner) -----------
$f = Get-VerifiedDownload -Name 'actions runner' -Component $C.actions_runner -FileName 'actions-runner.zip'
Expand-Archive -Path $f -DestinationPath 'C:\actions-runner' -Force
if (-not (Test-Path 'C:\actions-runner\run.cmd')) { throw 'the actions runner has no run.cmd' }

Set-MachineEnv -Name PLATFORM_IMAGES_HOME -Value $Oem

# --- Repository layers --------------------------------------------------------
$layers = Join-Path $Oem 'layers'
if (Test-Path $layers) {
    foreach ($layer in (Get-ChildItem -Path $layers -Directory | Sort-Object Name)) {
        $script = Join-Path $layer.FullName 'install.ps1'
        if (-not (Test-Path $script)) { throw ("layer {0} has no install.ps1" -f $layer.Name) }
        Write-PiLog ("layer {0}" -f $layer.Name)
        & $script
    }
}

# --- Record what is installed -----------------------------------------------
$versions = Join-Path $Oem 'status\versions.txt'
New-Item -ItemType Directory -Force -Path (Split-Path $versions) | Out-Null
$lines = @()
foreach ($cmd in @('git --version', 'pwsh --version', 'clang --version', 'nu --version', 'node --version', 'python --version', 'cmake --version', 'rustup --version')) {
    $parts = $cmd -split ' '
    $out = & $parts[0] $parts[1..($parts.Length - 1)] 2>&1 | Select-Object -First 1
    if ($LASTEXITCODE -ne 0) { throw ("{0} failed after provisioning" -f $cmd) }
    $lines += ("{0}: {1}" -f $parts[0], $out)
}
$lines | Set-Content -Path $versions
Write-PiLog 'provisioning done'
$lines | ForEach-Object { Write-PiLog $_ }
