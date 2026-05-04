//! User-mode networking configuration and dynamic port allocation.

use std::net::TcpListener;

use crate::config::{Result, TestbedError, VmProfile};

use super::ResolvedPorts;

/// Allocate free ports for the VM based on the profile's defaults.
///
/// If a default port is in use, scans upward (up to +100) to find a free one.
pub fn allocate_ports(profile: &VmProfile) -> Result<ResolvedPorts> {
    let ssh_port = allocate_port(profile.ssh_port)?;

    let winrm_port = profile.winrm_port.map(allocate_port).transpose()?;
    let rdp_port = profile.rdp_port.map(allocate_port).transpose()?;

    // VNC port is 5900 + offset. The profile's vnc_port IS the actual port number.
    let vnc_port = allocate_port(profile.vnc_port)?;

    Ok(ResolvedPorts {
        ssh_port,
        winrm_port,
        rdp_port,
        vnc_port,
    })
}

/// Find a free port starting from `start`, scanning up to +100.
pub fn allocate_port(start: u16) -> Result<u16> {
    for port in start..=start + 100 {
        if is_port_free(port) {
            return Ok(port);
        }
    }
    Err(TestbedError::PortInUse { port: start })
}

/// Check if a TCP port is free on 127.0.0.1.
fn is_port_free(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_port_returns_free_port() {
        // Port 0 is always free (OS picks one), but let's use a high port
        let port = allocate_port(30000).unwrap();
        assert!(port >= 30000);
        // The returned port should be free (we just allocated it, so it was free at check time)
        assert!(is_port_free(port));
    }

    #[test]
    fn test_is_port_free() {
        // Bind a port, verify it shows as in-use
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(!is_port_free(port), "bound port should not be free");
        drop(listener);
        assert!(is_port_free(port), "port should be free after drop");
    }
}
