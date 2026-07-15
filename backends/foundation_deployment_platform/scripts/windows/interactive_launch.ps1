# Launch a binary in vagrant's interactive desktop session — this script is
# the body of a Scheduled Task created with /RU vagrant /IT, so it inherits
# the logged-on desktop (Win32 GUI windows actually appear instead of being
# silently exited like they are under SSH/WinRM headless launch).
#
# After Start-Process, dismiss any hovering Start menu via SendKeys ESC and
# raise our window via SetForegroundWindow so the screencap on the Mac side
# captures the app window, not Windows shell chrome.
#
# Substitutions made by the Rust caller before push:
#   __BIN__       → absolute Windows path to the .exe (single-quote-safe)
#   __ARGLIST__   → either empty, or " -ArgumentList @('arg1','arg2')"

Start-Process -FilePath '__BIN__'__ARGLIST__
Start-Sleep -Milliseconds 800

$base = [System.IO.Path]::GetFileNameWithoutExtension('__BIN__')
$p = Get-Process $base -ErrorAction SilentlyContinue
if ($p -and $p.MainWindowHandle -ne 0) {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
    Start-Sleep -Milliseconds 200
    $sig = '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);'
    $api = Add-Type -MemberDefinition $sig -Name Win32 -Namespace U -PassThru
    $api::SetForegroundWindow($p.MainWindowHandle) | Out-Null
}

$dir = "$env:USERPROFILE\project\.testbed\windows-build\screenshot"
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
if ($p) {
    ('launched-pid=' + $p.Id) | Out-File "$dir\launched-pid.txt" -Force
}
