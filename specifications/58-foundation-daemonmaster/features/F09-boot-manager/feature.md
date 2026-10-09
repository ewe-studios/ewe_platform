---
workspace_name: "ewe_platform"
spec_directory: "specifications/58-foundation-daemonmaster"
feature_directory: "specifications/58-foundation-daemonmaster/features/F09-boot-manager"
this_file: "specifications/58-foundation-daemonmaster/features/F09-boot-manager/feature.md"

status: planned
priority: medium
created: 2026-07-18

depends_on:
  - "F02-process-lifecycle"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F09 — Boot-time auto-registration (systemd, LaunchAgent, Windows Run)

## Overview

Register any binary for boot-time auto-start via platform-native mechanisms.
**Public reusable APIs** — any binary can use `BootRegistrar`, not just the daemon
supervisor. Cross-level protection prevents orphaned registrations.

[spec](../spec.md).

---

## Part A — BootRegistrar trait (public, reusable)

```rust
// foundation_nativeapis/src/daemon/boot.rs

/// Platform-agnostic boot registration interface.
///
/// Any binary can implement this to register itself for auto-start at boot time.
pub trait BootRegistrar: Send + Sync {
    /// Register for boot-time auto-start.
    fn register(&self, config: BootConfig) -> Result<(), BootError>;
    /// Unregister from boot-time auto-start.
    fn unregister(&self) -> Result<(), BootError>;
    /// Check if currently registered.
    fn is_registered(&self) -> bool;
}

/// Boot registration configuration.
pub struct BootConfig {
    /// Binary path to execute.
    pub binary: std::path::PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Service name/identifier.
    pub service_name: String,
    /// Display name (for LaunchAgent).
    pub display_name: Option<String>,
    /// Working directory.
    pub working_dir: Option<std::path::PathBuf>,
    /// Environment variables.
    pub env: std::collections::BTreeMap<String, String>,
    /// Run as root/system vs. user session.
    pub privilege: BootPrivilege,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootPrivilege {
    /// User-level (user session, no root).
    User,
    /// System-level (root, system-wide).
    System,
}
```

---

## Part B — Linux systemd registrar

```rust
/// Systemd-based boot registration.
pub struct SystemdRegistrar;

impl BootRegistrar for SystemdRegistrar {
    fn register(&self, config: BootConfig) -> Result<(), BootError> {
        let unit_path = match config.privilege {
            BootPrivilege::System => "/etc/systemd/system/",
            BootPrivilege::User => {
                let dir = dirs::config_dir()
                    .ok_or(BootError::NoConfigDir)?
                    .join("systemd/user");
                std::fs::create_dir_all(&dir)?;
                dir
            }
        };

        let unit_file = unit_path.join(format!("{}.service", config.service_name));

        let unit_content = format!(
            "[Unit]
Description={}
After=network.target

[Service]
Type=simple
ExecStart={} {}
WorkingDirectory={}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
",
            config.display_name.as_deref().unwrap_or(&config.service_name),
            config.binary.display(),
            config.args.join(" "),
            config.working_dir.as_deref().unwrap_or(std::path::Path::new("/")).display(),
        );

        std::fs::write(&unit_file, unit_content)?;

        // Reload systemd and enable.
        if config.privilege == BootPrivilege::System {
            std::process::Command::new("systemctl")
                .args(["daemon-reload"])
                .status()?;
            std::process::Command::new("systemctl")
                .args(["enable", &config.service_name])
                .status()?;
        } else {
            std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status()?;
            std::process::Command::new("systemctl")
                .args(["--user", "enable", &config.service_name])
                .status()?;
        }

        Ok(())
    }

    fn unregister(&self) -> Result<(), BootError> {
        // Disable and remove unit file.
        // Handles both user and system level.
    }

    fn is_registered(&self) -> bool {
        // Check if unit file exists and is enabled.
    }
}
```

---

## Part C — macOS LaunchAgent registrar

