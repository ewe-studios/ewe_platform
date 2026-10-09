# Online VS Build Tools installation — downloads bootstrapper and runs install.
# Usage: powershell -File vs_install_online.ps1 -Bootstrapper C:\vs_buildtools.exe -InstallDir "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools"

param(
    [Parameter(Mandatory=$true)]
    [string]$Bootstrapper,

    [Parameter(Mandatory=$true)]
    [string]$InstallDir
)

$ErrorActionPreference = 'Continue'

Write-Output "Starting VS Build Tools online installation..."
$p = Start-Process -FilePath $Bootstrapper `
    -ArgumentList '--installPath', $InstallDir, `
        '--add', 'Microsoft.VisualStudio.Workload.VCTools', `
        '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64', `
        '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621', `
        '--includeRecommended', '--lang', 'en-US', `
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
Remove-Item $Bootstrapper -Force -ErrorAction SilentlyContinue
