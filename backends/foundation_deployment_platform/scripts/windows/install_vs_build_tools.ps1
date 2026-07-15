$ErrorActionPreference = 'Continue'
$bootstrapper = 'C:\vs_buildtools.exe'

# Download the VS Build Tools bootstrapper from Microsoft
if (-not (Test-Path $bootstrapper)) {
    Write-Output 'Downloading VS Build Tools bootstrapper...'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_buildtools.exe' -OutFile $bootstrapper -UseBasicParsing
}

# Install VS Build Tools with the C++ workload + ARM64 cross-tools.
Write-Output 'Installing VS Build Tools (C++ workload)...'
$p = Start-Process -FilePath $bootstrapper -ArgumentList @(
    '--add', 'Microsoft.VisualStudio.Workload.VCTools',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.ARM64',
    '--add', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64',
    '--add', 'Microsoft.VisualStudio.Component.Windows11SDK.22621',
    '--includeRecommended', '--quiet', '--norestart', '--wait'
) -Wait -NoNewWindow -PassThru
$p.ExitCode | Out-File 'C:\vs-exit.txt'
Write-Output "Installer exited with code $($p.ExitCode)"

# Clean up
Remove-Item $bootstrapper -Force -ErrorAction SilentlyContinue

if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010) {
    throw "VS Build Tools installer exited with code $($p.ExitCode)"
}

# Verify installation
$link = Get-ChildItem 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe' -ErrorAction SilentlyContinue
if (-not $link) {
    throw "link.exe not found after installation"
}
Write-Output "link.exe verified at $($link.FullName)"
