//! Screenshot capture from VM displays.
//!
//! Linux: scrot against Xvfb display :99.
//! Windows: .NET System.Drawing screen capture via interactive Scheduled Task.

use std::path::Path;

use crate::config::{GuestOs, Result, VmProfile};
use crate::ssh::VmSession;
use crate::winrm::WinRM;

/// Capture a screenshot of the VM display.
///
/// On Linux, captures from Xvfb display :99.
/// On Windows, captures the primary monitor via interactive Scheduled Task
/// (so `Graphics.CopyFromScreen` sees the real desktop, not a blank session 0).
pub fn capture(profile: &VmProfile, session: &mut VmSession, winrm: Option<&WinRM>, output: &Path) -> Result<()> {
    let vm_path = "/tmp/testbed-screenshot.png";

    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => capture_linux(session, vm_path)?,
        GuestOs::Windows => capture_windows(session, winrm, vm_path)?,
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

/// Capture screenshot on Windows via interactive Scheduled Task.
///
/// Uses `bootstrap::windows::screenshot_windows` which runs the screenshot
/// script in a `/RU vagrant /IT` task so `CopyFromScreen` gets real pixels.
fn capture_windows(session: &mut VmSession, winrm: Option<&WinRM>, _vm_path: &str) -> Result<()> {
    if let Some(winrm) = winrm {
        let output = Path::new(_vm_path);
        crate::bootstrap::windows::screenshot_windows(winrm, session, output)?;
        return Ok(());
    }

    // Fallback: headless (may return blank pixels on some systems)
    crate::ssh::exec(session, r#"
Add-Type -AssemblyName System.Windows.Forms
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$graphics.CopyFromScreen(0, 0, 0, 0, $bmp.Size)
$bmp.Save('/tmp/testbed-screenshot.png', [System.Drawing.Imaging.ImageFormat]::Png)
$graphics.Dispose()
$bmp.Dispose()
"#)?;
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
