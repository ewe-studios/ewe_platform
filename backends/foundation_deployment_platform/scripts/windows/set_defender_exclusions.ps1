# Add Windows Defender exclusions for cargo/mise/build paths.
# Defender's real-time scan locks freshly-written files during cargo
# extract/compile, causing "file in use" errors mid-build.
$paths = @(
  'C:\Users\vagrant\project',
  'C:\Users\vagrant\.cargo',
  'C:\Users\vagrant\.rustup',
  'C:\Users\vagrant\.local\share\mise',
  'C:\Users\vagrant\AppData\Local\mise'
)
# D: drive if present (cargo target on separate disk)
if (Test-Path 'D:\target') { $paths += 'D:\target' }
foreach ($p in $paths) {
  try { Add-MpPreference -ExclusionPath $p -ErrorAction Stop } catch { }
}
