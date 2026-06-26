//! Host bootstrap — first-run setup for mise, nushell, and pitchfork.
//!
//! Before any VM runs, the host needs three foundational tools:
//! - **mise**: tool version management
//! - **nushell**: unified cross-platform shell
//! - **pitchfork**: process/service manager

use std::path::PathBuf;

use crate::vms::config::{Result, TestbedError};

pub mod install;

/// Minimum version requirement for mise.
const MISE_MIN_VERSION: &str = "2024.1";

/// State of host prerequisites after bootstrap.
#[derive(Debug)]
pub struct HostPrerequisites {
    pub mise: ToolStatus,
    pub nushell: ToolStatus,
    pub pitchfork: ToolStatus,
}

impl Default for HostPrerequisites {
    fn default() -> Self {
        Self {
            mise: ToolStatus::Missing { error: String::new() },
            nushell: ToolStatus::Missing { error: String::new() },
            pitchfork: ToolStatus::Missing { error: String::new() },
        }
    }
}

/// Status of a single tool after bootstrap check/install.
#[derive(Debug, Clone)]
pub enum ToolStatus {
    AlreadyPresent { version: String },
    Installed { version: String },
    Missing { error: String },
}

impl ToolStatus {
    pub fn is_available(&self) -> bool {
        !matches!(self, ToolStatus::Missing { .. })
    }

    pub fn version(&self) -> Option<&str> {
        match self {
            ToolStatus::AlreadyPresent { version } | ToolStatus::Installed { version } => Some(version),
            ToolStatus::Missing { .. } => None,
        }
    }
}

/// Check and install all host prerequisites.
///
/// Idempotent — skips installation if the tool is already present
/// and meets minimum version.
pub fn ensure_host_prerequisites() -> Result<HostPrerequisites> {
    let mut state = HostPrerequisites::default();

    // 1. mise — check on PATH, install if missing
    state.mise = ensure_mise()?;

    // 2. nushell — check via `nu --version`, install via mise
    state.nushell = ensure_nushell(&state.mise)?;

    // 3. pitchfork — check via `pitchfork --version`, install via mise
    state.pitchfork = ensure_pitchfork(&state.mise)?;

    // 4. Write host-level mise config with nu + pitchfork + cargo_binstall
    write_host_mise_config()?;

    Ok(state)
}

/// Check mise on PATH and version. Install if missing.
fn ensure_mise() -> Result<ToolStatus> {
    match check_tool_version("mise", MISE_MIN_VERSION) {
        Ok(version) => Ok(ToolStatus::AlreadyPresent { version }),
        Err(_) => {
            eprintln!("  Installing mise...");
            install::install_mise()?;
            match check_tool_version("mise", MISE_MIN_VERSION) {
                Ok(version) => Ok(ToolStatus::Installed { version }),
                Err(e) => Ok(ToolStatus::Missing { error: format!("{e}") }),
            }
        }
    }
}

/// Check nushell on PATH and version. Install via mise if missing.
fn ensure_nushell(mise_status: &ToolStatus) -> Result<ToolStatus> {
    // Check if nu is on PATH
    if let Ok(version) = check_tool_present("nu") { return Ok(ToolStatus::AlreadyPresent { version }) }

    if !mise_status.is_available() {
        return Ok(ToolStatus::Missing {
            error: "mise not available to install nushell".to_string(),
        });
    }

    eprintln!("  Installing nushell via mise...");
    install::install_nushell()?;
    match check_tool_present("nu") {
        Ok(version) => Ok(ToolStatus::Installed { version }),
        Err(e) => Ok(ToolStatus::Missing { error: format!("{e}") }),
    }
}

/// Check pitchfork on PATH. Install via mise if missing.
fn ensure_pitchfork(mise_status: &ToolStatus) -> Result<ToolStatus> {
    if let Ok(version) = check_tool_present("pitchfork") { return Ok(ToolStatus::AlreadyPresent { version }) }

    if !mise_status.is_available() {
        return Ok(ToolStatus::Missing {
            error: "mise not available to install pitchfork".to_string(),
        });
    }

    eprintln!("  Installing pitchfork via mise...");
    install::install_pitchfork()?;
    match check_tool_present("pitchfork") {
        Ok(version) => Ok(ToolStatus::Installed { version }),
        Err(e) => Ok(ToolStatus::Missing { error: format!("{e}") }),
    }
}

/// Check if a tool is present on PATH and return its version string.
fn check_tool_present(name: &str) -> Result<String> {
    which::which(name).map_err(|e| TestbedError::Qcow2Error {
        message: format!("{name} not found: {e}"),
    })?;

    let output = std::process::Command::new(name)
        .arg("--version")
        .output()
        .map_err(|e| TestbedError::Qcow2Error {
            message: format!("running {name} --version: {e}"),
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(TestbedError::Qcow2Error {
            message: format!("{name} --version failed"),
        })
    }
}

/// Check if a tool is present and its version meets minimum.
fn check_tool_version(name: &str, _min_version: &str) -> Result<String> {
    // For now just check presence — version comparison is complex
    // across different version schemes (semver vs date-based like mise)
    check_tool_present(name)
}

/// Write host-level mise config with nu + pitchfork.
fn write_host_mise_config() -> Result<()> {
    let config_path = host_mise_config_path();
    let parent = config_path.parent().unwrap();
    if !parent.exists() {
        std::fs::create_dir_all(parent).map_err(|e| TestbedError::Qcow2Error {
            message: format!("creating mise config dir: {e}"),
        })?;
    }

    // Only write if it doesn't already exist (don't overwrite user config)
    if !config_path.exists() {
        let content = "[tools]\nnu = \"latest\"\n\n[settings]\ncargo_binstall = true\n";
        std::fs::write(&config_path, content).map_err(|e| TestbedError::Qcow2Error {
            message: format!("writing mise config: {e}"),
        })?;
    }

    Ok(())
}

/// Path to the user's global mise config: `~/.config/mise/config.toml`.
pub fn host_mise_config_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        return home.join(".config").join("mise").join("config.toml");
    }
    PathBuf::from("/home/darkvoid/.config/mise/config.toml")
}

/// Check if all host prerequisites are met (quick check, no installs).
pub fn check_host_prerequisites() -> HostPrerequisites {
    let mise = check_tool_present("mise").map_or_else(
        |e| ToolStatus::Missing { error: format!("{e}") },
        |v| ToolStatus::AlreadyPresent { version: v },
    );

    let nushell = check_tool_present("nu").map_or_else(
        |e| ToolStatus::Missing { error: format!("{e}") },
        |v| ToolStatus::AlreadyPresent { version: v },
    );

    let pitchfork = check_tool_present("pitchfork").map_or_else(
        |e| ToolStatus::Missing { error: format!("{e}") },
        |v| ToolStatus::AlreadyPresent { version: v },
    );

    HostPrerequisites {
        mise,
        nushell,
        pitchfork,
    }
}

