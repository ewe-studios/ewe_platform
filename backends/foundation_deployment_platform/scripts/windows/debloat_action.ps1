foreach ($p in $found) {
    Write-Output ('  removing  {0}...' -f $p.Name) -NoNewline
    try {
        Get-AppxPackage -Name $p.Name -AllUsers -ErrorAction SilentlyContinue |
            Remove-AppxPackage -AllUsers -ErrorAction SilentlyContinue
        Get-AppxProvisionedPackage -Online -ErrorAction SilentlyContinue |
            Where-Object { $_.DisplayName -eq $p.Name } |
            ForEach-Object {
                Remove-AppxProvisionedPackage -Online -PackageName $_.PackageName -ErrorAction SilentlyContinue | Out-Null
            }
        Write-Output ' ok'
    } catch {
        Write-Output (' FAILED: {0}' -f $_.Exception.Message)
    }
}
