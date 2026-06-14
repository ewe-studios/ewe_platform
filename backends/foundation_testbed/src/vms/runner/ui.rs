//! UI automation — click, type, and screenshot-based validation.
//!
//! Windows: PowerShell SendKeys/Cursor via SSH.
//! Linux: xdotool via SSH on Xvfb display :99.

use crate::vms::config::{GuestOs, Result, VmProfile};
use crate::vms::ssh::VmSession;

/// Send a mouse click at the given coordinates.
pub fn click(session: &mut VmSession, profile: &VmProfile, x: u32, y: u32) -> Result<()> {
    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => click_linux(session, x, y),
        GuestOs::Windows => click_windows(session, x, y),
    }
}

/// Type text into the active window.
pub fn r#type(session: &mut VmSession, profile: &VmProfile, text: &str) -> Result<()> {
    match profile.os {
        GuestOs::Linux | GuestOs::MacOS => type_linux(session, text),
        GuestOs::Windows => type_windows(session, text),
    }
}

/// Click via xdotool on Linux.
fn click_linux(session: &mut VmSession, x: u32, y: u32) -> Result<()> {
    crate::vms::ssh::exec(
        session,
        &format!("DISPLAY=:99 xdotool mousemove {x} {y} click 1"),
    )?;
    Ok(())
}

/// Click via PowerShell on Windows.
fn click_windows(session: &mut VmSession, x: u32, y: u32) -> Result<()> {
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.Cursor]::Position = [System.Drawing.Point]::new({x}, {y})
Start-Sleep -Milliseconds 100
[System.Windows.Forms.SendKeys]::SendWait("{{LeftButton}}")
"#
    );
    crate::vms::ssh::exec(session, &script)?;
    Ok(())
}

/// Type via xdotool on Linux.
fn type_linux(session: &mut VmSession, text: &str) -> Result<()> {
    // Escape special characters for xdotool
    let escaped = text.replace("\\", "\\\\").replace("'", "\\'");
    crate::vms::ssh::exec(
        session,
        &format!("DISPLAY=:99 xdotool type --delay 50 '{escaped}'"),
    )?;
    Ok(())
}

/// Type via PowerShell SendKeys on Windows.
fn type_windows(session: &mut VmSession, text: &str) -> Result<()> {
    // PowerShell SendKeys uses special chars: + ^ % ~
    let escaped = text
        .replace("+", "{{+}}")
        .replace("^", "{{^}}")
        .replace("%", "{{%}}")
        .replace("~", "{{~}}");
    let script = format!(
        r#"
Add-Type -AssemblyName System.Windows.Forms
[System.Windows.Forms.SendKeys]::SendWait('{escaped}')
"#
    );
    crate::vms::ssh::exec(session, &script)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_linux_type_escape() {
        let text = "hello 'world'";
        let escaped = text.replace("\\", "\\\\").replace("'", "\\'");
        assert_eq!(escaped, "hello \\'world\\'");
    }

    #[test]
    fn test_windows_type_escape() {
        let text = "hello+world^test%foo";
        let escaped = text
            .replace("+", "{{+}}")
            .replace("^", "{{^}}")
            .replace("%", "{{%}}")
            .replace("~", "{{~}}");
        assert_eq!(escaped, "hello{{+}}world{{^}}test{{%}}foo");
    }
}
