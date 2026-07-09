//! Proper domain types for Cloudflare API responses.
//!
//! Replaces the auto-generated `HashMap<String, Value>` wrappers with
//! typed Rust structs. Used by `CloudflareClient` for DNS record CRUD.

use chrono::{DateTime, Utc};
use derive_more::{Display, Error};
use serde::{Deserialize, Serialize};

// ── CloudflareError ──

/// Errors from Cloudflare API operations.
#[derive(Debug, Display, Error)]
pub enum CloudflareError {
    #[display("Cloudflare auth failed: {_0}")]
    Auth(String),
    #[display("Zone not found: {_0}")]
    ZoneNotFound(String),
    #[display("DNS record not found: {_0}")]
    DnsRecordNotFound(String),
    #[display("API error ({status}): {message}")]
    Api { status: u16, message: String },
    #[display("HTTP transport error: {_0}")]
    Http(String),
    #[display("JSON parse error: {_0}")]
    Json(String),
    #[display("I/O error: {_0}")]
    Io(String),
}

impl foundation_errstacks::ErrorTrace<CloudflareError> {
    pub fn cf_err(e: CloudflareError) -> Self { Self::new(e) }
}

impl std::error::Error for CloudflareError {}

// ── Zone ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: ZoneStatus,
    pub name_servers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoneStatus { Active, Pending, Initializing, Moved, Deleted }

// ── DNS Record ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub zone_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: DnsRecordType,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_on: DateTime<Utc>,
    pub modified_on: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DnsRecordType {
    A,
    Aaaa,
    Cname,
    Txt,
    Mx,
    Ns,
    Soa,
    Srv,
    Caa,
    Ptr,
    Loc,
    Ds,
    Dnskey,
    Https,
    Svcb,
}

impl DnsRecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A", Self::Aaaa => "AAAA", Self::Cname => "CNAME",
            Self::Txt => "TXT", Self::Mx => "MX", Self::Ns => "NS",
            Self::Soa => "SOA", Self::Srv => "SRV", Self::Caa => "CAA",
            Self::Ptr => "PTR", Self::Loc => "LOC", Self::Ds => "DS",
            Self::Dnskey => "DNSKEY", Self::Https => "HTTPS", Self::Svcb => "SVCB",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DnsRecordPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxied: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}
