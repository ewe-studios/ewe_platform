//! Cloudflare API v4 client — DNS records, zones, certificates.
//!
//! Hand-written domain logic (`client`, `types`, `dns_ops`).
//! Auto-generated API client functions live in `generated/` (run
//! `cargo run --bin genapi -- generate cloudflare` to regenerate).
//!
//! Workers runtime support (gated: `workers` feature):
//! Durable Objects, WebSocket, Env bindings for crates targeting
//! the Cloudflare Workers platform.

pub mod client;
pub mod dns_ops;
pub mod types;
// Re-export the generated shared types (ApiError, ApiResponse, etc.) and
// group modules via the generated entry point.
pub mod generated;

#[cfg(all(target_family = "wasm", feature = "workers"))]
pub mod workers;

pub use client::CloudflareClient;
pub use types::{
    cf_err, CloudflareApiError, CloudflareError, CloudflareResponse, DnsRecord, DnsRecordInput,
    DnsRecordPatch, DnsRecordType, ResultInfo, Zone, ZoneStatus,
};
