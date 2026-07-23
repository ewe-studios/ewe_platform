//! Container configuration — builder for Docker container specs.
//!
//! **WHY:** The `#[docker_container]` proc macro and programmatic API both
//! need a declarative way to specify image, ports, env, volumes, networking,
//! and readiness conditions. A builder pattern keeps this composable.
//!
//! **WHAT:** `ContainerConfig` is the Rust-native equivalent of a
//! `compose.yaml` service entry. Built via chained methods, consumed by
//! `ContainerHandle::start_async()`.
//!
//! **HOW:** Standard Rust builder with `new(image)` + chained setters.
//! Internally maps to bollard's `ContainerCreateBody` and `HostConfig`.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::docker::wait_for::WaitFor;

/// Builder for a Docker container configuration.
#[derive(Debug, Clone)]
pub struct ContainerConfig {
    /// Docker image name (e.g. "redis:7", "postgres:16", "dockurr/windows:5.15").
    pub image: String,
    /// Optional container name. Auto-generated if absent.
    pub name: Option<String>,
    /// Port mappings: container_port -> optional host_port (None = auto-assign).
    pub ports: Vec<PortMapping>,
    /// Environment variables: KEY=VALUE pairs.
    pub env: Vec<(String, String)>,
    /// Volume mounts.
    pub volumes: Vec<VolumeMount>,
    /// Network to attach to (None = default bridge).
    pub network: Option<String>,
    /// Network aliases for DNS resolution within the network.
    pub network_aliases: Vec<String>,
    /// Wait strategy for readiness.
    pub wait: WaitFor,
    /// Stop timeout in seconds.
    pub stop_timeout: u64,
    /// Memory limit (e.g. "512m", "2g").
    pub memory: Option<String>,
    /// CPU count.
    pub cpus: Option<u32>,
    /// Container labels.
    pub labels: HashMap<String, String>,
    /// Extra hosts entries.
    pub extra_hosts: Vec<String>,
    /// Whether to always pull the image (vs. use cached).
    pub always_pull: bool,
    /// Command to run (overrides image CMD).
    pub command: Option<Vec<String>>,
    /// Devices to pass through (for KVM, TUN, etc.).
    pub devices: Vec<DeviceMapping>,
    /// Linux capabilities to add.
    pub cap_add: Vec<String>,
    /// Whether this container is required (failure = panic, no graceful skip).
    pub required: bool,
}

/// A port mapping from container to host.
#[derive(Debug, Clone)]
pub struct PortMapping {
    pub container_port: u16,
    pub host_port: Option<u16>, // None = auto-assign
    pub protocol: PortProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortProtocol {
    Tcp,
    Udp,
}

/// A volume mount from source to container target.
#[derive(Debug, Clone)]
pub struct VolumeMount {
    pub source: VolumeSource,
    pub target: PathBuf,
    pub read_only: bool,
}

/// Volume source — either a host path bind mount or a named Docker volume.
#[derive(Debug, Clone)]
pub enum VolumeSource {
    Bind(PathBuf),
    Named(String),
}

/// A device to pass through to the container.
#[derive(Debug, Clone)]
pub struct DeviceMapping {
    pub host_path: PathBuf,
    pub container_path: Option<PathBuf>,
}

impl ContainerConfig {
    /// Create a new config with the given Docker image.
    #[must_use]
    pub fn new(image: impl Into<String>) -> Self {
        Self {
            image: image.into(),
            name: None,
            ports: Vec::new(),
            env: Vec::new(),
            volumes: Vec::new(),
            network: None,
            network_aliases: Vec::new(),
            wait: WaitFor::None,
            stop_timeout: 10,
            memory: None,
            cpus: None,
            labels: HashMap::new(),
            extra_hosts: Vec::new(),
            always_pull: false,
            command: None,
            devices: Vec::new(),
            cap_add: Vec::new(),
            required: false,
        }
    }

    /// Set the container name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Expose a TCP container port with an auto-assigned host port.
    #[must_use]
    pub fn port(mut self, container_port: u16) -> Self {
        self.ports.push(PortMapping {
            container_port,
            host_port: None,
            protocol: PortProtocol::Tcp,
        });
        self
    }

    /// Map a TCP container port to an explicit host port.
    #[must_use]
    pub fn port_mapped(mut self, container_port: u16, host_port: u16) -> Self {
        self.ports.push(PortMapping {
            container_port,
            host_port: Some(host_port),
            protocol: PortProtocol::Tcp,
        });
        self
    }

    /// Expose a UDP container port with an auto-assigned host port.
    #[must_use]
    pub fn port_udp(mut self, container_port: u16) -> Self {
        self.ports.push(PortMapping {
            container_port,
            host_port: None,
            protocol: PortProtocol::Udp,
        });
        self
    }

