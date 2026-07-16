# 24 — Cloudflare typed DNS records

**Date:** 2026-07-10
**Status:** Resolved

## Decision

`foundation_deployment_cloudflare` deserializes Cloudflare API responses into
typed `DnsRecord` structs instead of `HashMap<String, Value>`. This is the
final step in Decision 15's transition from auto-generated stubs to a
hand-maintained, type-safe Cloudflare client.

## Why

The current `CloudflareClient` wraps `foundation_netio::SimpleHttpClient` and
returns raw `HashMap<String, Value>` from list/upsert/delete operations. Callers
must `.get("name")`, `.as_str()`, `.unwrap()` — every access point is a
potential runtime panic on a schema mismatch.

Typed deserialization:
- **Catches API changes at compile time** — a renamed field breaks the build, not production.
- **Eliminates string-key typos** — `record.name` not `record.get("naem")`.
- **Enables pattern matching** — `match record.r#type { DnsRecordType::A => ..., ... }`.
- **Self-documenting** — the struct fields are the API contract.

## Types

```rust
/// A DNS record as returned by the Cloudflare API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: String,
    pub zone_id: String,
    pub zone_name: String,
    pub name: String,              // e.g. "*.example.com"
    #[serde(rename = "type")]
    pub r#type: DnsRecordType,
    pub content: String,           // e.g. "1.2.3.4"
    pub ttl: u32,
    pub proxied: bool,             // orange-cloud vs grey-cloud
    pub proxiable: bool,
    pub locked: bool,
    pub created_on: String,        // ISO 8601
    pub modified_on: String,       // ISO 8601
    pub comment: Option<String>,
    pub tags: Vec<String>,
    // Not exhaustive — Cloudflare may add fields. We deserialize only what we use.
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
    Srv,
    // ... others as needed
}

/// Request body for creating/updating a DNS record (subset of fields).
#[derive(Debug, Clone, Serialize)]
pub struct DnsRecordInput {
    #[serde(rename = "type")]
    pub r#type: DnsRecordType,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
    pub comment: Option<String>,
    pub tags: Vec<String>,
}

/// A Cloudflare zone.
#[derive(Debug, Clone, Deserialize)]
pub struct Zone {
    pub id: String,
    pub name: String,
    pub status: ZoneStatus,
    pub paused: bool,
    #[serde(rename = "name_servers")]
    pub nameservers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ZoneStatus {
    Active,
    Pending,
    Initializing,
    Moved,
    Deleted,
    Deactivated,
}

/// Paginated list response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct CloudflareResponse<T> {
    pub success: bool,
    pub errors: Vec<CloudflareError>,
    pub messages: Vec<String>,
    pub result: T,
    pub result_info: Option<ResultInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultInfo {
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
    pub count: u32,
    pub total_count: u32,
}
```

## Client API (updated)

```rust
impl CloudflareClient {
    /// List all DNS records for a zone, with optional type/name filters.
    pub fn list_dns_records(
        &self,
        zone_id: &str,
        record_type: Option<DnsRecordType>,
        name: Option<&str>,
    ) -> Result<Vec<DnsRecord>, CloudflareError>;

    /// Create or update a DNS record (idempotent — matches by name+type).
    pub fn upsert_dns_record(
        &self,
        zone_id: &str,
        input: &DnsRecordInput,
    ) -> Result<DnsRecord, CloudflareError>;

    /// Delete a DNS record by ID.
    pub fn delete_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
    ) -> Result<(), CloudflareError>;

    /// Delete all DNS records matching name+type (used for ACME cleanup).
    pub fn delete_dns_records_by_name(
        &self,
        zone_id: &str,
        name: &str,
        record_type: DnsRecordType,
    ) -> Result<(), CloudflareError>;

    /// Find a zone by domain name.
    pub fn find_zone(&self, domain: &str) -> Result<Option<Zone>, CloudflareError>;

    /// Ensure the wildcard A record exists (bootstrap).
    pub fn bootstrap_domain(
        &self,
        domain: &str,
        public_ip: &str,
    ) -> Result<DnsRecord, CloudflareError>;
}
```

## Migration path

The existing `dns_ops.rs` functions (which return `HashMap<String, Value>`) are
reimplemented to return typed structs. The old functions are removed. Callers
(`foundation_proxy`'s TLS cert manager, Decision 18) use the typed API directly.

## Dependencies

| Crate | Role |
|-------|------|
| `serde` + `serde_json` | Deserialize Cloudflare API JSON responses |
| `foundation_netio` (`SimpleHttpClient`) | HTTP transport (already used) |
| `foundation_errstacks` | `CloudflareError` (already used) |

## Verification

1. `CloudflareClient::list_dns_records()` against a real zone → typed `Vec<DnsRecord>`.
2. `CloudflareClient::upsert_dns_record()` → create, verify, update, verify.
3. `CloudflareClient::bootstrap_domain()` on a test domain → wildcard A record created.
4. Malformed API response (missing field) → `serde` error, not panic.
5. Existing `dns_ops` tests updated to use typed API.
