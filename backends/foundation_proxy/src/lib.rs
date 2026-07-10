//! `foundation_proxy` — reverse proxy with SSL termination, zero-downtime
//! deploys, and health-check-based traffic routing.
//!
//! **Three config paths** (all converge on `ProxyConfig`):
//! 1. `proxy!` macro — compile-time, type-checked, single binary
//! 2. Programmatic builder — runtime, dynamic, full Rust
//! 3. `proxy.toml` file — reloadable, clap `--config` arg

pub mod config;
pub mod server;

pub use config::{BackendTarget, HealthCheckConfig, ProxyConfig, ServiceConfig, SslConfig, SslProvider};
pub use server::ProxyServer;
