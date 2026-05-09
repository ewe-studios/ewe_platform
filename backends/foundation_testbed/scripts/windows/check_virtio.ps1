# Check if virtio-win is installed.
# Output: "VERSION|PATH" if installed, "not_installed" otherwise.
$ErrorActionPreference = 'SilentlyContinue'

# 1. Check registry
$reg = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\Virtio-win-driver-installer' -ErrorAction SilentlyContinue
if ($reg -and $reg.DisplayVersion) {
    $version = $reg.DisplayVersion
}

# 2. Check filesystem for virtiofs.exe
$paths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)
foreach ($p in $paths) {
    if (Test-Path $p) {
        $dir = (Get-Item $p).Directory.Parent.FullName
        $exe = $p
        break
    }
}

# 3. Fallback: search uninstall registry for any virtio entry
if (-not $version) {
    $virtio = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
              Where-Object { $_.DisplayName -like '*virtio*' } |
              Select-Object -First 1
    if ($virtio) {
        $version = $virtio.DisplayVersion
    }
}

if ($exe -and $dir) {
    if ($version) { "$version|$dir|$exe" } else { "unknown|$dir|$exe" }
} else {
    "not_installed"
}
