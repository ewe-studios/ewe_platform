# Delete all Volume Shadow Copies on C:.
$o = (& vssadmin.exe list shadows /for=C: 2>&1 | Out-String)
if ($o -match 'No items found' -or $o -match 'could not be found') {
    '  no shadows present'
} else {
    & vssadmin.exe delete shadows /for=C: /all /quiet | Out-Null
    '  shadows cleared'
}
