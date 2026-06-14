// Public API types — preserved signatures for compatibility with ewe_devserver consumers.
// Uses foundation_http/foundation_netio types internally.

use std::time;

use derive_more::{Debug, From};

// -- Proxy configuration

#[derive(Debug, Default, Clone, From)]
pub struct ProxyRemoteConfig {
    pub addr: String,
    pub port: usize,
}

impl ProxyRemoteConfig {
    #[must_use]
    pub fn new(addr: String, port: usize) -> Self {
        Self { addr, port }
    }
}

impl core::fmt::Display for ProxyRemoteConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.addr, self.port)
    }
}

// -- Tunnel

#[derive(Debug, Clone, From)]
pub struct Tunnel {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

impl core::fmt::Display for Tunnel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Tunnel {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self { source, destination }
    }
}

// -- Proxy transport protocols

#[derive(Debug, Clone)]
pub enum ProxyType {
    Tunnel(Tunnel),
    Http1(Http1),
    Http2(Http2),
    Http3(Http3),
}

impl core::fmt::Display for ProxyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl ProxyType {
    pub fn and_routes(&mut self, _mutator: impl Fn(&mut std::collections::HashMap<String, String>)) {
        // Routes are handled by foundation_http HttpApp + Serve handlers.
        // This method exists for API compat with old ewe_devserver consumers.
    }
}

// -- Http1

#[derive(Debug, Clone, From)]
pub struct Http1 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

impl Http1 {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self { source, destination }
    }
}

impl core::fmt::Display for Http1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Http1(source: {:?}, destination: {:?})", self.source, self.destination)
    }
}

// -- Http2 (stub — not implemented)

#[derive(Debug, Clone, From)]
pub struct Http2 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

impl Http2 {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self { source, destination }
    }
}

impl core::fmt::Display for Http2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Http2(source: {:?}, destination: {:?})", self.source, self.destination)
    }
}

// -- Http3 (stub — not implemented)

#[derive(Debug, Clone, From)]
pub struct Http3 {
    pub source: ProxyRemoteConfig,
    pub destination: ProxyRemoteConfig,
}

impl Http3 {
    #[must_use]
    pub fn new(source: ProxyRemoteConfig, destination: ProxyRemoteConfig) -> Self {
        Self { source, destination }
    }
}

impl core::fmt::Display for Http3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Http3(source: {:?}, destination: {:?})", self.source, self.destination)
    }
}

// -- ProjectDefinition

#[derive(Clone, Debug, From)]
pub struct ProjectDefinition {
    pub proxy: ProxyType,
    pub target_directory: String,
    pub workspace_root: String,
    pub crate_name: String,
    pub skip_rust_checks: bool,
    pub stop_on_failure: bool,
    pub reload_directories: Vec<String>,
    pub build_directories: Vec<String>,
    pub build_arguments: Vec<String>,
    pub run_arguments: Vec<String>,
    pub wait_before_reload: time::Duration,
}

impl core::fmt::Display for ProjectDefinition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
