//! Display mode configuration for QEMU VMs.
//!
//! Headful mode auto-detects the best available display backend:
//! SPICE > GTK > VNC (with gvncviewer fallback).
//! Mouse tracking is fixed via USB tablet device (added in mod.rs).

use crate::vms::config::DisplayMode;

/// Available QEMU display backends, in preference order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayBackend {
    Spice,  // -display spice-app (best: clipboard, resize, audio)
    Gtk,    // -display gtk (good: native window, no separate viewer)
    Vnc,    // -display vnc=:N (needs external viewer)
}

impl DisplayBackend {
    /// QEMU `-display` arguments for this backend.
    pub fn qemu_args(&self, vnc_offset: u32) -> Vec<String> {
        match self {
            DisplayBackend::Spice => vec!["-display".to_string(), "spice-app".to_string()],
            DisplayBackend::Gtk => vec!["-display".to_string(), "gtk".to_string()],
            DisplayBackend::Vnc => vec![
                "-display".to_string(),
                format!("vnc=:{}", vnc_offset),
            ],
        }
    }
}

/// Detect the best available QEMU display backend.
///
/// Runs `qemu-system-x86_64 -display help` and checks which backends
/// are compiled in. Returns Spice if available, then Gtk, then Vnc.
pub fn detect_backend() -> DisplayBackend {
    // Check spice first (best experience)
    if has_display_backend("spice-app") {
        return DisplayBackend::Spice;
    }
    if has_display_backend("gtk") {
        return DisplayBackend::Gtk;
    }
    // VNC is always available (built into qemu-base)
    DisplayBackend::Vnc
}

/// Check if a specific display backend is available in QEMU.
fn has_display_backend(name: &str) -> bool {
    let Ok(output) = std::process::Command::new("qemu-system-x86_64")
        .arg("-display")
        .arg("help")
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| line.contains(name))
}

/// Find an available viewer for the given backend.
/// Returns (command, args) or None if no viewer found.
pub fn find_viewer(backend: DisplayBackend, vnc_port: u16) -> Option<(&'static str, Vec<String>)> {
    match backend {
        DisplayBackend::Spice => {
            // spice-app opens natively, no separate viewer needed
            None
        }
        DisplayBackend::Gtk => {
            // GTK opens natively, no separate viewer needed
            None
        }
        DisplayBackend::Vnc => {
            // Try viewers in order of preference
            let vnc_display = format!("127.0.0.1:{}", vnc_port - 5900);
            if which::which("gvncviewer").is_ok() {
                Some(("gvncviewer", vec![vnc_display]))
            } else if which::which("vinagre").is_ok() {
                Some(("vinagre", vec![vnc_display]))
            } else if which::which("xtightvncviewer").is_ok() {
                Some(("xtightvncviewer", vec![vnc_display]))
            } else if which::which("tigervncviewer").is_ok() {
                Some(("tigervncviewer", vec![vnc_display]))
            } else if which::which("vncviewer").is_ok() {
                Some(("vncviewer", vec![vnc_display]))
            } else {
                None
            }
        }
    }
}

/// Launch a viewer for headful mode (non-blocking).
/// Returns the viewer name that was launched, or None if none available.
pub fn launch_viewer(backend: DisplayBackend, vnc_port: u16) -> Option<String> {
    if backend != DisplayBackend::Vnc {
        // Spice/Gtk open their own windows natively
        return None;
    }
    let (cmd, args) = find_viewer(backend, vnc_port)?;
    let _ = std::process::Command::new(cmd).args(&args).spawn();
    Some(cmd.to_string())
}

/// Get a human-readable description of the display mode and connection info.
pub fn connection_info(mode: DisplayMode, backend: DisplayBackend, vnc_port: u16) -> String {
    match mode {
        DisplayMode::Headless => {
            match backend {
                DisplayBackend::Vnc => format!("VNC server on 127.0.0.1:{vnc_port}"),
                _ => format!("{:?} display on 127.0.0.1:{vnc_port}", backend),
            }
        }
        DisplayMode::Headful => {
            match backend {
                DisplayBackend::Spice => "SPICE window will open automatically".to_string(),
                DisplayBackend::Gtk => "GTK window will open automatically".to_string(),
                DisplayBackend::Vnc => {
                    let viewer = find_viewer(backend, vnc_port)
                        .map(|(name, _)| name)
                        .unwrap_or("none");
                    format!("Launching {viewer} for VNC on 127.0.0.1:{vnc_port}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vnc_args() {
        let backend = DisplayBackend::Vnc;
        let args = backend.qemu_args(0);
        assert_eq!(args, vec!["-display", "vnc=:0"]);
    }

    #[test]
    fn test_connection_info_has_port() {
        let info = connection_info(DisplayMode::Headless, DisplayBackend::Vnc, 5900);
        assert!(info.contains("5900"));
    }
}
