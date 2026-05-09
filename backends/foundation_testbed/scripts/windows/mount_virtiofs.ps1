# Mount virtiofs project directory — called by scheduled task at every boot.
# Removes stale mount point, then starts virtiofs.exe as a background daemon.
$ErrorActionPreference = 'Stop'

$mountPoint = 'C:\Users\vagrant\project'
$maxRetries = 3
$retryDelay = 5

# Find virtiofs.exe
$found = $null
foreach ($p in @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)) {
    if (Test-Path $p) { $found = $p; break }
}
if (-not $found) {
    throw "virtiofs.exe not found - virtio-win drivers not installed"
}

# Remove stale mount point — virtiofs.exe creates its own (error 183 if exists)
if (Test-Path $mountPoint) {
    Remove-Item -Path $mountPoint -Recurse -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

# Try mount with retries
$mounted = $false
for ($attempt = 1; $attempt -le $maxRetries; $attempt++) {
    Write-Output "Mount attempt $attempt of $maxRetries..."

    # Start virtiofs.exe as background daemon
    $proc = Start-Process -FilePath $found -ArgumentList "-t project -m $mountPoint" -WindowStyle Hidden -PassThru

    # Wait for mount to be accessible
    Start-Sleep -Seconds 2

    for ($wait = 0; $wait -lt 10; $wait++) {
        if (Test-Path $mountPoint) {
            # Verify it's actually accessible
            try {
                $testFile = Join-Path $mountPoint "Cargo.toml"
                if (Test-Path $testFile) {
                    $mounted = $true
                    Write-Output "Mount successful at $mountPoint"
                    break
                }
            } catch {
                # Continue waiting
            }
        }
        Start-Sleep -Seconds 1
    }

    if ($mounted) { break }

    Write-Output "Mount attempt $attempt failed, cleaning up and retrying..."
    # Kill any lingering virtiofs process
    Get-Process -Name "virtiofs" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path $mountPoint) {
        Remove-Item -Path $mountPoint -Recurse -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds $retryDelay
}

if (-not $mounted) {
    throw "Mount failed after $maxRetries attempts"
}

Write-Output "Mount verified and ready"
