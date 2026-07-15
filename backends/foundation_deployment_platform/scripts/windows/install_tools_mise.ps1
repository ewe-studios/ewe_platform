$mise_toml = @"
{{MISE_TOML}}
"@

# Write to temp file for installation
Set-Content -Path "$env:TEMP\bootstrap-mise.toml" -Value $mise_toml -Encoding UTF8
$env:MISE_CONFIG_FILE = "$env:TEMP\bootstrap-mise.toml"
mise install
Remove-Item "$env:TEMP\bootstrap-mise.toml" -ErrorAction SilentlyContinue

# Ensure tools are in permanent config
$cfg = "$env:USERPROFILE\AppData\Roaming\mise\config.toml"
if (-not (Test-Path $cfg)) {
    Set-Content $cfg $mise_toml -Encoding UTF8
}

# Ensure cargo tools are in PATH for both User and Machine scope
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
