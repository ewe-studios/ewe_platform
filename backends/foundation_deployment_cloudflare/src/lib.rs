//! Cloudflare API v4 client — DNS records, zones, certificates.
//!
//! Hand-written domain logic (`client`, `types`, `dns_ops`).
//! Auto-generated API client functions live in `generated/` (run
//! `cargo run --bin genapi -- generate cloudflare` to regenerate).
//!
//! Workers runtime support (gated: `workers` feature):
//! Durable Objects, WebSocket, Env bindings for crates targeting
//! the Cloudflare Workers platform.

// Native-only: REST API client + generated API functions.
// The workers module uses `worker::Fetch` / `wasm-fetch` instead.
#[cfg(not(target_family = "wasm"))]
pub mod client;
#[cfg(not(target_family = "wasm"))]
pub mod dns_ops;
#[cfg(not(target_family = "wasm"))]
pub mod types;
#[cfg(not(target_family = "wasm"))]
pub mod generated;

#[cfg(all(target_family = "wasm", feature = "workers"))]
pub mod workers;

#[cfg(not(target_family = "wasm"))]
pub use client::CloudflareClient;
#[cfg(not(target_family = "wasm"))]
pub use types::{
    cf_err, CloudflareApiError, CloudflareError, CloudflareResponse, DnsRecord, DnsRecordInput,
    DnsRecordPatch, DnsRecordType, ResultInfo, Zone, ZoneStatus,
};
