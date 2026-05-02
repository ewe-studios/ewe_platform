//! Display mode configuration for QEMU VMs.
//!
//! Display mode is set at launch time and cannot be changed while
//! the VM is running.

use crate::config::DisplayMode;

/// Build the `-display` QEMU argument for the given mode.
pub fn display_arg(mode: DisplayMode, vnc_offset: u32) -> Vec<String> {
    match mode {
        DisplayMode::Headless => {
            vec!["-display".to_string(), format!("vnc=:{vnc_offset}")]
        }
        DisplayMode::Headful => {
            vec!["-display".to_string(), "spice-app".to_string()]
        }
    }
}

/// Get a human-readable description of the display mode and connection info.
pub fn connection_info(mode: DisplayMode, vnc_port: u16) -> String {
    match mode {
        DisplayMode::Headless => {
            format!("VNC server on 127.0.0.1:{vnc_port} — connect with: vncviewer 127.0.0.1:{vnc_port}")
        }
        DisplayMode::Headful => {
            "SPICE window will open automatically".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_arg_headless() {
        let args = display_arg(DisplayMode::Headless, 0);
        assert_eq!(args, vec!["-display", "vnc=:0"]);
    }

    #[test]
    fn test_display_arg_headful() {
        let args = display_arg(DisplayMode::Headful, 0);
        assert_eq!(args, vec!["-display", "spice-app"]);
    }

    #[test]
    fn test_connection_info_headless() {
        let info = connection_info(DisplayMode::Headless, 5900);
        assert!(info.contains("5900"));
        assert!(info.contains("vncviewer"));
    }
}
