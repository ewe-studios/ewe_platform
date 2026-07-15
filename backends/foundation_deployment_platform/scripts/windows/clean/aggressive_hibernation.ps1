# Disable hibernation and remove hiberfil.sys (typically 2-4 GB).
if (Test-Path "$env:SystemRoot\hiberfil.sys") {
    $sz = (Get-Item -LiteralPath "$env:SystemRoot\hiberfil.sys" -Force -ErrorAction SilentlyContinue).Length
    & powercfg.exe /h off | Out-Null
    if ($sz -ge 1GB) { ('  freed ~{0:N1} GB' -f ($sz/1GB)) } else { ('  freed ~{0:N0} MB' -f ($sz/1MB)) }
} else {
    '  hiberfil.sys absent (already off)'
}
