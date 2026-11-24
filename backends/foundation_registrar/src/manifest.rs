//! The part of the spec 78 manifest that the sidecar reads.
//!
//! WHY: the sidecar reads only `name`, `version`, `source.kind`,
//! `deploy.kind`, `proxy` and `probes`, and it must not share a Rust crate with
//! the installer (spec 82 feature 01, section 4.1). The manifest is untrusted
//! input, so this module validates it before the sidecar uses it.
//!
//! WHAT: [`ManifestView`] and its entry, deploy, proxy, route, probe and route
//! types. Validation reports [`RegistrarError::UnsupportedManifestVersion`] for
//! a version other than 1 and [`RegistrarError::Manifest`] with a JSON path for
//! any other fault.
//!
//! HOW: `serde_json` reads the value, a short pass checks the version, the
//! names, the source and deploy kinds and the probe kinds, and then serde
//! deserializes the typed view. Unknown fields are ignored.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RegistrarError;

/// The manifest version that this build reads (spec 78).
pub const SUPPORTED_MANIFEST_VERSION: u32 = 1;

/// The source kinds that the contract knows (spec 78, validation rule V6).
pub const SOURCE_KINDS: [&str; 6] = [
    "github-release",
    "url",
    "cargo",
    "apt",
    "mise",
    "docker-image",
];

/// The deploy kinds that the contract knows.
pub const DEPLOY_KINDS: [&str; 3] = ["systemd", "docker", "none"];

/// The part of the manifest that the sidecar reads. Unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestView {
    /// The contract version. This build reads version 1.
    pub manifest_version: u32,
    /// The entries, in the order of the file.
    pub entries: Vec<EntryView>,
}

/// One manifest entry, as the sidecar reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryView {
    /// The entry name. Same rule as the installer: `[a-z0-9-]{1,63}`.
    pub name: String,
    /// The software version. During an upgrade the old version runs for a
    /// short time, so discovery reads the running version, not this one.
    pub version: String,
    /// The source block. The sidecar reads only its kind.
    pub source: SourceView,
    /// How the app is deployed. This picks the discovery adapter.
    pub deploy: DeployView,
    /// `None`: the sidecar watches the app but does not register it.
    pub proxy: Option<ProxyBlock>,
    /// `None`: no probes. A running app counts as ready.
    pub probes: Option<ProbesBlock>,
}

/// The part of the source block that the sidecar reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceView {
    /// `github-release`, `url`, `cargo`, `apt`, `mise` or `docker-image`.
    pub kind: String,
}

/// The part of the deploy block that the sidecar reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeployView {
    /// `systemd`, `docker` or `none`.
    pub kind: String,
}

/// How the proxy routes to this app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyBlock {
    /// The proxy service name. Example: `agents`.
    pub service: String,
    /// Which client requests go to this service.
    pub route: RouteBlock,
    /// The port that the app listens on.
    pub port: u16,
    /// `http`, `https`, `connect`, `grpc` or `grpc-web`.
    pub protocol: String,
    /// Load-balancer weight. Default 1.
    #[serde(default)]
    pub weight: Option<u32>,
}

impl ProxyBlock {
    /// The wire protocol, or `None` when the manifest names an unknown one.
    #[must_use]
    pub fn protocol_enum(&self) -> Option<crate::proto::v1::Protocol> {
        match self.protocol.as_str() {
            "http" => Some(crate::proto::v1::Protocol::PROTOCOL_HTTP),
            "https" => Some(crate::proto::v1::Protocol::PROTOCOL_HTTPS),
            "connect" => Some(crate::proto::v1::Protocol::PROTOCOL_CONNECT),
            "grpc" => Some(crate::proto::v1::Protocol::PROTOCOL_GRPC),
            "grpc-web" => Some(crate::proto::v1::Protocol::PROTOCOL_GRPC_WEB),
            _ => None,
        }
    }

    /// The route path prefix. The contract default is `/`.
    #[must_use]
    pub fn path_prefix(&self) -> String {
        self.route
            .path_prefix
            .clone()
            .unwrap_or_else(|| "/".to_string())
    }
}

/// The route of a service: which client requests go to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RouteBlock {
    /// Exact host or `*.example.com`.
    pub host: String,
    /// Default `/`.
    #[serde(default)]
    pub path_prefix: Option<String>,
}

/// The three probe kinds. Each is optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbesBlock {
    /// The startup probe: liveness and readiness wait until it passes.
    pub startup: Option<ProbeSpec>,
    /// The liveness probe: asks whether the app is stuck.
    pub liveness: Option<ProbeSpec>,
    /// The readiness probe: asks whether the app can take traffic.
    pub readiness: Option<ProbeSpec>,
}

