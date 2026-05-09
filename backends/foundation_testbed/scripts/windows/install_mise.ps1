$miseDir = "$env:USERPROFILE\.local\bin"
if (-not (Test-Path $miseDir)) { New-Item -ItemType Directory -Path $miseDir -Force | Out-Null }
Expand-Archive -Force 'C:\mise.zip' $miseDir
Remove-Item 'C:\mise.zip' -Force -ErrorAction SilentlyContinue

$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notmatch [regex]::Escape($miseDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseDir;$userPath", 'User')
}

$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($miseDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseDir;$machinePath", 'Machine')
}

$env:PATH = "$miseDir;$env:PATH"
