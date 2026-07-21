@echo off
REM SPICE Guest Tools — enables clipboard sharing, display drivers, and
REM mouse integration between the VM host and the Windows guest.
REM Required for clipboard sync via the noVNC browser viewer.

echo [SPICE] installing SPICE Guest Tools...

REM Install SPICE Guest Tools silently
if exist "C:\OEM\spice-guest-tools.exe" (
    C:\OEM\spice-guest-tools.exe /S
    echo [SPICE] installed successfully.
) else (
    echo [SPICE] installer not found — downloading...
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
      "$url = 'https://www.spice-space.org/download/windows/spice-guest-tools/spice-guest-tools-latest.exe';" ^
      "$out = '$env:TEMP\spice-guest-tools.exe';" ^
      "Invoke-WebRequest -Uri $url -OutFile $out;" ^
      "& $out /S;" ^
      "Remove-Item $out -Force -ErrorAction SilentlyContinue"
    echo [SPICE] downloaded and installed.
)
