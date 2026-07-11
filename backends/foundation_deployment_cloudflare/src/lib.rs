//! Cloudflare API v4 client — DNS records, zones, certificates.
//!
//! Hand-written domain logic (`client`, `types`, `dns_ops`).
//! Auto-generated API client functions live in `generated/` (run
//! `cargo run --bin genapi -- generate cloudflare` to regenerate).

pub mod client;
pub mod types;
// Re-export the generated shared types (ApiError, ApiResponse, etc.) and
// group modules via the generated entry point.
pub mod generated;

pub use client::CloudflareClient;
pub use types::{cf_err, CloudflareError, DnsRecord, DnsRecordPatch, DnsRecordType, Zone, ZoneStatus};