    /// Map a UDP container port to an explicit host port.
    #[must_use]
    pub fn port_udp_mapped(mut self, container_port: u16, host_port: u16) -> Self {
        self.ports.push(PortMapping {
            container_port,
            host_port: Some(host_port),
            protocol: PortProtocol::Udp,
        });
        self
    }

    /// Set an environment variable.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Attach to a Docker network.
    #[must_use]
    pub fn network(mut self, name: impl Into<String>) -> Self {
        self.network = Some(name.into());
        self
    }

    /// Add a network alias for DNS resolution.
    #[must_use]
    pub fn network_alias(mut self, alias: impl Into<String>) -> Self {
        self.network_aliases.push(alias.into());
        self
    }

    /// Bind-mount a host path into the container.
    #[must_use]
    pub fn volume(mut self, host_path: impl Into<PathBuf>, container_path: impl Into<PathBuf>) -> Self {
        self.volumes.push(VolumeMount {
            source: VolumeSource::Bind(host_path.into()),
            target: container_path.into(),
            read_only: false,
        });
        self
    }

    /// Mount a named volume into the container.
    #[must_use]
    /// Bind-mount a host path read-only — the container can read it but any
    /// write is refused by the kernel.
    
    pub fn volume_read_only(
        mut self,
        host_path: impl Into<PathBuf>,
        container_path: impl Into<PathBuf>,
    ) -> Self {
        self.volumes.push(VolumeMount {
            source: VolumeSource::Bind(host_path.into()),
            target: container_path.into(),
            read_only: true,
        });
        self
    }

    #[must_use]
    pub fn named_volume(mut self, volume_name: impl Into<String>, container_path: impl Into<PathBuf>) -> Self {
        self.volumes.push(VolumeMount {
            source: VolumeSource::Named(volume_name.into()),
            target: container_path.into(),
            read_only: false,
        });
        self
    }

    /// Set the wait strategy.
    #[must_use]
    pub fn wait(mut self, strategy: WaitFor) -> Self {
        self.wait = strategy;
        self
    }

    /// Set the stop timeout in seconds.
    #[must_use]
    pub fn stop_timeout_secs(mut self, secs: u64) -> Self {
        self.stop_timeout = secs;
        self
    }

    /// Set a memory limit, as a plain byte count or with a `k`/`m`/`g`/`t`
    /// suffix (`"256m"`, `"1g"`) — the same forms `docker run --memory` takes.
    #[must_use]
    pub fn memory(mut self, mem: impl Into<String>) -> Self {
        self.memory = Some(mem.into());
        self
    }

    /// Set a CPU limit.
    #[must_use]
    pub fn cpus(mut self, count: u32) -> Self {
        self.cpus = Some(count);
        self
    }

    /// Force pull the image on every start.
    #[must_use]
    pub fn always_pull(mut self) -> Self {
        self.always_pull = true;
        self
    }

    /// Set a command override.
    #[must_use]
    pub fn command(mut self, cmd: Vec<String>) -> Self {
        self.command = Some(cmd);
        self
    }

    /// Pass through a host device.
    #[must_use]
    pub fn device(mut self, host_path: impl Into<PathBuf>) -> Self {
        self.devices.push(DeviceMapping {
            host_path: host_path.into(),
            container_path: None,
        });
        self
    }

    /// Add a Linux capability.
    #[must_use]
    pub fn cap_add(mut self, cap: impl Into<String>) -> Self {
        self.cap_add.push(cap.into());
        self
    }

    /// Mark this container as required (panic on start failure, no graceful skip).
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Add a container label.
    #[must_use]
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }
}

/// Parses a `docker run --memory`-style size into bytes: a plain byte count, or
/// a decimal with a `b`/`k`/`m`/`g`/`t` suffix (case-insensitive; `kb`/`mb`/…
/// accepted too). Multipliers are binary (1k = 1024), matching Docker.
///
/// Returns `Err` with the offending input rather than a fallback, so a
/// mistyped limit surfaces instead of silently running unlimited.
pub fn parse_memory_bytes(input: &str) -> Result<i64, String> {
    let text = input.trim().to_ascii_lowercase();
    if text.is_empty() {
        return Err("memory limit is empty".to_string());
    }

    let digits_end = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    let (number, suffix) = text.split_at(digits_end);

    let value: i64 = number
        .parse()
        .map_err(|_| format!("invalid memory limit {input:?}: expected a byte count like \"256m\""))?;

    let multiplier: i64 = match suffix.trim_end_matches('b') {
        "" => 1,
        "k" => 1024,
        "m" => 1024 * 1024,
        "g" => 1024 * 1024 * 1024,
        "t" => 1024_i64 * 1024 * 1024 * 1024,
        _ => {
            return Err(format!(
                "invalid memory limit {input:?}: unknown unit {suffix:?} (use b, k, m, g, or t)"
            ))
        }
    };

    value
        .checked_mul(multiplier)
        .ok_or_else(|| format!("memory limit {input:?} overflows i64 bytes"))
}
