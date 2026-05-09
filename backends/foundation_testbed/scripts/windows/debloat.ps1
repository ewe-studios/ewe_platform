# Remove pre-installed Windows Store apps that have no purpose on a build VM.
# Scans, then removes both per-user and provisioned packages.
# The Rust caller substitutes __PREFIXES__ and __ACTION__.

$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding           = [System.Text.Encoding]::UTF8

$prefixes = @(__PREFIXES__)

$drive   = Get-PSDrive C
$before  = $drive.Free
Write-Output ('C: free before: {0:N1} GB' -f ($before/1GB))
Write-Output ''
Write-Output '--- Scanning installed Appx packages ---'

$found = @()
foreach ($prefix in $prefixes) {
    Get-AppxPackage -AllUsers -Name "$prefix*" -ErrorAction SilentlyContinue | ForEach-Object {
        if ($_.SignatureKind -eq 'System') { return }
        if ($_.InstallLocation -and $_.InstallLocation -like 'C:\Windows\SystemApps\*') { return }
        if ($found.Name -notcontains $_.Name) {
            $found += $_
        }
    }
}

if ($found.Count -eq 0) {
    Write-Output '  (no debloat-list packages installed — already clean)'
    return
}

foreach ($p in $found) {
    Write-Output ('  found     {0}' -f $p.Name)
}
Write-Output ''
Write-Output ('Found {0} packages to remove.' -f $found.Count)
__ACTION__

$drive2 = Get-PSDrive C
$after  = $drive2.Free
$freed  = $after - $before
function Fmt($b) {
    if (-not $b -or $b -le 0) { return '0 B' }
    if ($b -ge 1GB) { return ('{0:N1} GB' -f ($b/1GB)) }
    if ($b -ge 1MB) { return ('{0:N0} MB' -f ($b/1MB)) }
    return ('{0:N0} KB' -f ($b/1KB))
}
Write-Output ''
Write-Output ('Freed:   {0}' -f (Fmt $freed))
Write-Output ('C: free: {0:N1} GB -> {1:N1} GB' -f ($before/1GB), ($after/1GB))
