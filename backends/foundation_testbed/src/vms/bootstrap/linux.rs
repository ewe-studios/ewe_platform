//! Linux bootstrap via SSH — step-by-step idempotent.

use crate::vms::bootstrap::{BOOTSTRAP_MISE_TOML, logger};
use crate::vms::bootstrap::BootstrapLogger;
use crate::vms::config::{Result, VmProfile};
use crate::vms::ssh::VmSession;

const INSTALL_SYSTEM_DEPS_SH: &str = include_str!("../../../scripts/linux/install_system_deps.sh");
const INSTALL_DEV_DEPS_SH: &str = include_str!("../../../scripts/linux/install_dev_deps.sh");
const INSTALL_MISE_SH: &str = include_str!("../../../scripts/linux/install_mise.sh");
const ACTIVATE_MISE_BASHRC_SH: &str = include_str!("../../../scripts/linux/activate_mise_bashrc.sh");
const INSTALL_CARGO_BINSTALL_SH: &str = include_str!("../../../scripts/linux/install_cargo_binstall.sh");
const CONFIGURE_MISE_CARGO_BINSTALL_SH: &str = include_str!("../../../scripts/linux/configure_mise_cargo_binstall.sh");
const INSTALL_TOOLS_MISE_SH: &str = include_str!("../../../scripts/linux/install_tools_mise.sh");
const SET_NUSHELL_DEFAULT_SHELL_SH: &str = include_str!("../../../scripts/linux/set_nushell_default_shell.sh");
const SETUP_SSH_KEYS_SH: &str = include_str!("../../../scripts/linux/setup_ssh_keys.sh");
const SETUP_PROJECT_MOUNT_SH: &str = include_str!("../../../scripts/linux/setup_project_mount.sh");
const INSTALL_GUI_SH: &str = include_str!("../../../scripts/linux/install_gui.sh");
const INSTALL_GNOME_SH: &str = include_str!("../../../scripts/linux/install_gnome.sh");
const START_DISPLAY_MANAGER_SH: &str = include_str!("../../../scripts/linux/start_display_manager.sh");
const START_GNOME_DM_SH: &str = include_str!("../../../scripts/linux/start_gnome_dm.sh");

/// Detect the home directory of the current user on the VM.
fn detect_home_dir(session: &mut VmSession) -> Result<String> {
    let home = crate::vms::ssh::exec(session, "echo $HOME")?;
    Ok(home.trim().to_string())
}

/// Replace hardcoded /home/vagrant paths with the detected home directory.
fn adapt_script(script: &str, home_dir: &str) -> String {
    script.replace("/home/vagrant/", &format!("{}/", home_dir))
}

