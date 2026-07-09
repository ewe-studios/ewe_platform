//! Cloudflare API v4 client — DNS records, zones, certificates.
//!
//! Built on top of the auto-generated valtron `TaskIterator` functions in
//! `zones/mod.rs`. `CloudflareClient` wraps those with auth injection and
//! typed domain structs (`DnsRecord`, `Zone`, `DnsRecordType`).

pub mod client;
pub mod types;
pub mod shared;

pub use client::CloudflareClient;
pub use types::{cf_err, CloudflareError, DnsRecord, DnsRecordPatch, DnsRecordType, Zone, ZoneStatus};
