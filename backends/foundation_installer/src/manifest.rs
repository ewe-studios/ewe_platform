//! The manifest: the spec 78 JSON contract as this crate reads it.
//!
//! The installer and the registrar parse the manifest independently, each with
//! its own Rust types. The JSON format is the only contract. This module owns
//! the installer's copy of the types and every rule of spec 78 section 6.
//!
//! `proxy` and `probes` stay `serde_json::Value` on purpose: the installer does
//! not own their meaning, so it must not reject a change that only the
//! registrar understands.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use crate::error::{FieldError, InstallError};

/// The manifest version that this build reads.
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// The source kind names of the contract.
pub const SOURCE_KINDS: &[&str] = &[
    "github-release",
    "url",
    "cargo",
    "apt",
    "mise",
    "docker-image",
];

/// The deploy kind names of the contract.
pub const DEPLOY_KINDS: &[&str] = &["systemd", "docker", "none"];

/// The number of hexadecimal characters in a SHA-256 value.
const SHA256_HEX_CHARS: usize = 64;

/// The longest program name that the installer accepts.
const MAX_PROGRAM_CHARS: usize = 128;

/// The parsed manifest. Made by [`Manifest::parse`]; read by
/// [`Installer::run`](crate::installer::Installer::run).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Manifest {
    /// The format version. This build reads version 1.
    pub manifest_version: u32,
    /// The entries, in install order.
    pub entries: Vec<Entry>,
}

/// One piece of software to install.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Entry {
    /// The checked entry name.
    pub name: EntryName,
    /// The publisher's version text.
    pub version: String,
    /// Where the software comes from.
    pub source: SourceSpec,
    /// How the software runs.
    pub deploy: DeploySpec,
    /// Kept, not read. The registrar (spec 82 feature 01) owns its meaning.
    #[serde(default)]
    pub proxy: Option<Value>,
    /// Kept, not read. The registrar (spec 82 feature 01) owns its meaning.
    #[serde(default)]
    pub probes: Option<Value>,
}

/// A checked entry name: `[a-z0-9-]{1,63}`, no leading or trailing `-`.
///
/// It is safe to put in a unit file name, a container name and a path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryName(String);

impl EntryName {
    /// Check a name and make an `EntryName`.
    ///
    /// # Errors
    ///
    /// Returns a [`FieldError`] at `path` when the name breaks rule R4.
    pub fn parse_at(name: &str, path: &str) -> Result<Self, FieldError> {
        if name.is_empty() {
            return Err(FieldError::new(path, "the name is empty"));
        }
        if name.len() > 63 {
            return Err(FieldError::new(
                path,
                "the name is longer than 63 characters",
            ));
        }
        if name.starts_with('-') || name.ends_with('-') {
            return Err(FieldError::new(path, "the name starts or ends with '-'"));
        }
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(FieldError::new(
                path,
                "the name may hold only lowercase letters, digits and '-'",
            ));
        }
        Ok(Self(name.to_string()))
    }

    /// The name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for EntryName {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for EntryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for EntryName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse_at(&raw, "/entries/name").map_err(D::Error::custom)
    }
}

/// Where the software of one entry comes from. JSON: `{"kind": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SourceSpec {
    /// A file attached to a GitHub release.
    GithubRelease {
        /// `owner/repo`.
        repo: String,
        /// The release tag.
        tag: String,
        /// One asset for each target triple, with its checksum. The installer
        /// picks the asset for the target it was built for.
        assets: BTreeMap<String, AssetSpec>,
    },
    /// A file at a plain HTTPS URL.
    Url {
        /// The HTTPS URL.
        url: String,
        /// The lowercase hex SHA-256 of the file.
        sha256: String,
    },
    /// `cargo install <crate> --version <version> --locked`.
    Cargo {
        /// The crate name.
        #[serde(rename = "crate")]
        krate: String,
        /// The program name that the crate installs. Default: the crate name.
        program: Option<String>,
    },
    /// `apt-get install <package>=<version>`.
    Apt {
        /// The Debian package name.
        package: String,
        /// The program name that the package installs. Default: the package.
        program: Option<String>,
    },
    /// `mise install <tool>@<version>`.
    Mise {
        /// The mise tool name.
        tool: String,
        /// The program name that the tool installs. Default: the tool name.
        program: Option<String>,
    },
    /// A container image, pulled through `foundation_deployment_docker`.
    DockerImage {
        /// The image repository.
        repository: String,
        /// The image tag.
        tag: String,
        /// The optional image digest. The installer checks it after the pull.
        digest: Option<String>,
    },
}

