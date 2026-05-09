$nu_path = (Get-Command nu -ErrorAction SilentlyContinue).Source
if ($nu_path) {
    # AutoRun makes nu start for every cmd session (including SSH)
    $reg_path = 'HKCU:\Software\Microsoft\Command Processor'
    if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
    Set-ItemProperty -Path $reg_path -Name 'AutoRun' -Value "nu" -Force

    # Also set PSProfile to auto-start nu for PowerShell sessions
    $psProfile = "$env:USERPROFILE\Documents\PowerShell\Microsoft.PowerShell_profile.ps1"
    $psDir = Split-Path $psProfile
    if (-not (Test-Path $psDir)) { New-Item -ItemType Directory -Path $psDir -Force }
    # Only write if it doesn't already have the mise PATH injection
    $content = Get-Content $psProfile -ErrorAction SilentlyContinue
    if ($content -notmatch 'mise.*PATH') {
        Add-Content $psProfile "`n`$env:PATH = `"$env:USERPROFILE\.local\bin;$env:PATH`"" -Encoding UTF8
    }
}
