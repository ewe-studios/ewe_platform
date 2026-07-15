//! `foundation_proxy` — reverse proxy with health-check routing, weighted load
//! balancing, HTTP forwarding, and raw TCP passthrough.
//!
//! **Stage 1 (this crate's current scope): the data plane.** It binds an HTTP
//! front end, routes by `Host` + longest `path_prefix` (wildcard hosts
//! supported), load-balances with weighted round-robin over only healthy,
//! `Active`, under-capacity backends, runs per-backend health probes, forwards
//! HTTP (stripping hop-by-hop headers, appending `X-Forwarded-*`), relays
//! WebSocket upgrades, and passes `tcp://` backends through byte-for-byte.
//!
//! TLS/ACME/Cloudflare, state persistence, Unix-socket RPC, and the `proxy!`
//! macro are later stages.
//!
//! **Three config paths** (all converge on `ProxyConfig`):
//! 1. `proxy!` macro — compile-time, type-checked, single binary (later stage)
//! 2. Programmatic builder — runtime, dynamic, full Rust
//! 3. `proxy.toml` file — reloadable, clap `--config` arg
//!
//! # Runtime requirement
//! [`ProxyServer::start`] serves via `foundation_http`, which submits
//! connections to a valtron pool. Callers **must** initialise a pool
//! (`foundation_core::valtron::initialize_pool`) and hold its guard for the
//! server's lifetime.

pub mod acme;
pub mod config;
pub mod control;
pub mod forward;
pub mod h2_proxy;
pub mod h3_proxy;
pub mod handler;
pub mod health;
pub mod passthrough;
pub mod persistence;
pub mod router;
pub mod runtime;
pub mod server;
pub mod state;
pub mod tls;

pub use config::{
    BackendProtocol, BackendState, BackendTarget, HealthCheckConfig, ProxyConfig, ProxyError,
    ServiceConfig, SslConfig, SslProvider,
};
pub use forward::SharedHttpClient;
pub use handler::ProxyHandler;
pub use health::{HealthMonitor, ProbeState};
pub use passthrough::{TcpPassthrough, UdpPassthrough};
pub use router::Router;
pub use runtime::{BackendLease, BackendRuntime, ServiceRuntime};
pub use server::ProxyServer;
pub use state::ProxyState;
pub use foundation_macros::proxy;
