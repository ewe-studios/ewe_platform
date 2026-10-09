# Set up project mount using pre-installed virtiofs.exe.
$ErrorActionPreference = 'Stop'

# 1. Find virtiofs.exe
$exe = $null
$paths = @(
    'C:\Program Files\Virtio-Win\VioFS\virtiofs.exe',
    'C:\Program Files (x86)\Virtio-Win\VioFS\virtiofs.exe'
)
foreach ($p in $paths) {
    if (Test-Path $p) { $exe = $p; break }
}
if (-not $exe) { throw "virtiofs.exe not found" }

Write-Output "Found virtiofs.exe at $exe"

# 2. Create mount point
$mountPoint = 'C:\Users\vagrant\project'
if (-not (Test-Path $mountPoint)) {
    New-Item -ItemType Directory -Path $mountPoint -Force | Out-Null
}

# 3. Mount now
Start-Process -FilePath $exe -ArgumentList '-t project', '-m C:\Users\vagrant\project' -WindowStyle Hidden

# 4. Wait for mount
for ($i = 0; $i -lt 15; $i++) {
    Start-Sleep -Seconds 2
    if (Test-Path "$mountPoint\Cargo.toml") { break }
}

if (-not (Test-Path "$mountPoint\Cargo.toml")) {
    $files = Get-ChildItem $mountPoint -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }
    throw "mount failed. Contents of $mountPoint : $($files -join ', ')"
}

Write-Output "Project mount verified at $mountPoint"
