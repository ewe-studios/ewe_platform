$link = Get-ChildItem 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe' -ErrorAction SilentlyContinue
if (-not $link) { $link = Get-ChildItem 'C:\Program Files\Microsoft Visual Studio\2026\Community\VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe' -ErrorAction SilentlyContinue }
$sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\Lib\*\um\x64\kernel32.lib' -ErrorAction SilentlyContinue
if ($link -and $sdk) { 'present' } else { 'missing' }
