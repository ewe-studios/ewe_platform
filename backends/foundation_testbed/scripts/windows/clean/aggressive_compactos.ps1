# Compress system files in-place (CompactOS). Frees ~2 GB on Windows 11.
# Idempotent — exits early if already compacted.
$state = (& compact.exe /CompactOS:query | Out-String)
if ($state -match 'system is in the Compact state') {
    '  already compacted'
} else {
    & compact.exe /CompactOS:always | Out-Null
    '  compacted'
}
