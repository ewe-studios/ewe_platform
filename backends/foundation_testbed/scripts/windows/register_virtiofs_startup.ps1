# Register a persistent scheduled task that auto-mounts virtiofs on every Windows startup.
# Also runs the mount immediately for the current boot.
$ErrorActionPreference = 'Continue'

$taskName = 'FoundationTestbed_VirtiofsMount'
$mountScript = 'C:\Users\vagrant\mount_virtiofs.ps1'

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

# Write mount script to VM
$mountScriptContent = @'
$ErrorActionPreference = 'Continue'
$mountPoint = 'C:\Users\vagrant\project'
$maxRetries = 3

$found = $null
foreach ($p in @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)) {
    if (Test-Path $p) { $found = $p; break }
}
if (-not $found) {
    throw "virtiofs.exe not found"
}

# Remove stale mount point
if (Test-Path $mountPoint) {
    Remove-Item -Path $mountPoint -Recurse -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

# Try mount with retries
$mounted = $false
for ($attempt = 1; $attempt -le $maxRetries; $attempt++) {
    Start-Process -FilePath $found -ArgumentList "-t project -m $mountPoint" -WindowStyle Hidden
    Start-Sleep -Seconds 3

    # Verify mount is accessible
    for ($i = 0; $i -lt 10; $i++) {
        if (Test-Path $mountPoint) {
            $mounted = $true
            break
        }
        Start-Sleep -Seconds 1
    }
    if ($mounted) { break }

    # Clean up and retry
    Get-Process -Name "virtiofs" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path $mountPoint) {
        Remove-Item -Path $mountPoint -Recurse -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds 2
}

if (-not $mounted) {
    throw "Mount failed after $maxRetries attempts"
}
'@

Set-Content -Path $mountScript -Value $mountScriptContent -Encoding UTF8

# Delete existing task (idempotent — ignore errors)
schtasks /Delete /TN $taskName /F 2>&1 | Out-Null

# Create the scheduled task
# /SC ONSTART = run at every boot
# /RU vagrant /RP vagrant = run as vagrant user
# /IT = interact with desktop (required for WinFsp mount visibility)
$result = schtasks /Create /TN $taskName /TR "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$mountScript`"" /SC ONSTART /RU vagrant /RP vagrant /IT /F 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create scheduled task: $result"
}
Write-Output "Scheduled task '$taskName' registered."

# Run mount task immediately for the current boot
schtasks /Run /TN $taskName /I 2>&1 | Out-Null
Write-Output "Mount task started."