/// One probe, with Kubernetes field names and defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeSpec {
    /// What the probe does.
    #[serde(flatten)]
    pub check: ProbeCheck,
    /// Seconds between checks. Default 10 (Kubernetes `periodSeconds`).
    pub period_secs: Option<u32>,
    /// Seconds before one check fails. Default 1 (Kubernetes `timeoutSeconds`).
    pub timeout_secs: Option<u32>,
    /// Failures in a row that make the probe fail. Default 3.
    pub failure_threshold: Option<u32>,
    /// Successes in a row that make the probe pass. Default 1.
    pub success_threshold: Option<u32>,
    /// Seconds to wait after the app starts. Default 0.
    pub initial_delay_secs: Option<u32>,
}

/// What one probe does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProbeCheck {
    /// `GET http://<address>:<port><path>`. A 2xx or 3xx status passes.
    Http {
        /// The app port.
        port: u16,
        /// The request path.
        path: String,
    },
    /// A TCP connect to `<address>:<port>` passes.
    Tcp {
        /// The app port.
        port: u16,
    },
    /// `grpc.health.v1.Health/Check`; `SERVING` passes. An empty `service`
    /// asks about the whole server.
    Grpc {
        /// The app port.
        port: u16,
        /// The gRPC service name. Empty = the whole server.
        #[serde(default)]
        service: String,
    },
    /// Runs a command (host mode only). Exit code 0 passes.
    Exec {
        /// The argument list. It is never a shell string.
        command: Vec<String>,
    },
}

impl ProbeCheck {
    /// The app port that the check uses, when it uses one.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        match self {
            Self::Http { port, .. } | Self::Tcp { port } | Self::Grpc { port, .. } => Some(*port),
            Self::Exec { .. } => None,
        }
    }

    /// The probe kind, for a log line or a test.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Http { .. } => "http",
            Self::Tcp { .. } => "tcp",
            Self::Grpc { .. } => "grpc",
            Self::Exec { .. } => "exec",
        }
    }
}

impl ManifestView {
    /// Parse and validate a manifest from text.
    ///
    /// # Errors
    /// [`RegistrarError::UnsupportedManifestVersion`] for a version other than
    /// 1, and [`RegistrarError::Manifest`] with a JSON path for any other
    /// fault.
    pub fn parse(text: &str) -> Result<Self, RegistrarError> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
            RegistrarError::Manifest(format!("/: the manifest is not valid JSON: {error}"))
        })?;
        Self::from_value(value)
    }

    /// Read, parse and validate a manifest file. The file is limited to
    /// `max_bytes`.
    ///
    /// # Errors
    /// [`RegistrarError::Manifest`] when the file cannot be read, is larger
    /// than `max_bytes`, or is not a valid manifest.
    pub fn parse_file(path: &Path, max_bytes: u64) -> Result<Self, RegistrarError> {
        let metadata = std::fs::metadata(path).map_err(|error| {
            RegistrarError::Manifest(format!("cannot read {}: {error}", path.display()))
        })?;
        if metadata.len() > max_bytes {
            return Err(RegistrarError::Manifest(format!(
                "{} is {} bytes, over the limit of {max_bytes}",
                path.display(),
                metadata.len()
            )));
        }
        let text = std::fs::read_to_string(path).map_err(|error| {
            RegistrarError::Manifest(format!("cannot read {}: {error}", path.display()))
        })?;
        Self::parse(&text)
    }

    /// The entries that this sidecar watches, in file order.
    pub fn watched<'a>(
        &'a self,
        entries: &'a [String],
    ) -> impl Iterator<Item = &'a EntryView> + 'a {
        self.entries.iter().filter(move |entry| {
            entries.is_empty() || entries.iter().any(|name| name == &entry.name)
        })
    }

    /// The entry with `name`.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<&EntryView> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    fn from_value(value: serde_json::Value) -> Result<Self, RegistrarError> {
        let object = value
            .as_object()
            .ok_or_else(|| RegistrarError::Manifest("/: the manifest is not an object".into()))?;
        check_version(object)?;
        validate_shape(object)?;
        validate_names(object)?;
        validate_kinds(object)?;
        validate_probes(object)?;
        validate_proxy(object)?;

        serde_json::from_value(value).map_err(|error| {
            RegistrarError::Manifest(format!(
                "/: the manifest does not match the contract: {error}"
            ))
        })
    }
}

fn check_version(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    let found = object
        .get("manifest_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            RegistrarError::Manifest(
                "/manifest_version: expected a non-negative 32-bit integer".into(),
            )
        })?;
    if found == SUPPORTED_MANIFEST_VERSION {
        Ok(())
    } else {
        Err(RegistrarError::UnsupportedManifestVersion {
            found,
            supported: SUPPORTED_MANIFEST_VERSION,
        })
    }
}

