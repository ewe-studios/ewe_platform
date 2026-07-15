# Configure sshd_config:
# - Uncomment PasswordAuthentication yes
# - Comment out the Match Group administrators block so ~/.ssh/authorized_keys
#   works for all users (admin or otherwise).
# - Comment out AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys
#   so admin users also fall through to ~/.ssh/.
$p = 'C:\ProgramData\ssh\sshd_config'
if (-not (Test-Path $p)) { return }
$c = Get-Content $p
$o = @()
foreach ($l in $c) {
    if ($l -match '^#PasswordAuthentication yes')              { $o += 'PasswordAuthentication yes' }
    elseif ($l -match '^Match Group administrators')           { $o += '#Match Group administrators' }
    elseif ($l -match 'AuthorizedKeysFile __PROGRAMDATA__')   { $o += '#AuthorizedKeysFile __PROGRAMDATA__/ssh/administrators_authorized_keys' }
    else                                                       { $o += $l }
}
$o | Set-Content $p -Force
Start-Service sshd -ErrorAction SilentlyContinue
Set-Service -Name sshd -StartupType Automatic
Restart-Service sshd
