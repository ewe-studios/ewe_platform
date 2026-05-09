# Mount the QEMU built-in SMB share as project mount on Windows guest.
# QEMU's user-mode netdev exposes a built-in SMB server at \\10.0.2.3\smb
# when the `smb=` option is passed. No host smbd or port forwarding needed.

$ErrorActionPreference = 'Stop'

$smbServer = '10.0.2.3'
$shareName = 'smb'
$mountLetter = 'Z'

# Clean up any existing Z: drive
$existing = Get-PSDrive -Name $mountLetter -ErrorAction SilentlyContinue
if ($existing) {
    Write-Output "Unmounting existing $mountLetter`:"
    Remove-PSDrive -Name $mountLetter -Force -ErrorAction SilentlyContinue | Out-Null
}

# Wait for SMB connectivity (QEMU user-mode net can take a moment after boot)
for ($i = 0; $i -lt 10; $i++) {
    if (Test-Connection -ComputerName $smbServer -Count 1 -Quiet -ErrorAction SilentlyContinue) {
        break
    }
    Start-Sleep -Seconds 3
}

# Map the network drive
$unc = "\\$smbServer\$shareName"
Write-Output "Mounting $unc as ${mountLetter}:"

# Try with no credentials first (QEMU's built-in SMB has no auth)
net use "${mountLetter}:" "$unc" /persistent:no 2>$null

# If net use failed, try New-PSDrive as fallback
if (-not (Test-Path "${mountLetter}:\")) {
    New-PSDrive -Name $mountLetter -PSProvider FileSystem -Root $unc -Scope Global | Out-Null
}

# Verify
if (-not (Test-Path "${mountLetter}:\Cargo.toml")) {
    $files = Get-ChildItem "${mountLetter}:" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }
    Write-Output "Share visible. Contents: $($files -join ', ')"
    throw "SMB mount failed — Cargo.toml not found at ${mountLetter}:\"
}

Write-Output "Project mount verified at ${mountLetter}:"
