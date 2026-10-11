# platform-images helpers for provisioning a Windows VM, shared by layer 1
# (provision.ps1) and repository layers (C:\OEM\layers\*\install.ps1).
# Windows PowerShell 5.1 compatible: it runs before PowerShell 7 exists.

Set-StrictMode -Version Latest

# Log to the host, not the pipeline: functions here return values (paths,
# exit codes), and Write-Output would mix log lines into them. Host output
# still lands in provision.log (install.bat redirects powershell's stdout).
function Write-PiLog {
    param([Parameter(Mandatory)][string]$Message)
    Write-Host ("[platform-images {0}] {1}" -f (Get-Date -Format s), $Message)
}

# Quote one argument for a Windows command line (CommandLineToArgvW rules).
function ConvertTo-PiArgument {
    param([Parameter(Mandatory)][AllowEmptyString()][string]$Value)
    if ($Value -ne '' -and $Value -notmatch '[\s"]') { return $Value }
    $escaped = $Value -replace '(\\*)"', '$1$1\"'
    $escaped = $escaped -replace '(\\+)$', '$1$1'
    return '"' + $escaped + '"'
}

# Run a native command, wait for it (also for GUI-subsystem installers,
# which `&` would not wait for) and fail on a non-zero exit code (PowerShell
# 5.1 does not do that by itself). -AllowedExitCodes covers installers that
# use e.g. 3010 for "success, reboot required".
function Invoke-Native {
    param(
        [Parameter(Mandatory)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [int[]]$AllowedExitCodes = @(0)
    )
    # Start-Process joins an argument array without quoting, so quote here.
    $line = ($ArgumentList | ForEach-Object { ConvertTo-PiArgument $_ }) -join ' '
    Write-PiLog ("> {0} {1}" -f $FilePath, $line)
    if ($line -eq '') {
        $p = Start-Process -FilePath $FilePath -NoNewWindow -Wait -PassThru
    } else {
        $p = Start-Process -FilePath $FilePath -ArgumentList $line -NoNewWindow -Wait -PassThru
    }
    if ($AllowedExitCodes -notcontains $p.ExitCode) {
        throw ("{0} exited with code {1}" -f $FilePath, $p.ExitCode)
    }
    return $p.ExitCode
}

# Download a component listed in components.json and check it before use:
# its sha256, or a valid Authenticode signature from the expected signer.
function Get-VerifiedDownload {
    param(
        [Parameter(Mandatory)][string]$Name,
        [Parameter(Mandatory)]$Component,
        [Parameter(Mandatory)][string]$FileName,
        [string]$Directory = 'C:\platform-images\downloads'
    )
    $dir = $Directory
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $file = Join-Path $dir $FileName
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $attempt = 0
    while ($true) {
        $attempt++
        try {
            Write-PiLog ("downloading {0} {1}" -f $Name, $Component.url)
            Invoke-WebRequest -Uri $Component.url -OutFile $file -UseBasicParsing
            break
        } catch {
            if ($attempt -ge 3) { throw }
            Write-PiLog ("download of {0} failed ({1}); retrying" -f $Name, $_.Exception.Message)
            Start-Sleep -Seconds (10 * $attempt)
        }
    }
    $props = $Component.PSObject.Properties.Name
    if ($props -contains 'sha256') {
        $actual = (Get-FileHash -Algorithm SHA256 -Path $file).Hash.ToLowerInvariant()
        if ($actual -ne $Component.sha256.ToLowerInvariant()) {
            throw ("{0}: sha256 {1} does not match the pinned {2}" -f $Name, $actual, $Component.sha256)
        }
    } elseif ($props -contains 'signer') {
        $sig = Get-AuthenticodeSignature -FilePath $file
        if ($sig.Status -ne 'Valid') {
            throw ("{0}: Authenticode signature is {1}" -f $Name, $sig.Status)
        }
        if ($sig.SignerCertificate.Subject -notlike ("*{0}*" -f $Component.signer)) {
            throw ("{0}: signed by '{1}', expected '{2}'" -f $Name, $sig.SignerCertificate.Subject, $Component.signer)
        }
    } else {
        throw ("{0}: components.json gives neither sha256 nor signer" -f $Name)
    }
    Write-PiLog ("verified {0}" -f $Name)
    return $file
}

function Set-MachineEnv {
    param([Parameter(Mandatory)][string]$Name, [Parameter(Mandatory)][string]$Value)
    [Environment]::SetEnvironmentVariable($Name, $Value, 'Machine')
    Set-Item -Path ("env:{0}" -f $Name) -Value $Value
}

# Prepend a directory to the machine PATH (and this process's PATH).
function Add-MachinePath {
    param([Parameter(Mandatory)][string]$Directory)
    $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
    $parts = $machine -split ';' | Where-Object { $_ -ne '' }
    if ($parts -notcontains $Directory) {
        [Environment]::SetEnvironmentVariable('Path', ($Directory + ';' + $machine), 'Machine')
    }
    if (($env:Path -split ';') -notcontains $Directory) {
        $env:Path = $Directory + ';' + $env:Path
    }
}

# Install the tools a mise.toml (+ mise.lock) in <ConfigDir> pins, with the
# platform-images mise and data dir, and put their bin directories on the
# machine PATH. Locked: downloads come from the lock and are checked against
# its sha256 (so mise's GitHub attestation lookup, which needs the GitHub
# API, is skipped for python).
function Install-MiseTools {
    param([Parameter(Mandatory)][string]$ConfigDir)
    $config = Join-Path $ConfigDir 'mise.toml'
    if (-not (Test-Path $config)) { throw ("no mise.toml in {0}" -f $ConfigDir) }
    $env:MISE_YES = '1'
    if (Test-Path (Join-Path $ConfigDir 'mise.lock')) {
        $env:MISE_LOCKED = '1'
        $env:MISE_PYTHON_GITHUB_ATTESTATIONS = 'false'
    }
    Push-Location $ConfigDir
    try {
        Invoke-Native -FilePath 'mise' -ArgumentList @('trust', '--yes', $config) | Out-Null
        Invoke-Native -FilePath 'mise' -ArgumentList @('install', '--yes') | Out-Null
        $paths = & mise bin-paths
        if ($LASTEXITCODE -ne 0) { throw 'mise bin-paths failed' }
    } finally {
        Pop-Location
        Remove-Item env:MISE_LOCKED -ErrorAction SilentlyContinue
        Remove-Item env:MISE_PYTHON_GITHUB_ATTESTATIONS -ErrorAction SilentlyContinue
    }
    $count = 0
    foreach ($p in $paths) {
        if ($p -and (Test-Path $p)) { Add-MachinePath $p; $count++ }
    }
    if ($count -eq 0) { throw ("no tool directories for {0}" -f $config) }
    Write-PiLog ("mise tools from {0}: {1} directories on PATH" -f $config, $count)
}

Export-ModuleMember -Function Write-PiLog, Invoke-Native, Get-VerifiedDownload, Set-MachineEnv, Add-MachinePath, Install-MiseTools
