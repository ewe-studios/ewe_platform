$cfg = "$env:USERPROFILE\AppData\Roaming\mise\config.toml"
$dir = Split-Path $cfg
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }

# Read existing config or create new
if (Test-Path $cfg) {
    $content = Get-Content $cfg -Raw
} else {
    $content = ""
}

# Ensure [tools] section exists with required tools
$toolsSection = @"
[tools]
rust = "stable"
"aqua:nushell/nushell" = "latest"
cargo-binstall = "latest"
sccache = "latest"
"cargo:tauri-cli" = "2"
"@

if ($content -notmatch '\[tools\]') {
    $content = $toolsSection + "`n" + $content
    Set-Content $cfg $content -Encoding UTF8
}

# Ensure [settings] section exists
if ($content -notmatch 'cargo\.binstall') {
    if ($content -notmatch '\[settings\]') {
        Add-Content $cfg "`n[settings]`ncargo.binstall = true`n" -Encoding UTF8
    } else {
        # Add under existing [settings]
        $content = $content -replace '(\[settings\][^\[]*)', "`$1`ncargo.binstall = true`n"
        Set-Content $cfg $content -Encoding UTF8
    }
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
