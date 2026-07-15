$reg_path = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
if (-not (Test-Path $reg_path)) { New-Item -Path $reg_path -Force }
Set-ItemProperty -Path $reg_path -Name 'LocalAccountTokenFilterPolicy' -Value 1 -Type DWord -Force
