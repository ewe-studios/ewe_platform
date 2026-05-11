//! Host and VM health checks.
//!
//! Validates that required system tools, KVM, disk space, and VM
//! services are available and functioning.

use crate::config::{self, GuestOs, VmProfile};

/// Status of a single health check.
#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub message: String,
    pub suggestion: Option<&'static str>,
}

/// Result of running all host-level health checks.
#[derive(Debug)]
pub struct HostHealth {
    pub checks: Vec<Check>,
}

impl HostHealth {
    pub fn is_healthy(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }

    pub fn print(&self) {
        println!("=== Host Health ===");
        for c in &self.checks {
            let status = if c.ok { "OK" } else { "FAIL" };
            println!("  [{status}] {} — {}", c.name, c.message);
            if let Some(s) = c.suggestion {
                println!("         Suggestion: {}", s);
            }
        }

        let available = self.checks.iter().find(|c| c.name == "disk_space");
        if let Some(c) = available {
            println!("  {}", c.message);
        }
    }
}

/// Run all host-level health checks.
pub fn check_host() -> HostHealth {
    let mut checks = Vec::new();

    // Host prerequisites: mise, nushell, pitchfork
    let prereqs = crate::host_bootstrap::check_host_prerequisites();
    checks.push(Check {
        name: "mise",
        ok: prereqs.mise.is_available(),
        message: match &prereqs.mise {
            crate::host_bootstrap::ToolStatus::AlreadyPresent { version }
            | crate::host_bootstrap::ToolStatus::Installed { version } => {
                format!("mise {version}")
            }
            crate::host_bootstrap::ToolStatus::Missing { error } => error.clone(),
        },
        suggestion: Some("run: ewe_platform testbed doctor --install to auto-install"),
    });
    checks.push(Check {
        name: "nushell",
        ok: prereqs.nushell.is_available(),
        message: match &prereqs.nushell {
            crate::host_bootstrap::ToolStatus::AlreadyPresent { version }
            | crate::host_bootstrap::ToolStatus::Installed { version } => {
                format!("nushell {version}")
            }
            crate::host_bootstrap::ToolStatus::Missing { error } => error.clone(),
        },
        suggestion: Some("run: ewe_platform testbed doctor --install to auto-install"),
    });
    checks.push(Check {
        name: "pitchfork",
        ok: prereqs.pitchfork.is_available(),
        message: match &prereqs.pitchfork {
            crate::host_bootstrap::ToolStatus::AlreadyPresent { version }
            | crate::host_bootstrap::ToolStatus::Installed { version } => {
                format!("pitchfork {version}")
            }
            crate::host_bootstrap::ToolStatus::Missing { error } => error.clone(),
        },
        suggestion: Some("run: ewe_platform testbed doctor --install to auto-install (optional)"),
    });

    // KVM
    let kvm_ok = kvm_available();
    checks.push(Check {
        name: "kvm",
        ok: kvm_ok,
        message: if kvm_ok {
            "/dev/kvm is available".to_string()
        } else {
            "KVM not available".to_string()
        },
        suggestion: Some("ensure kvm_intel/kvm_amd kernel module is loaded"),
    });

    // qemu-system-x86_64
    let qemu_ok = which::which("qemu-system-x86_64").is_ok();
    checks.push(Check {
        name: "qemu-system-x86_64",
        ok: qemu_ok,
        message: if qemu_ok {
            "found on PATH".to_string()
        } else {
            "not found on PATH".to_string()
        },
        suggestion: Some("run: mise install qemu or pacman -S qemu-base"),
    });

    // qemu-img
    let qemu_img_ok = which::which("qemu-img").is_ok();
    checks.push(Check {
        name: "qemu-img",
        ok: qemu_img_ok,
        message: if qemu_img_ok {
            "found on PATH".to_string()
        } else {
            "not found on PATH".to_string()
        },
        suggestion: Some("run: mise install qemu or pacman -S qemu-base"),
    });

    // SSH key
    let ssh_key = find_ssh_key();
    let has_key = ssh_key.is_some();
    checks.push(Check {
        name: "ssh_key",
        ok: has_key,
        message: if has_key {
            format!("found: {}", ssh_key.unwrap().display())
        } else {
            "no SSH key found".to_string()
        },
        suggestion: Some("run: ssh-keygen -t ed25519"),
    });

    // Disk space — fs2 requires the path to exist, so walk up to first existing parent
    let cache_dir = config::cache_dir();
    let disk_target = if cache_dir.exists() {
        cache_dir
    } else {
        cache_dir.ancestors().find(|p| p.exists()).map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("/"))
    };
    let disk_msg = match fs2::available_space(&disk_target) {
        Ok(bytes) => {
            let gb = bytes as f64 / 1_073_741_824.0;
            let ok = gb > 10.0;
            Check {
                name: "disk_space",
                ok,
                message: format!("{:.1} GB available on {}", gb, disk_target.display()),
                suggestion: if !ok { Some("free up disk space or expand partition") } else { None },
            }
        }
        Err(_) => Check {
            name: "disk_space",
            ok: false,
            message: "could not determine available disk space".to_string(),
            suggestion: None,
        },
    };
    checks.push(disk_msg);

    // Port availability (check default profile ports)
    for profile in config::list_profiles() {
        let port_free = is_port_free(profile.ssh_port);
        if !port_free {
            checks.push(Check {
                name: "port_check",
                ok: false,
                message: format!("default SSH port {} (profile '{}') is in use", profile.ssh_port, profile.name),
                suggestion: Some("stop the running VM or override the port in testbed.toml"),
            });
        }
    }

    HostHealth { checks }
}

