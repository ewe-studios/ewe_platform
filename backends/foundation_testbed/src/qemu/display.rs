//! Display mode configuration for QEMU VMs.
//!
//! Both headless and headful modes use VNC. Headful mode also
//! auto-launches a VNC viewer (gvncviewer) for a visible display.

use crate::config::DisplayMode;

/// Build the `-display` QEMU argument for the given mode.
pub fn display_arg(mode: DisplayMode, vnc_offset: u32) -> Vec<String> {
    // Both modes use VNC — headful additionally auto-launches a viewer
    vec!["-display".to_string(), format!("vnc=:{vnc_offset}")]
}

/// Get a human-readable description of the display mode and connection info.
pub fn connection_info(mode: DisplayMode, vnc_port: u16) -> String {
    match mode {
        DisplayMode::Headless => {
            format!("VNC server on 127.0.0.1:{vnc_port}")
        }
        DisplayMode::Headful => {
            format!("VNC server on 127.0.0.1:{vnc_port} — connecting with gvncviewer...")
        }
    }
}

/// Launch a VNC viewer for headful mode (non-blocking).
pub fn launch_viewer(_mode: DisplayMode, vnc_port: u16) {
    // Auto-launch gvncviewer for headful mode
    let _ = std::process::Command::new("gvncviewer")
        .arg(format!("127.0.0.1:{}", vnc_port - 5900))
        .spawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_arg_uses_vnc() {
        let args = display_arg(DisplayMode::Headless, 0);
        assert_eq!(args, vec!["-display", "vnc=:0"]);
    }

    #[test]
    fn test_connection_info_has_port() {
        let info = connection_info(DisplayMode::Headless, 5900);
        assert!(info.contains("5900"));
    }
}
