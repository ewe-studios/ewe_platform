# Windows guest disk cleanup. Categorized scan of transient targets.
# The Rust caller substitutes __DEEP_BLOCK__ and __ACTION__.
$ErrorActionPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$targets = @(
    @{ N='testbed build/run logs';     P=@("$env:USERPROFILE\.testbed-build", "$env:USERPROFILE\.testbed-run") },
    @{ N='Bootstrap installers (C:\)'; P=@('C:\vs_buildtools.exe','C:\webview2_setup.exe','C:\vs-exit.txt') },
    @{ N='User temp';                  P=@("$env:USERPROFILE\AppData\Local\Temp") },
    @{ N='System temp';                P=@('C:\Windows\Temp') },
    @{ N='Windows Update cache';       P=@('C:\Windows\SoftwareDistribution\Download') },
    @{ N='VS Package Cache leftovers'; P=@('C:\ProgramData\Package Cache') },
    @{ N='Crash dumps';                P=@("$env:USERPROFILE\AppData\Local\CrashDumps", 'C:\Windows\Memory.dmp', 'C:\Windows\Minidump') },
    @{ N='Recycle Bin';                P=@('C:\$Recycle.Bin') }
__DEEP_BLOCK__
)

function Bytes($paths) {
    $sum = [int64]0
    foreach ($p in $paths) {
        if (Test-Path -LiteralPath $p) {
            $m = Get-ChildItem -LiteralPath $p -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum
            if ($m -and $m.Sum) { $sum += [int64]$m.Sum }
        }
    }
    return $sum
}
function Fmt($b) {
    if (-not $b -or $b -le 0) { return '0 B' }
    if ($b -ge 1GB) { return ('{0:N1} GB' -f ($b/1GB)) }
    if ($b -ge 1MB) { return ('{0:N0} MB' -f ($b/1MB)) }
    if ($b -ge 1KB) { return ('{0:N0} KB' -f ($b/1KB)) }
    return ('{0} B' -f $b)
}

$drive = Get-PSDrive C
$before = $drive.Free
Write-Output ('C: free before: {0:N1} GB' -f ($before/1GB))
Write-Output ''
Write-Output '--- Scanning ---'

$plan = @()
foreach ($t in $targets) {
    $b = Bytes $t.P
    Write-Output ('  {0,-40} {1}' -f $t.N, (Fmt $b))
    if ($b -gt 0) { $plan += @{ N=$t.N; P=$t.P; B=$b } }
}
$total = ($plan | ForEach-Object { $_.B } | Measure-Object -Sum).Sum
Write-Output ('  {0,-40} {1}' -f '--- scan total ---', (Fmt $total))
Write-Output ''

if ($plan.Count -eq 0) {
    Write-Output '(no transient targets to clean)'
}
__ACTION__

$drive2 = Get-PSDrive C
$after = $drive2.Free
$freed = $after - $before
Write-Output ''
Write-Output ('Freed:    {0}' -f (Fmt $freed))
Write-Output ('C: free:  {0:N1} GB -> {1:N1} GB' -f ($before/1GB), ($after/1GB))