/// Run VM-level health checks for a specific profile.
pub fn check_vm(profile: &VmProfile) -> HostHealth {
    let mut checks = Vec::new();

    // State check
    let running = crate::state::is_running(profile.name);
    checks.push(Check {
        name: "running",
        ok: running,
        message: if running {
            format!("VM '{}' is running", profile.name)
        } else {
            format!("VM '{}' is not running", profile.name)
        },
        suggestion: Some("run: ewe_platform testbed start <profile>"),
    });

    if !running {
        return HostHealth { checks };
    }

    // SSH reachability
    match crate::ssh::check(profile) {
        Ok(()) => checks.push(Check {
            name: "ssh",
            ok: true,
            message: "SSH is reachable".to_string(),
            suggestion: None,
        }),
        Err(_) => checks.push(Check {
            name: "ssh",
            ok: false,
            message: "SSH is not reachable".to_string(),
            suggestion: Some("wait for VM to finish booting, then try again"),
        }),
    }

    // WinRM (Windows only)
    if profile.os == GuestOs::Windows {
        match crate::winrm::WinRM::from_profile(profile) {
            Ok(winrm) => {
                if winrm.ping() {
                    checks.push(Check {
                        name: "winrm",
                        ok: true,
                        message: "WinRM is reachable".to_string(),
                        suggestion: None,
                    });
                } else {
                    checks.push(Check {
                        name: "winrm",
                        ok: false,
                        message: "WinRM responded but may not be functional".to_string(),
                        suggestion: None,
                    });
                }
            }
            Err(_) => checks.push(Check {
                name: "winrm",
                ok: false,
                message: "WinRM port not configured for this profile".to_string(),
                suggestion: None,
            }),
        }
    }

    // Bootstrap status
    if crate::bootstrap::is_bootstrapped(profile) {
        checks.push(Check {
            name: "bootstrapped",
            ok: true,
            message: "VM has been bootstrapped".to_string(),
            suggestion: None,
        });
    } else {
        checks.push(Check {
            name: "bootstrapped",
            ok: false,
            message: "VM has not been bootstrapped yet".to_string(),
            suggestion: Some("bootstrap will run automatically on first start"),
        });
    }

    HostHealth { checks }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Check if KVM is available.
pub fn kvm_available() -> bool {
    let path = std::path::Path::new("/dev/kvm");
    path.exists() && std::fs::metadata(path).map(|m| !m.permissions().readonly()).unwrap_or(false)
}

/// Find the user's SSH public key (ed25519 preferred).
fn find_ssh_key() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    let ssh_dir = home.join(".ssh");

    for name in ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"] {
        let path = ssh_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Check if a TCP port is free on localhost.
fn is_port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_health_returns_checks() {
        let health = check_host();
        assert!(!health.checks.is_empty());
        // Should have at least kvm, qemu, qemu-img, ssh_key, disk_space
        assert!(health.checks.len() >= 5);
    }

    #[test]
    fn test_kvm_available() {
        // Just verify it doesn't panic
        let _ = kvm_available();
    }

    #[test]
    fn test_find_ssh_key() {
        // Just verify it doesn't panic
        let _ = find_ssh_key();
    }
}
