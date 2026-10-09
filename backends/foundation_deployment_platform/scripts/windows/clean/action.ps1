foreach ($p in $plan) {
    Write-Output ('  {0,-40} cleaning... ' -f $p.N) -NoNewline
    foreach ($path in $p.P) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
    Write-Output 'done'
}
Write-Output ''
Write-Output '--- DISM /StartComponentCleanup /ResetBase ---'
& dism.exe /Online /Cleanup-Image /StartComponentCleanup /ResetBase | Out-Null
Write-Output '  (DISM finished)'