impl SourceSpec {
    /// The `kind` value of this source.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::GithubRelease { .. } => "github-release",
            Self::Url { .. } => "url",
            Self::Cargo { .. } => "cargo",
            Self::Apt { .. } => "apt",
            Self::Mise { .. } => "mise",
            Self::DockerImage { .. } => "docker-image",
        }
    }

    /// The class of artifact that this source gives: `file`, `tool` or
    /// `image`.
    #[must_use]
    pub fn artifact_class(&self) -> &'static str {
        match self {
            Self::GithubRelease { .. } | Self::Url { .. } => "file",
            Self::Cargo { .. } | Self::Apt { .. } | Self::Mise { .. } => "tool",
            Self::DockerImage { .. } => "image",
        }
    }
}

/// One file attached to a release, with its checksum.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AssetSpec {
    /// The asset file name in the release.
    pub name: String,
    /// The lowercase hex SHA-256 of the asset.
    pub sha256: String,
}

/// How one entry runs on the host. JSON: `{"kind": "...", ...}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DeploySpec {
    /// A systemd service. The installer writes
    /// `<systemd_unit_dir>/ewe-<name>.service`.
    Systemd {
        /// Arguments for the program.
        #[serde(default)]
        args: Vec<String>,
        /// Environment values for the unit.
        #[serde(default)]
        env: BTreeMap<String, String>,
        /// The OS user that runs the program. Default `ewe`.
        user: Option<String>,
        /// The systemd `Restart=` value. Default `on-failure`.
        restart: Option<SystemdRestart>,
    },
    /// A container. The container name is the entry name.
    Docker {
        /// Published ports.
        #[serde(default)]
        ports: Vec<PortSpec>,
        /// Environment values for the container.
        #[serde(default)]
        env: BTreeMap<String, String>,
        /// Host and container paths to bind.
        #[serde(default)]
        volumes: Vec<VolumeSpec>,
        /// The Docker restart policy. Default `unless-stopped`.
        restart: Option<DockerRestart>,
        /// Overrides the image command.
        command: Option<Vec<String>>,
    },
    /// Install only. Nothing runs.
    None,
}

impl DeploySpec {
    /// The `kind` value of this deploy target.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Systemd { .. } => "systemd",
            Self::Docker { .. } => "docker",
            Self::None => "none",
        }
    }

    /// True when this target can run an artifact of `class`.
    #[must_use]
    pub fn accepts(&self, class: &str) -> bool {
        match self {
            Self::Systemd { .. } => matches!(class, "file" | "tool"),
            Self::Docker { .. } => class == "image",
            Self::None => matches!(class, "file" | "tool" | "image"),
        }
    }
}

/// The systemd `Restart=` values of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SystemdRestart {
    /// Do not restart.
    No,
    /// Restart only after a clean exit.
    OnSuccess,
    /// Restart only after a failure. The default.
    OnFailure,
    /// Always restart.
    Always,
}

impl SystemdRestart {
    /// The text that systemd expects.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::OnSuccess => "on-success",
            Self::OnFailure => "on-failure",
            Self::Always => "always",
        }
    }
}

/// The Docker restart policies of the contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DockerRestart {
    /// Do not restart.
    No,
    /// Always restart.
    Always,
    /// Restart after a failure.
    OnFailure,
    /// Restart unless the operator stopped the container. The default.
    UnlessStopped,
}

impl DockerRestart {
    /// The text that Docker expects.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Always => "always",
            Self::OnFailure => "on-failure",
            Self::UnlessStopped => "unless-stopped",
        }
    }
}

/// One published container port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct PortSpec {
    /// The host port.
    pub host: u16,
    /// The container port.
    pub container: u16,
    /// The transport protocol.
    pub protocol: PortProtocol,
}

/// The transport protocol of a published port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortProtocol {
    /// Transmission Control Protocol.
    Tcp,
    /// User Datagram Protocol.
    Udp,
}

impl PortProtocol {
    /// The text that Docker expects.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

/// One host path bound into a container.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VolumeSpec {
    /// The absolute host path.
    pub host: PathBuf,
    /// The absolute container path.
    pub container: PathBuf,
}

