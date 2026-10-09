# Move pagefile to D:\pagefile.sys so C: can be smaller. Reboot required.
if (-not (Test-Path 'D:\')) { '  D: not present, skip'; return }
try {
    $cs = Get-CimInstance -ClassName Win32_ComputerSystem -ErrorAction Stop
    if (-not $cs.AutomaticManagedPagefile) { '  already custom-managed (skip)'; return }
    Set-CimInstance -InputObject $cs -Property @{ AutomaticManagedPagefile = $false } -ErrorAction Stop
    Get-CimInstance -ClassName Win32_PageFileSetting -ErrorAction SilentlyContinue | Remove-CimInstance -ErrorAction SilentlyContinue
    New-CimInstance -ClassName Win32_PageFileSetting -Property @{
        Name = 'D:\pagefile.sys'; InitialSize = 0; MaximumSize = 0
    } -ErrorAction Stop | Out-Null
    '  set to D: (reboot required)'
} catch { '  skipped: ' + $_.Exception.Message }
