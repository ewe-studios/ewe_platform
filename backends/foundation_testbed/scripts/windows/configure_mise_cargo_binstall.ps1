$cfg = "$env:USERPROFILE\AppData\Roaming\mise\config.toml"
$dir = Split-Path $cfg
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
if (-not (Test-Path $cfg)) {
    Set-Content $cfg "[settings]`ncargo_binstall = true`n" -Encoding UTF8
} elseif ((Get-Content $cfg -Raw) -notmatch 'cargo_binstall') {
    Add-Content $cfg "`n[settings]`ncargo_binstall = true`n" -Encoding UTF8
}

# Ensure mise shims are in PATH for cargo tools (cargo-binstall, sccache, tauri-cli)
$shimsDir = "$env:USERPROFILE\AppData\Local\mise\shims"
$cargoBinDir = "$env:USERPROFILE\AppData\Local\mise\installs\cargo-crate\bin"

$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notmatch [regex]::Escape($shimsDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$shimsDir;$userPath", 'User')
}
if ($userPath -notmatch [regex]::Escape($cargoBinDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$cargoBinDir;$userPath", 'User')
}

$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($shimsDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$shimsDir;$machinePath", 'Machine')
}
if ($machinePath -notmatch [regex]::Escape($cargoBinDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$cargoBinDir;$machinePath", 'Machine')
}

# Also update current session
$env:PATH = "$shimsDir;$cargoBinDir;$env:PATH"
