# Capture vagrant's interactive desktop session as a PNG. Runs inside a
# Scheduled Task created with /RU vagrant /IT — that's the only way `Graphics.
# CopyFromScreen` returns real pixels (a non-interactive WinSta0 session has
# no desktop for it to copy from). Saves to __OUTPUT_PATH__ on the VM; the
# Rust caller scp's it back to the host.

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)

$out = '__OUTPUT_PATH__'
$dir = Split-Path $out
if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
$bmp.Save($out)
$graphics.Dispose()
$bmp.Dispose()
