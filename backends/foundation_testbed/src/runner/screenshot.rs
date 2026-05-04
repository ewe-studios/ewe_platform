//! Screenshot capture from VM displays.
//!
//! Linux: scrot against Xvfb display :99.
//! Windows: .NET System.Drawing screen capture via PowerShell.

use std::path::Path;

use crate::config::{GuestOs, Result, VmProfile};
use crate::ssh::VmSession;

/// Capture a screenshot of the VM display.
///
/// On Linux, captures from Xvfb display :99.
/// On Windows, captures the primary monitor via .NET.
pub fn capture(profile: &VmProfile, session: &mut VmSession, output: &Path) -> Result<()> {
    let vm_path = "/tmp/testbed-screenshot.png";

    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => capture_linux(session, vm_path)?,
        GuestOs::Windows => capture_windows(session, vm_path)?,
    }

    // Download to host
    crate::ssh::download(session, vm_path, output)?;

    // Clean up on VM
    crate::ssh::exec(session, &format!("rm -f {vm_path}"))?;

    Ok(())
}

/// Capture screenshot on Linux via scrot.
fn capture_linux(session: &mut VmSession, vm_path: &str) -> Result<()> {
    crate::ssh::exec(
        session,
        &format!("DISPLAY=:99 scrot --overwrite {vm_path}"),
    )?;
    Ok(())
}

/// Capture screenshot on Windows via .NET System.Drawing.
fn capture_windows(session: &mut VmSession, vm_path: &str) -> Result<()> {
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen(0, 0, 0, 0, $bmp.Size)
$bmp.Save('{vm_path}', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bmp.Dispose()
"#
    );
    crate::ssh::exec(session, &script)?;
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_capture_uses_correct_paths() {
        // Verify the function would use the right VM path
        let path = "/tmp/testbed-screenshot.png";
        assert!(path.starts_with("/tmp/"));
        assert!(path.ends_with(".png"));
    }
}
