# Clear Windows Event Log channels — skip OpenSSH/Security/System/Setup
# to keep ssh/security/boot history for diagnosis.
$skip = @('OpenSSH', '-Security', '^Security$', '^System$', '^Setup$')
$n = 0
foreach ($log in (& wevtutil.exe el 2>$null)) {
    $s = $false
    foreach ($pat in $skip) { if ($log -match $pat) { $s = $true; break } }
    if (-not $s) { & wevtutil.exe cl "$log" 2>$null; $n++ }
}
('  cleared {0} channels' -f $n)
