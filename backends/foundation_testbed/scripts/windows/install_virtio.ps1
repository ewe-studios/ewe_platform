# Install virtio-win from attached CD-ROM or downloaded ISO.
$ErrorActionPreference = 'Stop'

# 1. Try attached CD-ROM first
$cd = $null
try {
    $cd = (Get-Volume | Where-Object { $_.FileSystemLabel -like 'virtio*' }).DriveLetter
} catch {}
if (-not $cd) {
    Get-CimInstance Win32_CDROMDrive | ForEach-Object {
        if (Test-Path "$($_.Drive):\guest-tools.exe") { $cd = $_.Drive }
    }
}

if ($cd) {
    Write-Output "Using attached CD-ROM at ${cd}:"
    $installer = "${cd}:\guest-tools.exe"
} else {
    # 2. Download ISO, mount it
    $isoUrl = 'https://fedorapeople.org/groups/virt/virtio-win/direct-downloads/archive-virtio/virtio-win-0.1.285-1/virtio-win-0.1.285.iso'
    $isoPath = 'C:\virtio-win-0.1.285.iso'

    if (-not (Test-Path $isoPath)) {
        Write-Output "Downloading virtio-win ISO..."
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $isoUrl -OutFile $isoPath -UseBasicParsing
    }

    Write-Output "Mounting ISO..."
    $vol = Mount-DiskImage -ImagePath $isoPath -PassThru
    $driveLetter = ($vol | Get-Volume).DriveLetter
    $installer = "${driveLetter}:\guest-tools.exe"
}

if (-not (Test-Path $installer)) {
    throw "guest-tools.exe not found at $installer"
}

Write-Output "Running virtio-win guest-tools installer..."
$p = Start-Process -FilePath $installer -ArgumentList '/S' -Wait -NoNewWindow -PassThru

Write-Output "Installer exited with code $($p.ExitCode)"

# Cleanup mounted ISO
if ($vol) {
    Dismount-DiskImage -ImagePath $isoPath -ErrorAction SilentlyContinue
}

# Verify installation
if (-not (Test-Path 'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe')) {
    throw "virtiofs.exe not found after installation"
}
Write-Output "virtio-win installed OK"
