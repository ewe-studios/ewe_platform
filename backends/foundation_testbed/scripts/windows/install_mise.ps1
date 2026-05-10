$miseDir = "$env:USERPROFILE\.local\bin"
$miseBinDir = "$miseDir\mise\bin"
if (-not (Test-Path $miseDir)) { New-Item -ItemType Directory -Path $miseDir -Force | Out-Null }
Expand-Archive -Force 'C:\mise.zip' $miseDir
Remove-Item 'C:\mise.zip' -Force -ErrorAction SilentlyContinue

# mise binary is at .local/bin/mise/bin/mise.exe
$userPath = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($userPath -notmatch [regex]::Escape($miseBinDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseBinDir;$userPath", 'User')
}

$machinePath = [Environment]::GetEnvironmentVariable('PATH', 'Machine')
if ($machinePath -notmatch [regex]::Escape($miseBinDir)) {
    [Environment]::SetEnvironmentVariable('PATH', "$miseBinDir;$machinePath", 'Machine')
}

$env:PATH = "$miseBinDir;$env:PATH"
