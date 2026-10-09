# Authorise the host's public key in BOTH user-level and admin-level paths.
$key = '{{KEY}}'

# User-level (works when Match Group administrators is commented out).
$dir = "$env:USERPROFILE\.ssh"
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir | Out-Null }
$f = "$dir\authorized_keys"
if (-not (Test-Path $f) -or ((Get-Content $f -ErrorAction SilentlyContinue) -notcontains $key)) {
    Add-Content $f $key -Encoding ASCII
}
icacls $f /inheritance:r /grant ($env:USERNAME + ':F') /grant 'SYSTEM:F' | Out-Null

# Admin-level (always-honoured location for Match Group administrators users).
$adm = 'C:\ProgramData\ssh\administrators_authorized_keys'
if (-not (Test-Path $adm) -or ((Get-Content $adm -ErrorAction SilentlyContinue) -notcontains $key)) {
    Add-Content $adm $key -Encoding ASCII
}
icacls $adm /inheritance:r /grant 'Administrators:F' /grant 'SYSTEM:F' | Out-Null

Restart-Service sshd -ErrorAction SilentlyContinue