```rust
/// macOS LaunchAgent-based boot registration.
pub struct LaunchAgentRegistrar;

impl BootRegistrar for LaunchAgentRegistrar {
    fn register(&self, config: BootConfig) -> Result<(), BootError> {
        let plist_path = match config.privilege {
            BootPrivilege::User => {
                let dir = dirs::home_dir()
                    .ok_or(BootError::NoHomeDir)?
                    .join("Library/LaunchAgents");
                std::fs::create_dir_all(&dir)?;
                dir.join(format!("{}.plist", config.service_name))
            }
            BootPrivilege::System => {
                std::path::PathBuf::from("/Library/LaunchAgents")
                    .join(format!("{}.plist", config.service_name))
            }
        };

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        {args}
    </array>
    <key>WorkingDirectory</key>
    <string>{workdir}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>"#,
            label = config.service_name,
            binary = config.binary.display(),
            args = config.args.iter()
                .map(|a| format!("<string>{}</string>", a))
                .collect::<Vec<_>>()
                .join("\n        "),
            workdir = config.working_dir
                .as_deref()
                .unwrap_or(std::path::Path::new("/"))
                .display(),
        );

        std::fs::write(&plist_path, plist_content)?;

        // Load via launchctl.
        std::process::Command::new("launchctl")
            .args(["load", plist_path.to_str().unwrap()])
            .status()?;

        Ok(())
    }

    fn unregister(&self) -> Result<(), BootError> {
        // Unload via launchctl and remove plist.
    }

    fn is_registered(&self) -> bool {
        // Check if plist file exists and is loaded.
    }
}
```

---

## Part D — Windows Run key registrar

```rust
/// Windows Registry Run key-based boot registration.
#[cfg(target_os = "windows")]
pub struct WindowsRunRegistrar;

#[cfg(target_os = "windows")]
impl BootRegistrar for WindowsRunRegistrar {
    fn register(&self, config: BootConfig) -> Result<(), BootError> {
        use winreg::RegKey;
        use winreg::enums::*;

        let key_path = match config.privilege {
            BootPrivilege::User => r"Software\Microsoft\Windows\CurrentVersion\Run",
            BootPrivilege::System => r"Software\Microsoft\Windows\CurrentVersion\Run",
        };

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu.create_subkey(key_path)?;

        let cmd_line = format!(
            "\"{}\" {}",
            config.binary.display(),
            config.args.join(" "),
        );

        key.set_value(&config.service_name, &cmd_line)?;

        Ok(())
    }

    fn unregister(&self) -> Result<(), BootError> {
        // Delete value from Run key.
    }

    fn is_registered(&self) -> bool {
        // Check if value exists in Run key.
    }
}
```

---

## Part E — Cross-level protection

Prevent orphaned registrations when switching privilege levels:

```rust
impl BootRegistrar {
    /// Register with cross-level protection.
    ///
    /// Errors if already registered at the OTHER privilege level.
    /// Cleans up both levels on unregister to prevent orphans.
    pub fn register_protected(&self, config: BootConfig) -> Result<(), BootError> {
        // Check if registered at other privilege level.
        let other_privilege = match config.privilege {
            BootPrivilege::User => BootPrivilege::System,
            BootPrivilege::System => BootPrivilege::User,
        };

        if self.is_registered_at(other_privilege) {
            return Err(BootError::AlreadyRegisteredAtOtherLevel(other_privilege));
        }

        self.register(config)
    }

    /// Unregister from BOTH privilege levels.
    pub fn unregister_all(&self) -> Result<(), BootError> {
        // Clean up user level.
        let _ = self.unregister_at(BootPrivilege::User);
        // Clean up system level.
        let _ = self.unregister_at(BootPrivilege::System);
        Ok(())
    }

    fn is_registered_at(&self, privilege: BootPrivilege) -> bool;
    fn unregister_at(&self, privilege: BootPrivilege) -> Result<(), BootError>;
}
```

---

## Part F — Supervisor boot mode

When the supervisor starts with `--boot` flag, it runs daemons marked as boot-start:

```toml
# daemon.toml
[daemons.api]
run = ["cargo", "run", "--bin", "api-server"]
boot_start = true  # start at boot time
```

```rust
impl Supervisor {
    /// Start daemons marked as boot_start.
    pub async fn boot_start(&self) -> Result<(), SupervisorError> {
        let daemons = self.daemons.lock().await;
        let boot_daemons: Vec<_> = daemons.iter()
            .filter(|(_, m)| m.def.boot_start)
            .collect();

        // Start in dependency order (only boot daemons).
        let levels = startup_order(&boot_daemons)?;
        for level in levels {
            for id in level {
                self.spawn_daemon(&id).await?;
            }
        }
        Ok(())
    }
}
```

---

## Verification

```bash
cargo test --package foundation_nativeapis --features daemon,daemon-boot -- daemon::boot
```

Tests cover:
- Systemd registrar: unit file creation, daemon-reload, enable
- Systemd registrar: unregister removes unit file
- LaunchAgent registrar: plist creation, launchctl load
- Cross-level protection: register at user → try system → error
- Cross-level protection: unregister_all cleans both levels
- BootConfig parsing from TOML (boot_start flag)
- Supervisor boot_start: only boots daemons marked boot_start
