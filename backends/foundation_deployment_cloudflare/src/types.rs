//! Proper domain types for Cloudflare API responses.
//!
//! Replaces the auto-generated `HashMap<String, Value>` wrappers with
//! typed Rust structs. Used by `CloudflareClient` for DNS record CRUD.

use chrono::{DateTime, Utc};
use derive_more::Display;
use foundation_errstacks::ErrorTrace;
use serde::{Deserialize, Serialize};

// ── CloudflareError ──

/// Errors from Cloudflare API operations.
#[derive(Debug, Display)]
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

impl std::error::Error for CloudflareError {}

/// Convenience: wrap a CloudflareError in an ErrorTrace.
pub fn cf_err(e: CloudflareError) -> ErrorTrace<CloudflareError> {
    ErrorTrace::new(e)
}

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

// ── DnsRecordInput (request body for create/update) ──

/// Request body for creating or updating a DNS record.
///
/// Contains only the fields the Cloudflare API accepts in POST/PUT bodies
/// (no `id`, `zone_id`, `created_on`, `modified_on`).
#[derive(Debug, Clone, Serialize)]
pub struct DnsRecordInput {
    #[serde(rename = "type")]
    pub r#type: DnsRecordType,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl From<&DnsRecord> for DnsRecordInput {
    fn from(r: &DnsRecord) -> Self {
        Self {
            r#type: r.r#type,
            name: r.name.clone(),
            content: r.content.clone(),
            ttl: r.ttl,
            proxied: r.proxied,
            comment: r.comment.clone(),
            tags: r.tags.clone(),
        }
    }
}

// ── Cloudflare API envelope types ──

/// Typed wrapper for the Cloudflare v4 API response envelope
/// `{ success, errors, messages, result, result_info }`.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareResponse<T> {
    pub success: bool,
    #[serde(default)]
    pub errors: Vec<CloudflareApiError>,
    #[serde(default)]
    pub messages: Vec<String>,
    pub result: T,
    #[serde(default)]
    pub result_info: Option<ResultInfo>,
}

/// API-level error returned inside a Cloudflare response.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareApiError {
    pub code: u32,
    pub message: String,
}

/// Pagination metadata returned with list responses.
#[derive(Debug, Clone, Deserialize)]
pub struct ResultInfo {
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub count: u32,
    pub total_count: u32,
}