pub fn bootstrap_linux(_profile: &VmProfile, session: &mut VmSession, logger: &BootstrapLogger) -> Result<()> {
    // Detect home directory first
    let home_dir = detect_home_dir(session)?;
    logger.message(&format!("Detected home directory: {}", home_dir));

    logger::step(logger, "install system deps", || {
        let script = adapt_script(INSTALL_SYSTEM_DEPS_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    // Install comprehensive dev dependencies (Rust, LLVM, GCC, ARM cross-compile, Tauri)
    logger::step(logger, "install dev dependencies", || {
        // Check if already installed by looking for key tools
        let has_llvm = crate::vms::ssh::exec(
            session,
            "command -v llvm-ar >/dev/null 2>&1 && echo present || echo missing",
        ).unwrap_or_default();
        let has_arm_gcc = crate::vms::ssh::exec(
            session,
            "command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 && echo present || echo missing",
        ).unwrap_or_default();

        if !has_llvm.contains("present") || !has_arm_gcc.contains("present") {
            logger.message("  Installing dev deps (LLVM, GCC, ARM cross-compile, Tauri deps)...");
            let script = adapt_script(INSTALL_DEV_DEPS_SH, &home_dir);
            crate::vms::ssh::exec(session, &script)?;
            logger.message("  Dev dependencies installed");
        } else {
            logger.message("  Skipping dev deps install (already present)");
        }
        Ok(())
    })?;

    logger::step(logger, "install mise", || {
        let script = adapt_script(INSTALL_MISE_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "activate mise in .bashrc", || {
        let script = adapt_script(ACTIVATE_MISE_BASHRC_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "install cargo-binstall", || {
        let present = crate::vms::ssh::exec(
            session,
            "[ -x \"$HOME/.cargo/bin/cargo-binstall\" ] && echo present || echo missing",
        )
        .unwrap_or_default();
        if present.contains("present") {
            return Ok(());
        }
        let arch = crate::vms::ssh::exec(session, "uname -m").unwrap_or_default();
        let target = if arch.trim() == "aarch64" { "aarch64-unknown-linux-musl" } else { "x86_64-unknown-linux-musl" };
        let script = adapt_script(INSTALL_CARGO_BINSTALL_SH, &home_dir).replace("{{TARGET}}", &target);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "configure mise cargo_binstall", || {
        let script = adapt_script(CONFIGURE_MISE_CARGO_BINSTALL_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "install tools via mise", || {
        let script = adapt_script(INSTALL_TOOLS_MISE_SH, &home_dir).replace("{{MISE_TOML}}", BOOTSTRAP_MISE_TOML);
        crate::vms::ssh::exec(session, &script)?;
        crate::vms::ssh::exec(session, "$HOME/.local/bin/mise exec -- rustc --version")?;
        Ok(())
    })?;

    logger::step(logger, "set nushell as default shell", || {
        let script = adapt_script(SET_NUSHELL_DEFAULT_SHELL_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    logger::step(logger, "authorise host SSH key", || {
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/darkvoid"));
        let key_names = ["id_ed25519.pub", "id_rsa.pub", "id_ecdsa.pub"];
        let mut pub_key = String::new();
        for key_name in key_names {
            let path = home.join(".ssh").join(key_name);
            if path.exists() && let Ok(key) = std::fs::read_to_string(&path) {
                pub_key = key.trim().to_string();
                break;
            }
        }
        if !pub_key.is_empty() {
            let script = adapt_script(SETUP_SSH_KEYS_SH, &home_dir).replace("{{KEY}}", &pub_key);
            crate::vms::ssh::exec(session, &script)?;
        }
        Ok(())
    })?;

    logger::step(logger, "set up project mount", || {
        let script = adapt_script(SETUP_PROJECT_MOUNT_SH, &home_dir);
        crate::vms::ssh::exec(session, &script)?;
        Ok(())
    })?;

    // Optional: Install GUI packages (not enabled by default)
    // This can be enabled via a profile flag or environment variable
    logger::step(logger, "install gui (optional)", || {
        if std::env::var("TESTBED_INSTALL_GNOME").is_ok() {
            logger.message("  Installing GNOME desktop environment (this may take 5-10 minutes)...");
            // Write script locally and upload via SCP (now with key auth)
            let temp_path = std::env::temp_dir().join("install_gnome.sh");
            let script = adapt_script(INSTALL_GNOME_SH, &home_dir);
            std::fs::write(&temp_path, script)
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "write gnome install script".to_string(),
                    message: e.to_string(),
                })?;
            crate::vms::ssh::upload(session, &temp_path, "/tmp/install_gnome.sh")
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "upload gnome install script".to_string(),
                    message: format!("{:?}", e),
                })?;
            // Use streaming exec to see real-time output
            let exit_code = crate::vms::ssh::streaming::exec_streaming_session(
                session,
                "chmod +x /tmp/install_gnome.sh && sudo /tmp/install_gnome.sh"
            )?;
            if exit_code != 0 {
                return Err(crate::vms::config::TestbedError::BootstrapFailed {
                    step: "install gnome".to_string(),
                    message: format!("GNOME install script exited with code {}", exit_code),
                });
            }
            logger.message("  GNOME desktop environment installed");
        } else if std::env::var("TESTBED_INSTALL_GUI").is_ok() {
            logger.message("  Installing GUI environment (this may take a few minutes)...");
            // Write script locally and upload via SCP (now with key auth)
            let temp_path = std::env::temp_dir().join("install_gui.sh");
            let script = adapt_script(INSTALL_GUI_SH, &home_dir);
            std::fs::write(&temp_path, script)
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "write gui install script".to_string(),
                    message: e.to_string(),
                })?;
            crate::vms::ssh::upload(session, &temp_path, "/tmp/install_gui.sh")
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "upload gui install script".to_string(),
                    message: format!("{:?}", e),
                })?;
            // Use streaming exec to see real-time output
            let exit_code = crate::vms::ssh::streaming::exec_streaming_session(
                session,
                "chmod +x /tmp/install_gui.sh && sudo /tmp/install_gui.sh"
            )?;
            if exit_code != 0 {
                return Err(crate::vms::config::TestbedError::BootstrapFailed {
                    step: "install gui".to_string(),
                    message: format!("GUI install script exited with code {}", exit_code),
                });
            }
            logger.message("  GUI environment installed");
        } else {
            logger.message("  Skipping GUI install (set TESTBED_INSTALL_GUI=1 or TESTBED_INSTALL_GNOME=1 to enable)");
        }
        Ok(())
    })?;

    // Optional: Start the display manager (requires GUI to be installed)
    logger::step(logger, "start display manager", || {
        if std::env::var("TESTBED_START_GUI").is_ok() || std::env::var("TESTBED_INSTALL_GUI").is_ok() || std::env::var("TESTBED_INSTALL_GNOME").is_ok() {
            // Determine which display manager script to use
            let (script_name, script_content, dm_name) = if std::env::var("TESTBED_INSTALL_GNOME").is_ok() {
                ("start_gnome_dm.sh", START_GNOME_DM_SH, "GDM")
            } else {
                ("start_dm.sh", START_DISPLAY_MANAGER_SH, "LightDM")
            };

            logger.message(&format!("  Starting display manager ({})...", dm_name));
            // Write script locally and upload via SCP (now with key auth)
            let temp_path = std::env::temp_dir().join(script_name);
            let script = adapt_script(script_content, &home_dir);
            std::fs::write(&temp_path, script)
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "write display manager script".to_string(),
                    message: e.to_string(),
                })?;
            crate::vms::ssh::upload(session, &temp_path, &format!("/tmp/{}", script_name))
                .map_err(|e| crate::vms::config::TestbedError::BootstrapFailed {
                    step: "upload display manager script".to_string(),
                    message: format!("{:?}", e),
                })?;
            // Use streaming exec to see real-time output
            match crate::vms::ssh::streaming::exec_streaming_session(
                session,
                &format!("chmod +x /tmp/{} && sudo /tmp/{}", script_name, script_name)
            ) {
                Ok(exit_code) => {
                    if exit_code != 0 {
                        logger.message(&format!("  Warning: Display manager script exited with code {}", exit_code));
                        logger.message(&format!("  You can start it manually later with: sudo systemctl start {}", dm_name.to_lowercase()));
                    }
                }
                Err(e) => {
                    logger.message(&format!("  Warning: Could not start display manager: {}", e));
                    logger.message(&format!("  You can start it manually later with: sudo systemctl start {}", dm_name.to_lowercase()));
                }
            }
        } else {
            logger.message("  Skipping display manager start (set TESTBED_START_GUI=1 to enable)");
        }
        Ok(())
    })?;

    logger::step(logger, "write bootstrap marker", || {
        crate::vms::ssh::exec(session, "touch ~/.testbed-bootstrapped")?;
        Ok(())
    })?;

    Ok(())
}

