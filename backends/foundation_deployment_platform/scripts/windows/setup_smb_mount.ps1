# Mount the QEMU built-in SMB share as project mount on Windows guest.
# QEMU's user-mode netdev exposes a built-in SMB server at \\10.0.2.4\qemu
# when the `smb=` option is passed. No host smbd or port forwarding needed.

$ErrorActionPreference = 'Stop'

$smbServer = '10.0.2.4'
$shareName = 'qemu'
$mountLetter = 'Z'

# Clean up any existing drive
$existing = Get-PSDrive -Name $mountLetter -ErrorAction SilentlyContinue
if ($existing) {
    Write-Output "Unmounting existing $mountLetter`:"
    Remove-PSDrive -Name $mountLetter -Force -ErrorAction SilentlyContinue | Out-Null
    Start-Sleep -Seconds 1
}

# Remove any existing network mapping for this UNC
$unc = "\\$smbServer\$shareName"
net use "${mountLetter}:" /delete /y 2>$null | Out-Null

# Wait for SMB connectivity (QEMU user-mode net can take a moment after boot)
Write-Output "Waiting for SMB server at $smbServer..."
$connected = $false
for ($i = 0; $i -lt 15; $i++) {
    if (Test-Connection -ComputerName $smbServer -Count 1 -Quiet -ErrorAction SilentlyContinue) {
        Write-Output "SMB server reachable"
        $connected = $true
        break
    }
    Write-Output "  Retry $i..."
    Start-Sleep -Seconds 2
}

if (-not $connected) {
    throw "SMB server at $smbServer not reachable after 30 seconds"
}

# Map the network drive
Write-Output "Mounting $unc as ${mountLetter}:"

# Try with net use first (more reliable)
$result = net use "${mountLetter}:" "$unc" /persistent:no 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Output "net use failed: $result"

    # Fallback: New-PSDrive
    Write-Output "Trying New-PSDrive..."
    try {
        New-PSDrive -Name $mountLetter -PSProvider FileSystem -Root $unc -Scope Global -Persist | Out-Null
    } catch {
        throw "Failed to map drive with both net use and New-PSDrive: $_"
    }
}

# Wait a moment for mount to settle
Start-Sleep -Seconds 2

# Verify mount exists
if (-not (Test-Path "${mountLetter}:")) {
    throw "Drive ${mountLetter}: not accessible after mapping"
}

# List directory contents for debugging
$files = Get-ChildItem "${mountLetter}:" -ErrorAction SilentlyContinue | Select-Object -First 5 | ForEach-Object { $_.Name }
Write-Output "Share contents: $($files -join ', ')"

# Verify Cargo.toml exists (indicator of project root)
if (-not (Test-Path "${mountLetter}:\Cargo.toml")) {
    Write-Warning "Cargo.toml not found at ${mountLetter}:\ - mount may be incomplete"
    # Don't throw - mount succeeded, just might be empty or different content
}

Write-Output "SMB mount successful at ${mountLetter}:"
