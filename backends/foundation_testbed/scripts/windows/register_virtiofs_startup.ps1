# Register a persistent scheduled task that auto-mounts virtiofs on every Windows startup.
# Also runs the mount immediately for the current boot.
$ErrorActionPreference = 'Continue'

$taskName = 'FoundationTestbed_VirtiofsMount'
$mountScript = 'C:\Users\vagrant\mount_virtiofs.ps1'

# Find virtiofs.exe — FAST PATH: check common locations first
$found = $null
$commonPaths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)

foreach ($p in $commonPaths) {
    if (Test-Path $p) {
        $found = $p
        break
    }
}

# FALLBACK: Search registry if not found
if (-not $found) {
    $reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer' -ErrorAction SilentlyContinue
    if ($reg -and $reg.InstallLocation) {
        $regPath = Join-Path $reg.InstallLocation "VioFS\virtiofs.exe"
        if (Test-Path $regPath) {
            $found = $regPath
        }
    }
}

if (-not $found) {
    $virtioEntry = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
                   Where-Object { $_.DisplayName -like '*virtio*' } |
                   Select-Object -First 1
    if ($virtioEntry -and $virtioEntry.InstallLocation) {
        $regPath = Join-Path $virtioEntry.InstallLocation "VioFS\virtiofs.exe"
        if (Test-Path $regPath) {
            $found = $regPath
        }
    }
}

if (-not $found) {
    throw "virtiofs.exe not found - virtio-win drivers not installed. Checked common paths and registry."
}

# Write mount script to VM - embedded script also uses fast-path then registry fallback
$mountScriptContent = @'
$ErrorActionPreference = 'Continue'
$mountPoint = 'C:\Users\vagrant\project'
$maxRetries = 3

# FAST PATH: Check common locations
$found = $null
$commonPaths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)

foreach ($p in $commonPaths) {
    if (Test-Path $p) { $found = $p; break }
}

# FALLBACK: Search registry
if (-not $found) {
    $reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer' -ErrorAction SilentlyContinue
    if ($reg -and $reg.InstallLocation) {
        $testPath = Join-Path $reg.InstallLocation "VioFS\virtiofs.exe"
        if (Test-Path $testPath) { $found = $testPath }
    }
}

if (-not $found) {
    $virtioEntry = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
                   Where-Object { $_.DisplayName -like '*virtio*' } |
                   Select-Object -First 1
    if ($virtioEntry -and $virtioEntry.InstallLocation) {
        $testPath = Join-Path $virtioEntry.InstallLocation "VioFS\virtiofs.exe"
        if (Test-Path $testPath) { $found = $testPath }
    }
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
# /RU SYSTEM = run as SYSTEM user (no password needed, works reliably)
$result = schtasks /Create /TN $taskName /TR "powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$mountScript`"" /SC ONSTART /RU SYSTEM /F 2>&1
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create scheduled task: $result"
}
Write-Output "Scheduled task '$taskName' registered."

# Run mount task immediately for the current boot
schtasks /Run /TN $taskName 2>&1 | Out-Null
Write-Output "Mount task started."
