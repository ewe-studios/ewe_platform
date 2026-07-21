@echo off
REM ewe-test-windows first-boot provisioning.
REM dockurr runs C:\OEM\install.bat once after Windows setup completes.
REM Installs the test toolchain: Rust, Edge WebView2 (Tauri dep), OpenSSH.

echo [ewe-test-windows] provisioning...

REM ── Install winget if not already available ─────────────────────────────────
where winget >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo [ewe-test-windows] winget not found — installing App Installer
    REM On Windows 11, App Installer should already be present. Best effort.
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "Add-AppxPackage -RegisterByFamilyName -MainPackage Microsoft.DesktopAppInstaller_8wekyb3d8bbwe" >nul 2>&1
)

REM ── Rust (via rustup) ──────────────────────────────────────────────────────
echo [ewe-test-windows] installing Rust toolchain
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Invoke-WebRequest -Uri 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe' -OutFile '$env:TEMP\rustup-init.exe';" ^
  "& $env:TEMP\rustup-init.exe -y --default-toolchain stable --profile minimal;" ^
  "Remove-Item $env:TEMP\rustup-init.exe"
REM Add cargo to PATH for this session
set PATH=%USERPROFILE%\.cargo\bin;%PATH%

REM ── Edge WebView2 (Evergreen runtime) ───────────────────────────────────────
REM Tauri on Windows requires WebView2. The Evergreen Bootstrapper installs the
REM runtime system-wide (small download, auto-updates). The Fixed Version
REM (standalone) is larger but doesn't auto-update — we use Evergreen.
echo [ewe-test-windows] installing Edge WebView2 Runtime
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Invoke-WebRequest -Uri 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile '$env:TEMP\WebView2Setup.exe';" ^
  "& $env:TEMP\WebView2Setup.exe /silent /install;" ^
  "Remove-Item $env:TEMP\WebView2Setup.exe"

REM ── MSVC Build Tools (needed for native crate compilation) ──────────────────
REM Tauri apps compile native Rust, which needs the MSVC linker. The Visual
REM Studio Build Tools provide this without the full IDE.
echo [ewe-test-windows] installing Visual Studio Build Tools
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Invoke-WebRequest -Uri 'https://aka.ms/vs/17/release/vs_buildtools.exe' -OutFile '$env:TEMP\vs_buildtools.exe';" ^
  "& $env:TEMP\vs_buildtools.exe --quiet --wait --norestart --nocache --includeRecommended --add Microsoft.VisualStudio.Workload.VCTools;" ^
  "Remove-Item $env:TEMP\vs_buildtools.exe"

REM ── SPICE Guest Tools (clipboard sharing via noVNC/VNC) ─────────────────────
echo [ewe-test-windows] installing SPICE Guest Tools
if exist "C:\OEM\spice-guest-tools.exe" (
    C:\OEM\spice-guest-tools.exe /S
    echo [ewe-test-windows] SPICE Guest Tools installed (local).
) else (
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$url = 'https://www.spice-space.org/download/windows/spice-guest-tools/spice-guest-tools-latest.exe';" ^
      "$out = '$env:TEMP\spice-guest-tools.exe';" ^
      "Invoke-WebRequest -Uri $url -OutFile $out;" ^
      "& $out /S;" ^
      "Remove-Item $out -Force -ErrorAction SilentlyContinue"
    echo [ewe-test-windows] SPICE Guest Tools installed (downloaded).
)

REM ── OpenSSH Server ─────────────────────────────────────────────────────────
echo [ewe-test-windows] enabling OpenSSH Server
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0 -ErrorAction SilentlyContinue;" ^
  "Set-Service -Name sshd -StartupType Automatic -ErrorAction SilentlyContinue;" ^
  "Start-Service sshd -ErrorAction SilentlyContinue;" ^
  "if (-not (Get-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -ErrorAction SilentlyContinue)) { New-NetFirewallRule -Name 'OpenSSH-Server-In-TCP' -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22 }"

REM ── Authorize SSH key from shared volume ────────────────────────────────────
REM Drop your key at \\host.lan\Data\dev\authorized_keys (the shared volume).
if exist "\\host.lan\Data\dev\authorized_keys" (
    echo [ewe-test-windows] installing administrator authorized_keys
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$dst = \"$env:ProgramData\ssh\administrators_authorized_keys\";" ^
      "Copy-Item -Force '\\host.lan\Data\dev\authorized_keys' $dst;" ^
      "icacls $dst /inheritance:r /grant 'Administrators:F' 'SYSTEM:F' | Out-Null"
) else (
    echo [ewe-test-windows] no \\host.lan\Data\dev\authorized_keys — SSH key auth not configured
)

echo [ewe-test-windows] provisioning complete.
echo   Rust:  rustup installed (stable-x86_64-pc-windows-msvc)
echo   WebView2: Edge WebView2 Runtime (Evergreen)
echo   MSVC: Visual Studio Build Tools (VCTools workload)
echo   SSH:   OpenSSH Server running on port 22
