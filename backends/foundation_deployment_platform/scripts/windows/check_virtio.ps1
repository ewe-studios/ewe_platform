# Check if virtio-win is installed.
# Output: "VERSION|PATH" if installed, "not_installed" otherwise.
$ErrorActionPreference = 'SilentlyContinue'

# 1. FAST PATH: Check common installation locations first
$commonPaths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)

foreach ($p in $commonPaths) {
    if (Test-Path $p) {
        $exe = $p
        $dir = (Get-Item $p).Directory.Parent.FullName
        # Try to get version from registry
        $reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer' -ErrorAction SilentlyContinue
        $version = if ($reg -and $reg.DisplayVersion) { $reg.DisplayVersion } else { "unknown" }
        "$version|$dir|$exe"
        exit 0
    }
}

# 2. FALLBACK: Check registry for installation path
$reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer' -ErrorAction SilentlyContinue
if ($reg -and $reg.DisplayVersion) {
    $version = $reg.DisplayVersion
    # Try to find install location from registry
    if ($reg.InstallLocation -and (Test-Path (Join-Path $reg.InstallLocation "VioFS\virtiofs.exe"))) {
        $dir = $reg.InstallLocation
        $exe = Join-Path $reg.InstallLocation "VioFS\virtiofs.exe"
        "$version|$dir|$exe"
        exit 0
    }
}

# 3. FALLBACK: Search uninstall registry for any virtio entry
$virtio = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
          Where-Object { $_.DisplayName -like '*virtio*' } |
          Select-Object -First 1
if ($virtio) {
    $version = $virtio.DisplayVersion
    # Try InstallLocation from found entry
    if ($virtio.InstallLocation) {
        $testPath = Join-Path $virtio.InstallLocation "VioFS\virtiofs.exe"
        if (Test-Path $testPath) {
            $exe = $testPath
            $dir = $virtio.InstallLocation
            "$version|$dir|$exe"
            exit 0
        }
    }
}

# 4. Not found - need to install
"not_installed"
