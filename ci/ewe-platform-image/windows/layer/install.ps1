# ewe_platform's provisioning layer for the Windows CI VM. Run inside the VM
# by platform-images' provision.ps1 (Windows PowerShell 5.1) after layer 1
# has installed MSVC, LLVM, the Vulkan SDK, mise, nushell and rustup.
#
#   - the toolchain rust-toolchain.toml pins (+ clippy), set as the default
#   - the tools in the repository's mise.toml, from its lock
#   - the submodules at their gitlink SHAs under C:\ewe\submodules, and
#     PLATFORM_IMAGES_SUBMODULES_LOCK for link-submodules.nu

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module 'C:\OEM\PlatformImages.psm1' -Force

$Here = $PSScriptRoot
$Scripts = 'C:\OEM\scripts'

Write-PiLog 'ewe_platform: Rust toolchain'
Invoke-Native -FilePath 'nu' -ArgumentList @((Join-Path $Scripts 'install-rust-toolchain.nu'), (Join-Path $Here 'files\rust-toolchain.toml'), '--components', 'clippy') | Out-Null

Write-PiLog 'ewe_platform: repository tools'
Install-MiseTools -ConfigDir (Join-Path $Here 'mise')

Write-PiLog 'ewe_platform: submodules'
$dest = 'C:\ewe\submodules'
Invoke-Native -FilePath 'nu' -ArgumentList @((Join-Path $Scripts 'bake-submodules.nu'), (Join-Path $Here 'submodules.lock.json'), $dest) | Out-Null
Set-MachineEnv -Name PLATFORM_IMAGES_SUBMODULES_LOCK -Value (Join-Path $dest 'submodules.lock.json')
