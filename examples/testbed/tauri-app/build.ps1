# Build and run the Tauri E2E test app inside a Windows VM.
#
# Usage (inside guest, PowerShell):
#   .\build.ps1 build    — build the Tauri app in release mode
#   .\build.ps1 verify   — check the build artifact exists and is a PE binary
#   .\build.ps1 launch   — launch the built binary headlessly, check alive after 5s
#
# The mise toolchain (rust, tauri-cli) is expected to be pre-installed
# by the testbed bootstrap (BOOTSTRAP_MISE_TOML).

param(
    [ValidateSet("build", "verify", "launch")]
    [string]$Command = "build"
)

$ErrorActionPreference = "Stop"
$exe = "target\x86_64-pc-windows-msvc\release\tauri-e2e-test.exe"

function Build-App {
    Write-Host "[tauri-app] building in release mode..."
    & cargo tauri build
    if (-not (Test-Path $exe)) {
        throw "[tauri-app] build failed — artifact not found: $exe"
    }
    Write-Host "[tauri-app] built: $exe"
}

function Verify-App {
    if (Test-Path $exe) {
        Write-Host "[tauri-app] artifact found: $exe"
        # Check MZ header (PE binary)
        $bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $exe))
        if ($bytes[0] -eq 0x4D -and $bytes[1] -eq 0x5A) {
            Write-Host "[tauri-app] valid PE binary"
            exit 0
        } else {
            Write-Host "[tauri-app] NOT a valid PE binary"
            exit 1
        }
    } else {
        Write-Host "[tauri-app] artifact missing: $exe"
        exit 1
    }
}

function Launch-App {
    if (-not (Test-Path $exe)) {
        Write-Host "[tauri-app] binary not found, building first..."
        Build-App
    }
    Write-Host "[tauri-app] launching headlessly..."
    Start-Process -FilePath (Resolve-Path $exe) -WindowStyle Hidden
    Start-Sleep -Seconds 5
    $proc = Get-Process -Name "tauri-e2e-test" -ErrorAction SilentlyContinue
    if ($null -ne $proc) {
        Write-Host "[tauri-app] alive"
        Stop-Process -Name "tauri-e2e-test" -Force -ErrorAction SilentlyContinue
        exit 0
    } else {
        Write-Host "[tauri-app] dead after 5s"
        exit 1
    }
}

switch ($Command) {
    "build"   { Build-App }
    "verify"  { Verify-App }
    "launch"  { Launch-App }
}
