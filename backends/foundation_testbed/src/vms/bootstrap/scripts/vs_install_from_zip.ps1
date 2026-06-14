# Install VS Build Tools from a pre-created offline layout zip.
# Usage: powershell -File vs_install_from_zip.ps1 -ZipPath C:\vsb_layout.zip -InstallDir "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"

param(
    [Parameter(Mandatory=$true)]
    [string]$ZipPath,

    [Parameter(Mandatory=$true)]
    [string]$LayoutDir,

    [Parameter(Mandatory=$true)]
    [string]$InstallDir
)

$ErrorActionPreference = 'Continue'
$zipDest = "C:\vsb_layout"

Write-Output "Extracting layout zip to $zipDest..."
Expand-Archive -Force -LiteralPath $ZipPath -DestinationPath $zipDest

Write-Output "Installing VS Build Tools from layout (offline, --noweb)..."
$bootstrapper = "$zipDest\vs_buildtools.exe"
if (-not (Test-Path $bootstrapper)) {
    # Try vs_setup_bootstrapper.exe as alternative name
    $bootstrapper = "$zipDest\vs_setup_bootstrapper.exe"
    if (-not (Test-Path $bootstrapper)) {
        throw "vs_buildtools.exe not found in layout zip at $zipDest"
    }
}

$p = Start-Process -FilePath $bootstrapper `
    -WorkingDirectory $zipDest `
    -ArgumentList '--installPath', $InstallDir, `
        '--noweb', `
        '--add', 'Microsoft.VisualStudio.Workload.VCTools', `
        '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64', `
        '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621', `
        '--includeRecommended', `
        '--quiet', '--norestart', '--wait', `
        '--log', 'C:\vs_buildtools-install.log' `
    -WindowStyle Hidden -PassThru

Write-Output "Bootstrapper PID: $($p.Id), waiting for exit..."
Wait-Process -Id $p.Id -ErrorAction SilentlyContinue

# vs_buildtools.exe spawns vs_installer.exe which does the actual work.
# Give the child process time to appear after the bootstrapper exits.
Start-Sleep -Seconds 30
Write-Output "Polling for installer processes to exit..."
while ($true) {
    $running = Get-Process -Name 'vs_buildtools','vs_installer' -ErrorAction SilentlyContinue
    $setup = Get-Process -Name 'Setup' -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path -like '*VisualStudio*' }
    if (-not $running -and -not $setup) { break }
    Write-Output "Still running: $(($running + $setup | Select-Object -ExpandProperty Name -Unique) -join ', ')"
    Start-Sleep -Seconds 15
}

Write-Output "VS Build Tools install processes exited"

# Cleanup zip
Remove-Item $ZipPath -Force -ErrorAction SilentlyContinue
