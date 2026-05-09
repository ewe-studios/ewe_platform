# Install cargo-binstall directly so mise's cargo backend can use it to fetch
# prebuilt tools from GitHub Releases instead of compiling from source.
$dest = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path "$dest\cargo-binstall.exe")) {
    if (-not (Test-Path $dest)) { New-Item -ItemType Directory -Path $dest | Out-Null }
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    $zip = "$env:TEMP\cargo-binstall.zip"
    Invoke-WebRequest -Uri 'https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-pc-windows-msvc.zip' -OutFile $zip -UseBasicParsing
    Expand-Archive -Force $zip $dest
    Remove-Item $zip -Force -ErrorAction SilentlyContinue
}
# Ensure ~/.cargo/bin is on PATH for future shell sessions.
$path = [Environment]::GetEnvironmentVariable('PATH', 'User')
if ($path -notmatch [regex]::Escape($dest)) {
    [Environment]::SetEnvironmentVariable('PATH', $path + ';' + $dest, 'User')
}
