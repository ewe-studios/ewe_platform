# Install WinFsp — required by virtiofs.exe for filesystem mounting.
$ErrorActionPreference = 'Stop'

# Check if already installed
$service = Get-Service -Name 'WinFsp.Launcher' -ErrorAction SilentlyContinue
if ($service) {
    Write-Output "WinFsp already installed"
    exit 0
}

# Download WinFsp installer
$installerUrl = 'https://github.com/winfsp/winfsp/releases/download/v2.0/winfsp-2.0.23075.msi'
$installerPath = "$env:TEMP\winfsp.msi"

try {
    Write-Output "Downloading WinFsp..."
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing
} catch {
    # Fallback: check if curl.exe is available
    & curl.exe -L -o $installerPath $installerUrl
    if (-not (Test-Path $installerPath)) {
        throw "Failed to download WinFsp: $_"
    }
}

Write-Output "Installing WinFsp..."
Start-Process -FilePath msiexec.exe -ArgumentList "/i", $installerPath, "/qn", "/norestart" -Wait

# Verify
$service = Get-Service -Name 'WinFsp.Launcher' -ErrorAction SilentlyContinue
if ($service -and $service.Status -eq 'Running') {
    Write-Output "WinFsp installed and running"
} else {
    throw "WinFsp service not found after installation"
}
