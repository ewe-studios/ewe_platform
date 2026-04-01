//! Project scanning and metadata detection.
//!
//! WHY: Before deploying, the system needs to discover what kind of project
//! it's working with — which provider, what language, what tooling is present.
//!
//! WHAT: `ProjectScanner` examines a directory to build a `ProjectInfo` struct
//! with provider target, project name, and tooling presence flags.
//!
//! HOW: Check for known config files and build tool markers in the project root.

use std::path::{Path, PathBuf};

use crate::config::DeploymentTarget;
use crate::error::DeploymentError;

/// Scanned project information (provider-agnostic).
#[derive(Debug)]
pub struct ProjectInfo {
    /// Project name (derived from directory name or config).
    pub name: String,
    /// Root directory of the project.
    pub root_dir: PathBuf,
    /// Detected deployment target.
    pub target: DeploymentTarget,
    /// Whether a `Cargo.toml` exists (Rust project).
    pub has_cargo_toml: bool,
    /// Whether a `Dockerfile` exists.
    pub has_dockerfile: bool,
    /// Whether a `mise.toml` exists.
    pub has_mise_toml: bool,
    /// Path to the provider-specific config file.
    pub config_file: PathBuf,
}

/// Scans a project directory to detect provider and gather metadata.
pub struct ProjectScanner;

impl ProjectScanner {
    /// Scan a project directory, detect provider, and gather metadata.
    ///
    /// # Errors
    ///
    /// Returns `DeploymentError::NoProviderDetected` if no recognized
    /// config file is found in `root`.
    pub fn scan(root: &Path) -> Result<ProjectInfo, DeploymentError> {
        let target = DeploymentTarget::detect(root).ok_or_else(|| {
            DeploymentError::NoProviderDetected {
                project_dir: root.display().to_string(),
            }
        })?;

        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());

        Ok(ProjectInfo {
            name,
            root_dir: root.to_path_buf(),
            target,
            has_cargo_toml: root.join("Cargo.toml").exists(),
            has_dockerfile: root.join("Dockerfile").exists(),
            has_mise_toml: root.join("mise.toml").exists(),
            config_file: root.join(target.config_file()),
        })
    }
}
