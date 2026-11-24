//! The bootstrap script.
//!
//! The script holds the manifest, writes it to disk, downloads the installer,
//! checks its SHA-256 and runs it. It does nothing else (spec 79 section 4.7).
//! `rustup` and `mise` use the same pattern.

use std::collections::BTreeMap;
use std::path::PathBuf;

use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::error::{FieldError, InstallError};

/// The script template.
const BOOTSTRAP_TEMPLATE: &str = include_str!("../templates/bootstrap.sh.j2");

/// The default path of the manifest on a host.
pub const DEFAULT_MANIFEST_PATH: &str = "/etc/ewe/manifest.json";

/// Renders the bootstrap script from `templates/bootstrap.sh.j2`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BootstrapScript;

/// Everything the script needs. Made by the release pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootstrapInput {
    /// The manifest JSON text. The script holds it as base64.
    pub manifest_json: String,
    /// Where the script writes the manifest. Default
    /// [`DEFAULT_MANIFEST_PATH`].
    pub manifest_path: PathBuf,
    /// One installer build for each target, chosen with `uname -s -m`.
    pub installers: Vec<InstallerBuild>,
}

impl BootstrapInput {
    /// An input with the default manifest path.
    #[must_use]
    pub fn new(manifest_json: impl Into<String>, installers: Vec<InstallerBuild>) -> Self {
        Self {
            manifest_json: manifest_json.into(),
            manifest_path: PathBuf::from(DEFAULT_MANIFEST_PATH),
            installers,
        }
    }
}

/// One `ewe-installer` build of a release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallerBuild {
    /// Example: `x86_64-unknown-linux-gnu`.
    pub target: String,
    /// An HTTPS URL of the `ewe-installer` binary.
    pub url: String,
    /// Its lowercase hex SHA-256. The script refuses a binary that does not
    /// match.
    pub sha256: String,
}

impl BootstrapScript {
    /// Render the script for `input`.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Parse`] when a URL is not HTTPS, a checksum is
    /// not a SHA-256, no installer is given, or a target has no `uname`
    /// mapping.
    pub fn render(input: &BootstrapInput) -> Result<String, InstallError> {
        let mut faults = Vec::new();
        if input.installers.is_empty() {
            faults.push(FieldError::new(
                "/installers",
                "expected at least one installer build",
            ));
        }
        for (index, build) in input.installers.iter().enumerate() {
            let base = format!("/installers/{index}");
            if build.target.is_empty() {
                faults.push(FieldError::new(
                    format!("{base}/target"),
                    "the target is empty",
                ));
            }
            if !build.url.starts_with("https://") {
                faults.push(FieldError::new(
                    format!("{base}/url"),
                    "expected an https URL",
                ));
            }
            if !is_sha256(&build.sha256) {
                faults.push(FieldError::new(
                    format!("{base}/sha256"),
                    "expected 64 lowercase hexadecimal characters",
                ));
            }
        }
        if !faults.is_empty() {
            return Err(InstallError::Parse(faults));
        }

        let mut builds: BTreeMap<&'static str, BuildView> = BTreeMap::new();
        for build in &input.installers {
            let os_arch = uname_pattern(&build.target).ok_or_else(|| {
                InstallError::Parse(vec![FieldError::new(
                    "/installers",
                    format!("the target {} has no uname mapping", build.target),
                )])
            })?;
            builds.entry(os_arch).or_insert_with(|| BuildView {
                os_arch,
                target_shell: shell_quote(&build.target),
                url_shell: shell_quote(&build.url),
                sha256: shell_quote(&build.sha256),
            });
        }

        let params = TemplateParams {
            manifest_base64: base64::engine::general_purpose::STANDARD
                .encode(input.manifest_json.as_bytes()),
            manifest_path_shell: shell_quote(&input.manifest_path.display().to_string()),
            builds: builds.into_values().collect(),
        };
        minijinja::Environment::new()
            .render_str(BOOTSTRAP_TEMPLATE, &params)
            .map_err(|err| {
                InstallError::Parse(vec![FieldError::new(
                    "/",
                    format!("cannot render the bootstrap script: {err}"),
                )])
            })
    }
}

#[derive(Debug, Serialize)]
struct TemplateParams {
    manifest_base64: String,
    manifest_path_shell: String,
    builds: Vec<BuildView>,
}

#[derive(Debug, Serialize)]
struct BuildView {
    os_arch: &'static str,
    target_shell: String,
    url_shell: String,
    sha256: String,
}

/// The `uname -s -m` value of a Rust target triple.
fn uname_pattern(target: &str) -> Option<&'static str> {
    match target {
        "x86_64-unknown-linux-gnu" | "x86_64-unknown-linux-musl" => Some("Linux/x86_64"),
        "aarch64-unknown-linux-gnu" | "aarch64-unknown-linux-musl" => Some("Linux/aarch64"),
        "i686-unknown-linux-gnu" | "i686-unknown-linux-musl" => Some("Linux/i686"),
        "armv7-unknown-linux-gnueabihf" | "armv7-unknown-linux-musleabihf" => Some("Linux/armv7l"),
        "x86_64-apple-darwin" => Some("Darwin/x86_64"),
        "aarch64-apple-darwin" => Some("Darwin/arm64"),
        _ => None,
    }
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
