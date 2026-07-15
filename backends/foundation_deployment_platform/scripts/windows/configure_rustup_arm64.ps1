$candidates = @(
  'D:\mise\installs\rust\stable\rustup.exe',
  "$env:USERPROFILE\.local\share\mise\installs\rust\stable\rustup.exe"
)
$rustup = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if ($rustup) {
  & $rustup set default-host x86_64-pc-windows-msvc
  & $rustup default --force-non-host stable-x86_64-pc-windows-msvc
}
