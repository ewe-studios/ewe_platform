$mise_toml = @"
{{MISE_TOML}}
"@
Set-Content -Path "$env:TEMP\bootstrap-mise.toml" -Value $mise_toml -Encoding UTF8
$env:MISE_CONFIG_FILE = "$env:TEMP\bootstrap-mise.toml"
mise install
Remove-Item "$env:TEMP\bootstrap-mise.toml" -ErrorAction SilentlyContinue

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
