$RegistryPath = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon'

Set-ItemProperty -Path $RegistryPath -Name 'AutoAdminLogon' -Value '1'
Set-ItemProperty -Path $RegistryPath -Name 'DefaultUserName' -Value 'vagrant'
Set-ItemProperty -Path $RegistryPath -Name 'DefaultPassword' -Value 'vagrant'
Set-ItemProperty -Path $RegistryPath -Name 'DefaultDomainName' -Value $env:COMPUTERNAME
Remove-ItemProperty -Path $RegistryPath -Name 'AutoLogonCount' -ErrorAction SilentlyContinue