impl Manifest {
    /// Parse and validate a manifest from text.
    ///
    /// Validation follows spec 78 section 6: the version first, then the
    /// entries, names, kinds, source fields, deploy fields and compatibility.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::UnsupportedManifestVersion`] for a version other
    /// than 1, [`InstallError::UnknownKind`] for an unknown `kind`, and
    /// [`InstallError::Parse`] with the JSON path of any field fault.
    pub fn parse(text: &str) -> Result<Self, InstallError> {
        let value: Value = serde_json::from_str(text).map_err(|err| {
            InstallError::Parse(vec![FieldError::new(
                "/",
                format!("the manifest is not valid JSON: {err}"),
            )])
        })?;
        Self::from_value(value)
    }

    /// Read, parse and validate a manifest file.
    ///
    /// The file is small, so it is read whole, but never more than
    /// `max_bytes` (rule 4 allows a small file whose contents are needed).
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::ManifestIo`] when the file cannot be read,
    /// [`InstallError::ManifestTooLarge`] when it is over the limit, and the
    /// errors of [`Manifest::parse`].
    pub fn parse_file(path: &std::path::Path, max_bytes: u64) -> Result<Self, InstallError> {
        let metadata = std::fs::metadata(path).map_err(|err| InstallError::ManifestIo {
            path: path.display().to_string(),
            reason: err.to_string(),
        })?;
        if metadata.len() > max_bytes {
            return Err(InstallError::ManifestTooLarge {
                size: metadata.len(),
                max: max_bytes,
            });
        }
        let text = std::fs::read_to_string(path).map_err(|err| InstallError::ManifestIo {
            path: path.display().to_string(),
            reason: err.to_string(),
        })?;
        Self::parse(&text)
    }

    fn from_value(value: Value) -> Result<Self, InstallError> {
        let object = value
            .as_object()
            .ok_or_else(|| parse_fault("/", "the manifest is not a JSON object"))?;
        check_version(object)?;
        validate_shape(object)?;
        validate_names(object)?;
        validate_kinds(object)?;
        validate_source_fields(object)?;
        validate_deploy_fields(object)?;
        validate_compatibility(object)?;

        serde_json::from_value(value).map_err(|err| {
            parse_fault(
                "/",
                format!("the manifest does not match the contract: {err}"),
            )
        })
    }
}

fn parse_fault(path: impl Into<String>, message: impl Into<String>) -> InstallError {
    InstallError::Parse(vec![FieldError::new(path, message)])
}

fn check_version(object: &Map<String, Value>) -> Result<(), InstallError> {
    match object.get("manifest_version") {
        None => Err(parse_fault(
            "/manifest_version",
            "required field is missing",
        )),
        Some(Value::Number(number)) => {
            let found = number
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| {
                    parse_fault(
                        "/manifest_version",
                        "expected a non-negative 32-bit integer",
                    )
                })?;
            if found == SUPPORTED_MANIFEST_VERSION {
                Ok(())
            } else {
                Err(InstallError::UnsupportedManifestVersion {
                    found,
                    supported: SUPPORTED_MANIFEST_VERSION,
                })
            }
        }
        Some(_) => Err(parse_fault("/manifest_version", "expected an integer")),
    }
}

fn entries_of(object: &Map<String, Value>) -> &[Value] {
    object
        .get("entries")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn entry_object(entry: &Value) -> Option<&Map<String, Value>> {
    entry.as_object()
}

fn entry_name(entry: &Value) -> &str {
    entry
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn kind_of<'a>(entry: &'a Value, block: &str) -> &'a str {
    entry
        .get(block)
        .and_then(Value::as_object)
        .and_then(|map| map.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// Validation rule V4: the entries array and the required fields exist and
/// have the right broad type.
fn validate_shape(object: &Map<String, Value>) -> Result<(), InstallError> {
    let Some(entries) = object.get("entries") else {
        return Err(parse_fault("/entries", "required field is missing"));
    };
    let Some(entries) = entries.as_array() else {
        return Err(parse_fault("/entries", "expected an array"));
    };

    let mut faults = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let base = format!("/entries/{index}");
        let Some(map) = entry_object(entry) else {
            faults.push(FieldError::new(base, "expected an object"));
            continue;
        };
        for field in ["name", "version"] {
            match map.get(field) {
                None => faults.push(FieldError::new(
                    format!("{base}/{field}"),
                    "required field is missing",
                )),
                Some(Value::String(_)) => {}
                Some(_) => faults.push(FieldError::new(
                    format!("{base}/{field}"),
                    "expected a string",
                )),
            }
        }
        for field in ["source", "deploy"] {
            match map.get(field) {
                None => faults.push(FieldError::new(
                    format!("{base}/{field}"),
                    "required field is missing",
                )),
                Some(Value::Object(_)) => {}
                Some(_) => faults.push(FieldError::new(
                    format!("{base}/{field}"),
                    "expected an object",
                )),
            }
        }
    }

    if faults.is_empty() {
        Ok(())
    } else {
        Err(InstallError::Parse(faults))
    }
}

/// Validation rule V5: every name is valid and unique.
fn validate_names(object: &Map<String, Value>) -> Result<(), InstallError> {
    let mut faults = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(name) = entry.get("name").and_then(Value::as_str) else {
            continue;
        };
        let path = format!("/entries/{index}/name");
        match EntryName::parse_at(name, &path) {
            Err(fault) => faults.push(fault),
            Ok(_) if !seen.insert(name.to_string()) => {
                faults.push(FieldError::new(path, "the name repeats an earlier entry"));
            }
            Ok(_) => {}
        }
    }
    if faults.is_empty() {
        Ok(())
    } else {
        Err(InstallError::Parse(faults))
    }
}

/// Validation rule V6: every `kind` is one that the contract knows.
fn validate_kinds(object: &Map<String, Value>) -> Result<(), InstallError> {
    for (index, entry) in entries_of(object).iter().enumerate() {
        for (block, known) in [("source", SOURCE_KINDS), ("deploy", DEPLOY_KINDS)] {
            let Some(map) = entry.get(block).and_then(Value::as_object) else {
                continue;
            };
            let path = format!("/entries/{index}/{block}/kind");
            match map.get("kind") {
                None => return Err(parse_fault(path, "required field is missing")),
                Some(Value::String(kind)) if known.contains(&kind.as_str()) => {}
                Some(Value::String(kind)) => {
                    return Err(InstallError::UnknownKind {
                        path,
                        kind: kind.clone(),
                    });
                }
                Some(_) => return Err(parse_fault(path, "expected a string")),
            }
        }
    }
    Ok(())
}

/// Validation rule V7: the source fields follow spec 78 section 5.4.
fn validate_source_fields(object: &Map<String, Value>) -> Result<(), InstallError> {
    let mut faults = Vec::new();
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(source) = entry.get("source").and_then(Value::as_object) else {
            continue;
        };
        let base = format!("/entries/{index}/source");
        if let Err(fault) = validate_one_source(source, &base) {
            faults.push(fault);
        }
    }
    if faults.is_empty() {
        Ok(())
    } else {
        Err(InstallError::Parse(faults))
    }
}

fn validate_one_source(source: &Map<String, Value>, base: &str) -> Result<(), FieldError> {
    let kind = source
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "github-release" => {
            let repo = required_string(source, "repo", base)?;
            if !valid_repo(&repo) {
                return Err(FieldError::new(
                    format!("{base}/repo"),
                    "expected `owner/repo`",
                ));
            }
            required_non_empty(source, "tag", base)?;
            let assets = source
                .get("assets")
                .and_then(Value::as_object)
                .ok_or_else(|| FieldError::new(format!("{base}/assets"), "expected an object"))?;
            if assets.is_empty() {
                return Err(FieldError::new(
                    format!("{base}/assets"),
                    "expected at least one target triple",
                ));
            }
            for (target, asset) in assets {
                if target.is_empty() {
                    return Err(FieldError::new(
                        format!("{base}/assets"),
                        "the target triple is empty",
                    ));
                }
                let target_base = format!("{base}/assets/{target}");
                let asset = asset
                    .as_object()
                    .ok_or_else(|| FieldError::new(target_base.clone(), "expected an object"))?;
                required_non_empty(asset, "name", &target_base)?;
                let sha = required_string(asset, "sha256", &target_base)?;
                check_sha256(&sha, &format!("{target_base}/sha256"))?;
            }
            Ok(())
        }
        "url" => {
            let url = required_string(source, "url", base)?;
            if !valid_https_url(&url) {
                return Err(FieldError::new(
                    format!("{base}/url"),
                    "expected an https URL",
                ));
            }
            let sha = required_string(source, "sha256", base)?;
            check_sha256(&sha, &format!("{base}/sha256"))
        }
        "cargo" => {
            required_non_empty(source, "crate", base)?;
            check_optional_program(source, base)
        }
        "apt" => {
            required_non_empty(source, "package", base)?;
            check_optional_program(source, base)
        }
        "mise" => {
            required_non_empty(source, "tool", base)?;
            check_optional_program(source, base)
        }
        "docker-image" => {
            required_non_empty(source, "repository", base)?;
            required_non_empty(source, "tag", base)?;
            if let Some(digest) = optional_string(source, "digest", base)? {
                check_digest(&digest, &format!("{base}/digest"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Validation rule V8: the deploy fields follow spec 78 section 5.5.
fn validate_deploy_fields(object: &Map<String, Value>) -> Result<(), InstallError> {
    let mut faults = Vec::new();
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(deploy) = entry.get("deploy").and_then(Value::as_object) else {
            continue;
        };
        let base = format!("/entries/{index}/deploy");
        if let Err(fault) = validate_one_deploy(deploy, &base) {
            faults.push(fault);
        }
    }
    if faults.is_empty() {
        Ok(())
    } else {
        Err(InstallError::Parse(faults))
    }
}

fn validate_one_deploy(deploy: &Map<String, Value>, base: &str) -> Result<(), FieldError> {
    let kind = deploy
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "systemd" => {
            check_string_array(deploy, "args", base)?;
            check_string_map(deploy, "env", base)?;
            if let Some(user) = optional_string(deploy, "user", base)? {
                if user.is_empty() {
                    return Err(FieldError::new(format!("{base}/user"), "the user is empty"));
                }
            }
            if let Some(restart) = optional_string(deploy, "restart", base)? {
                if !matches!(
                    restart.as_str(),
                    "no" | "on-success" | "on-failure" | "always"
                ) {
                    return Err(FieldError::new(
                        format!("{base}/restart"),
                        "expected no, on-success, on-failure or always",
                    ));
                }
            }
            Ok(())
        }
        "docker" => {
            check_ports(deploy, base)?;
            check_string_map(deploy, "env", base)?;
            check_volumes(deploy, base)?;
            if let Some(restart) = optional_string(deploy, "restart", base)? {
                if !matches!(
                    restart.as_str(),
                    "no" | "always" | "on-failure" | "unless-stopped"
                ) {
                    return Err(FieldError::new(
                        format!("{base}/restart"),
                        "expected no, always, on-failure or unless-stopped",
                    ));
                }
            }
            check_string_array(deploy, "command", base)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Validation rule V9: the source class and the deploy target agree.
fn validate_compatibility(object: &Map<String, Value>) -> Result<(), InstallError> {
    for entry in entries_of(object) {
        let source_kind = kind_of(entry, "source");
        let deploy_kind = kind_of(entry, "deploy");
        let class = match source_kind {
            "github-release" | "url" => "file",
            "cargo" | "apt" | "mise" => "tool",
            "docker-image" => "image",
            _ => continue,
        };
        let accepts = match deploy_kind {
            "systemd" => matches!(class, "file" | "tool"),
            "docker" => class == "image",
            "none" => true,
            _ => continue,
        };
        if !accepts {
            return Err(InstallError::Incompatible {
                entry: entry_name(entry).to_string(),
                deploy: deploy_kind.to_string(),
                artifact: class.to_string(),
            });
        }
    }
    Ok(())
}

fn required_string(map: &Map<String, Value>, key: &str, base: &str) -> Result<String, FieldError> {
    match map.get(key) {
        None => Err(FieldError::new(
            format!("{base}/{key}"),
            "required field is missing",
        )),
        Some(Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(FieldError::new(
            format!("{base}/{key}"),
            "expected a string",
        )),
    }
}

fn optional_string(
    map: &Map<String, Value>,
    key: &str,
    base: &str,
) -> Result<Option<String>, FieldError> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(FieldError::new(
            format!("{base}/{key}"),
            "expected a string",
        )),
    }
}

fn required_non_empty(
    map: &Map<String, Value>,
    key: &str,
    base: &str,
) -> Result<String, FieldError> {
    let value = required_string(map, key, base)?;
    if value.is_empty() {
        return Err(FieldError::new(
            format!("{base}/{key}"),
            "the value is empty",
        ));
    }
    Ok(value)
}

fn check_optional_program(map: &Map<String, Value>, base: &str) -> Result<(), FieldError> {
    if let Some(program) = optional_string(map, "program", base)? {
        if !valid_program(&program) {
            return Err(FieldError::new(
                format!("{base}/program"),
                "expected a program name of at most 128 characters without a path",
            ));
        }
    }
    Ok(())
}

fn check_string_array(map: &Map<String, Value>, key: &str, base: &str) -> Result<(), FieldError> {
    let Some(value) = map.get(key) else {
        return Ok(());
    };
    let Some(items) = value.as_array() else {
        return Err(FieldError::new(
            format!("{base}/{key}"),
            "expected an array",
        ));
    };
    if let Some(index) = items.iter().position(|item| !item.is_string()) {
        return Err(FieldError::new(
            format!("{base}/{key}/{index}"),
            "expected a string",
        ));
    }
    Ok(())
}

fn check_string_map(map: &Map<String, Value>, key: &str, base: &str) -> Result<(), FieldError> {
    let Some(value) = map.get(key) else {
        return Ok(());
    };
    let Some(entries) = value.as_object() else {
        return Err(FieldError::new(
            format!("{base}/{key}"),
            "expected an object",
        ));
    };
    if let Some((name, _)) = entries.iter().find(|(_, item)| !item.is_string()) {
        return Err(FieldError::new(
            format!("{base}/{key}/{name}"),
            "expected a string",
        ));
    }
    Ok(())
}

fn check_ports(deploy: &Map<String, Value>, base: &str) -> Result<(), FieldError> {
    let Some(value) = deploy.get("ports") else {
        return Ok(());
    };
    let Some(ports) = value.as_array() else {
        return Err(FieldError::new(
            format!("{base}/ports"),
            "expected an array",
        ));
    };
    for (index, port) in ports.iter().enumerate() {
        let path = format!("{base}/ports/{index}");
        let port = port
            .as_object()
            .ok_or_else(|| FieldError::new(path.clone(), "expected an object"))?;
        for field in ["host", "container"] {
            match port.get(field).and_then(Value::as_u64) {
                Some(value) if (1..=65535).contains(&value) => {}
                Some(_) => {
                    return Err(FieldError::new(
                        format!("{path}/{field}"),
                        "expected a port from 1 to 65535",
                    ));
                }
                None => {
                    return Err(FieldError::new(
                        format!("{path}/{field}"),
                        "expected a port number",
                    ));
                }
            }
        }
        match port.get("protocol").and_then(Value::as_str) {
            Some("tcp" | "udp") => {}
            _ => {
                return Err(FieldError::new(
                    format!("{path}/protocol"),
                    "expected tcp or udp",
                ));
            }
        }
    }
    Ok(())
}

fn check_volumes(deploy: &Map<String, Value>, base: &str) -> Result<(), FieldError> {
    let Some(value) = deploy.get("volumes") else {
        return Ok(());
    };
    let Some(volumes) = value.as_array() else {
        return Err(FieldError::new(
            format!("{base}/volumes"),
            "expected an array",
        ));
    };
    for (index, volume) in volumes.iter().enumerate() {
        let path = format!("{base}/volumes/{index}");
        let volume = volume
            .as_object()
            .ok_or_else(|| FieldError::new(path.clone(), "expected an object"))?;
        for field in ["host", "container"] {
            match volume.get(field).and_then(Value::as_str) {
                Some(value) if value.starts_with('/') => {}
                _ => {
                    return Err(FieldError::new(
                        format!("{path}/{field}"),
                        "expected an absolute path",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn check_sha256(value: &str, path: &str) -> Result<(), FieldError> {
    if value.len() == SHA256_HEX_CHARS
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(FieldError::new(
            path,
            "expected 64 lowercase hexadecimal characters",
        ))
    }
}

fn check_digest(value: &str, path: &str) -> Result<(), FieldError> {
    let hex = value.strip_prefix("sha256:").unwrap_or(value);
    check_sha256(hex, path)
}

fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty()
    ) && !repo.chars().any(char::is_whitespace)
}

fn valid_https_url(url: &str) -> bool {
    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
        && !url.chars().any(char::is_whitespace)
}

fn valid_program(program: &str) -> bool {
    !program.is_empty()
        && program.len() <= MAX_PROGRAM_CHARS
        && !program.contains('/')
        && !program.contains('\0')
}

/// Format bytes as lowercase hexadecimal.
#[must_use]
pub fn hex_lower(bytes: &[u8]) -> String {
    use core::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}