fn entries_of(object: &serde_json::Map<String, serde_json::Value>) -> Vec<serde_json::Value> {
    object
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn validate_shape(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    let entries = object
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| RegistrarError::Manifest("/entries: expected an array".into()))?;
    for (index, entry) in entries.iter().enumerate() {
        let map = entry.as_object().ok_or_else(|| {
            RegistrarError::Manifest(format!("/entries/{index}: expected an object"))
        })?;
        for field in ["name", "version"] {
            if !map.get(field).is_some_and(serde_json::Value::is_string) {
                return Err(RegistrarError::Manifest(format!(
                    "/entries/{index}/{field}: expected a string"
                )));
            }
        }
        for field in ["source", "deploy"] {
            if !map.get(field).is_some_and(serde_json::Value::is_object) {
                return Err(RegistrarError::Manifest(format!(
                    "/entries/{index}/{field}: expected an object"
                )));
            }
        }
    }
    Ok(())
}

/// Validation rule V5: every name is valid and unique.
fn validate_names(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    let mut seen = std::collections::BTreeSet::new();
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(name) = entry.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let path = format!("/entries/{index}/name");
        if !valid_entry_name(name) {
            return Err(RegistrarError::Manifest(format!(
                "{path}: the name must match [a-z0-9-]{{1,63}} and not start or end with '-'"
            )));
        }
        if !seen.insert(name.to_string()) {
            return Err(RegistrarError::Manifest(format!(
                "{path}: the name repeats an earlier entry"
            )));
        }
    }
    Ok(())
}

/// Validation rule V6: every source and deploy kind is one that the contract
/// knows.
fn validate_kinds(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    for (index, entry) in entries_of(object).iter().enumerate() {
        for (block, known) in [
            ("source", SOURCE_KINDS.as_slice()),
            ("deploy", DEPLOY_KINDS.as_slice()),
        ] {
            let Some(map) = entry.get(block).and_then(serde_json::Value::as_object) else {
                continue;
            };
            let path = format!("/entries/{index}/{block}/kind");
            match map.get("kind") {
                None => {
                    return Err(RegistrarError::Manifest(format!(
                        "{path}: required field is missing"
                    )))
                }
                Some(serde_json::Value::String(kind)) if known.contains(&kind.as_str()) => {}
                Some(serde_json::Value::String(kind)) => {
                    return Err(RegistrarError::Manifest(format!(
                        "{path}: unknown kind {kind:?}"
                    )))
                }
                Some(_) => {
                    return Err(RegistrarError::Manifest(format!(
                        "{path}: expected a string"
                    )))
                }
            }
        }
    }
    Ok(())
}

/// A bad probe kind fails with the entry named, so an operator can find it.
fn validate_probes(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    const KINDS: [&str; 4] = ["http", "tcp", "grpc", "exec"];
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(name) = entry.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let Some(probes) = entry.get("probes").and_then(serde_json::Value::as_object) else {
            continue;
        };
        for phase in ["startup", "liveness", "readiness"] {
            let Some(probe) = probes.get(phase).and_then(serde_json::Value::as_object) else {
                continue;
            };
            let Some(kind) = probe.get("kind").and_then(serde_json::Value::as_str) else {
                return Err(RegistrarError::Manifest(format!(
                    "entry {name}: probes/{phase}/kind is missing"
                )));
            };
            if !KINDS.contains(&kind) {
                return Err(RegistrarError::Manifest(format!(
                    "entry {name}: probes/{phase}/kind {kind:?} is not one of http, tcp, grpc, exec"
                )));
            }
        }
        // Keep the index used, so a later rule can name the exact path.
        let _ = index;
    }
    Ok(())
}

/// The protocol must be a wire protocol that the proxy speaks.
fn validate_proxy(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), RegistrarError> {
    for (index, entry) in entries_of(object).iter().enumerate() {
        let Some(proxy) = entry.get("proxy").and_then(serde_json::Value::as_object) else {
            continue;
        };
        let unknown = proxy.get("protocol").and_then(serde_json::Value::as_str);
        if let Some(protocol) = unknown {
            if crate::manifest::protocol_known(protocol).is_none() {
                return Err(RegistrarError::Manifest(format!(
                    "/entries/{index}/proxy/protocol: unknown protocol {protocol:?}"
                )));
            }
        }
    }
    Ok(())
}

fn protocol_known(protocol: &str) -> Option<()> {
    match protocol {
        "http" | "https" | "connect" | "grpc" | "grpc-web" => Some(()),
        _ => None,
    }
}

fn valid_entry_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && !name.starts_with('-')
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}
