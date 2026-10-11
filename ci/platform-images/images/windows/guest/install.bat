@echo off
REM platform-images: dockurr runs C:\OEM\install.bat once, as the last step
REM of the unattended Windows install. Everything happens in provision.ps1
REM (Windows PowerShell 5.1, the only PowerShell a fresh install has); this
REM only records the outcome where `vm.nu wait` looks for it:
REM   C:\OEM\status\provisioned.ok      success
REM   C:\OEM\status\provision.failed    failure (details in provision.log)

mkdir C:\OEM\status 2>nul
powershell -NoProfile -ExecutionPolicy Bypass -File C:\OEM\provision.ps1 > C:\OEM\status\provision.log 2>&1
if errorlevel 1 (
  echo provision.ps1 failed, see C:\OEM\status\provision.log > C:\OEM\status\provision.failed
  exit /b 1
)
echo ok > C:\OEM\status\provisioned.ok
exit /b 0
