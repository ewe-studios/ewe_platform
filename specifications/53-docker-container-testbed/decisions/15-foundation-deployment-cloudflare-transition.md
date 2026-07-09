# 15 — foundation_deployment_cloudflare Transition

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Convert `foundation_deployment_cloudflare` from an auto-generated API stub to a
hand-maintained crate. Take the auto-generated code as a starting point, then
replace the `HashMap<String, Value>` response types with proper domain structs,
add auth management, and build the higher-level operations needed by
`foundation_proxy` (DNS record CRUD with proper types, zone lookup, token
management). The auto-generated code is the initial scaffold, not the final form.

## Table of Contents

1. [What exists today](#what-exists-today)
2. [What's wrong with the auto-generated code](#whats-wrong-with-the-auto-generated-code)
3. [What we keep and what we replace](#what-we-keep-and-what-we-replace)
4. [New type hierarchy](#new-type-hierarchy)
5. [Module reorganisation](#module-reorganisation)
6. [Feature gate strategy](#feature-gate-strategy)
7. [Implementation plan](#implementation-plan)

---

## What exists today

51K lines of auto-generated code (`cargo run --bin ewe_platform gen_api`),
1,215 public items (functions, structs, enums) across 46 feature-gated modules:

```
foundation_deployment_cloudflare/
  src/lib.rs          // "Cloudflare API provider modules."
  src/mod.rs          // Auto-generated, all modules feature-gated
  src/shared/mod.rs   // 50+ shared response types + common re-exports
  src/zones/mod.rs    // 1.8MB, 51K lines — the main module we care about
  src/dns/mod.rs      // Account-level DNS infrastructure (firewalls, views, TSIG)
  src/certificates/   // SSL certificate management
  src/workers/        // Workers and Pages
  src/zones/          // Zone-level management including DNS records
  ... 40+ more modules for every Cloudflare API surface
```

Every request function follows this pattern:

```rust
pub fn dns_records_for_a_zone_list_dns_records_request<R, F>(
    client: &SimpleHttpClient<R>,
    args: &DnsRecordsForAZoneListDnsRecordsArgs,
    builder_mod: Option<F>,
) -> Result<impl TaskIterator<...>, ApiError>
where
    R: DnsResolver + Clone + Default + 'static,
    F: FnOnce(&mut ClientRequestBuilder<R>),

// Called with auth injected via closure:
let task = list_dns_records_request(&client, &args, Some(|b| {
    b.header("Authorization", "Bearer {token}")
}))?;
```

Every response type is a `HashMap<String, Value>` in a serde(flatten) wrapper:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonHash)]
pub struct DnsRecordsDnsResponseSingle {
    #[serde(flatten)]
    pub data: HashMap<String, serde_json::Value>,
}
```

---

## What's wrong with the auto-generated code

1. **No type safety.** Every response is `HashMap<String, Value>`. You can't
   access `record.name` or `record.content` — you parse JSON at runtime. The
   compiler can't help with field renames, missing fields, or type mismatches.
2. **No auth management.** The `builder_mod` closure pattern requires every
   call site to manually inject the `Authorization` header. This is boilerplate
   and error-prone — easy to forget, easy to leak the token into logs.
3. **No higher-level operations.** `upsert_record` needs two API calls (list by
   name+type → create or update). The auto-generated code has no composition
   — every function is exactly one HTTP endpoint.
4. **Massive binary bloat.** 51K lines of functions, each monomorphised per
   `R: DnsResolver` and `F: FnOnce`. A crate that uses even a few endpoints
   pays for all 1,215 functions in compile time and IDE analysis.
5. **Not editable.** The header says "DO NOT EDIT MANUALLY." We need to own
   the types — add methods, derive traits, refactor, fix bugs.

---

## What we keep and what we replace

### Keep

- **Request builder pattern** — the valtron/SimpleHttpClient integration is
  correct. Functions returning `Result<impl TaskIterator<...>>` compose well
  with the valtron executor.
- **Feature-gated module structure** — `#[cfg(feature = "cloudflare_zones")]`
  keeps compile times down for crates that only need DNS. We keep this pattern.
- **Shared types** — `ApiError`, `ApiResponse<T>`, `Operation`, `Empty` from
  `foundation_deployment::providers::common`. These are correct.
- **DNS record endpoint coverage** — all 10 DNS record operations (list, create,
  get, update, patch, delete, batch, export, import, scan) are needed. The
  function signatures are a starting point.

### Replace

- **`HashMap<String, Value>` response types** → proper domain structs
  (`DnsRecord`, `DnsRecordList`, `Zone`, etc. with named fields).
- **`builder_mod` auth closure** → `CloudflareClient` struct that holds the
  token and injects it automatically.
- **Thin endpoint functions** → higher-level methods on `CloudflareClient`
  (`upsert_dns_record`, `delete_dns_record_by_name`, `find_zone_by_domain`).

### Remove

- **40+ modules we don't use** — only `zones` (DNS records + zone management),
  `certificates` (Origin CA), and `shared` (common types) are needed for
  `foundation_proxy`. Other modules (workers, analytics, stream, security,
  queues, etc.) are noise. They can stay as feature-gated stubs or be deleted
  — we don't compile them.

---

## New type hierarchy

```rust
// ── Core client ──

/// Owns the HTTP client + token. Every Cloudflare operation goes through this.
/// Replaces the `builder_mod` closure pattern.
pub struct CloudflareClient {
    client: SimpleHttpClient<DnsResolver>,
    token: String,   // "Authorization: Bearer {token}" injected automatically
}

impl CloudflareClient {
    /// Create from env: reads CLOUDFLARE_API_TOKEN and CLOUDFLARE_ZONE_ID
    pub fn from_env() -> Result<Self>;
    pub fn new(token: String) -> Self;
}

// ── Domain types (replace HashMap<String, Value>) ──

pub struct Zone {
    pub id: String,
    pub name: String,                  // e.g. "example.com"
    pub status: ZoneStatus,            // active, pending, initializing, etc.
    pub name_servers: Vec<String>,
    pub plan: ZonePlan,
}

pub enum ZoneStatus { Active, Pending, Initializing, Moved, Deleted }

pub struct DnsRecord {
    pub id: String,
    pub zone_id: String,
    pub name: String,                  // e.g. "*.example.com"
    pub r#type: DnsRecordType,         // A, AAAA, CNAME, TXT, MX, etc.
    pub content: String,               // e.g. "1.2.3.4" or "abc123..."
    pub ttl: u32,                      // 1 = auto
    pub proxied: bool,                 // orange-cloud
    pub priority: Option<u32>,         // MX only
    pub comment: Option<String>,
    pub tags: Vec<String>,
    pub created_on: DateTime<Utc>,
    pub modified_on: DateTime<Utc>,
}

pub enum DnsRecordType {
    A, Aaaa, Cname, Txt, Mx, Ns, Soa, Srv, Caa, Ptr, Loc, Ds, Dnskey, Https, Svcb,
}

// ── Higher-level operations on CloudflareClient ──

impl CloudflareClient {
    // ── Zones ──

    /// Find a zone by domain name. Returns None if no zone matches.
    pub async fn find_zone(&self, domain: &str) -> Result<Option<Zone>>;

    // ── DNS Records ──

    /// List all DNS records, optionally filtered by type and/or name.
    pub async fn list_dns_records(
        &self, zone_id: &str, r#type: Option<DnsRecordType>, name: Option<&str>,
    ) -> Result<Vec<DnsRecord>>;

    /// Create a DNS record. Returns the created record (with server-assigned id).
    pub async fn create_dns_record(&self, zone_id: &str, record: &DnsRecord) -> Result<DnsRecord>;

    /// Update an existing DNS record (PUT — full replacement).
    pub async fn update_dns_record(&self, zone_id: &str, record: &DnsRecord) -> Result<DnsRecord>;

    /// Patch a DNS record (PATCH — partial update, e.g. TTL or proxied only).
    pub async fn patch_dns_record(
        &self, zone_id: &str, record_id: &str, fields: &DnsRecordPatch,
    ) -> Result<DnsRecord>;

    /// Delete a DNS record by ID.
    pub async fn delete_dns_record(&self, zone_id: &str, record_id: &str) -> Result<()>;

    /// Upsert: if a record with the same name + type exists, update it.
    /// Otherwise create it. Returns the record ID.
    pub async fn upsert_dns_record(
        &self, zone_id: &str, record: &DnsRecord,
    ) -> Result<String>;

    /// Delete all records matching a name + type. Used for ACME challenge cleanup.
    pub async fn delete_dns_records_by_name(
        &self, zone_id: &str, name: &str, r#type: DnsRecordType,
    ) -> Result<usize>;  // returns count of deleted records

    // ── Domain bootstrap (for foundation_proxy startup) ──

    /// Ensure wildcard and optional apex A records exist pointing to the
    /// proxy's public IP. Idempotent — skips if records already exist.
    pub async fn bootstrap_domain(
        &self, domain: &str, public_ip: &str, setup_apex: bool,
    ) -> Result<()>;
}
```

The `DnsRecord` struct has real typed fields. Accessing `record.name` returns
a `&str`. The compiler catches typos and missing fields.

---

## Module reorganisation

```
foundation_deployment_cloudflare/
  Cargo.toml
  src/
    lib.rs             // pub mod client; pub mod types; + re-exports
    client.rs          // CloudflareClient (token + HTTP + high-level methods)
    types.rs           // Zone, DnsRecord, DnsRecordType, ZoneStatus, DnsRecordPatch
    error.rs           // CloudflareError enum
    dns.rs             // DNS record CRUD (low-level request builders, kept from auto-gen)
    zones.rs           // Zone management (find_zone, list_zones)
    certs.rs           // Origin CA certificate operations
    shared.rs          // ApiError, ApiResponse, Operation, Empty (from foundation_deployment)

  // All other auto-generated modules moved to archive/ as reference material.
  // They're not compiled unless a feature flag explicitly includes them.
  archive/
    workers/
    analytics/
    stream/
    security/
    ... (40+ modules, feature-gated off by default)
```

### Feature flags

```toml
[features]
default = ["dns", "zones"]       # foundation_proxy needs these
dns = []                         # DNS record CRUD
zones = []                       # zone management
certs = []                       # Origin CA cert provisioning
full = ["dns", "zones", "certs"] # everything we actively maintain

# Legacy auto-generated surfaces (not compiled by default):
workers = []
analytics = []
stream = []
# ... etc
```

---

## Implementation plan

### Phase 1: Core types (no HTTP)

1. Write `src/types.rs` — `Zone`, `DnsRecord`, `DnsRecordType`, `ZoneStatus`,
   `DnsRecordPatch`. Serde derives for Cloudflare API JSON shapes.
2. Write `src/error.rs` — `CloudflareError` enum (Api, Auth, ZoneNotFound,
   DnsRecordNotFound, Io, Json).
3. Update `Cargo.toml` — add `serde`, `serde_json`, `chrono`, change
   description from "placeholder".

### Phase 2: HTTP client

4. Write `src/client.rs` — `CloudflareClient` with `from_env()`, token
   injection, and the higher-level CRUD methods defined above.
5. Under the hood, these call the existing auto-generated request builders
   from `zones/mod.rs` — we keep the raw HTTP plumbing, just wrap it.
6. The `builder_mod` closure is now internal — `CloudflareClient` owns the
   token and passes it automatically.

### Phase 3: DNS record operations

7. Implement `list_dns_records`, `create_dns_record`, `delete_dns_record`,
   `update_dns_record` — each maps to one auto-generated request builder.
8. Implement `upsert_dns_record` — list by name+type, create or update.
9. Implement `delete_dns_records_by_name` — list, then batch-delete matches.
10. Implement `bootstrap_domain` — upsert wildcard A record, optional apex.

### Phase 4: Archive and clean up

11. Move unused modules to `src/archive/` — keep as reference, gate behind
    feature flags, exclude from default compilation.
12. Update `src/lib.rs` to only expose `client`, `types`, `error`, `dns`,
    `zones`, `certs` by default. Archive modules behind explicit features.
13. Write tests against Cloudflare's API sandbox or recorded HTTP fixtures.

### Phase 5: Integrate with foundation_proxy

14. `foundation_proxy` depends on `foundation_deployment_cloudflare` (default
    features: `dns`, `zones`).
15. Replace decision 14's inline `CloudflareDns` with
    `foundation_deployment_cloudflare::CloudflareClient`.
16. `CloudflareDns01CertManager` uses `CloudflareClient::upsert_dns_record()`
    and `delete_dns_records_by_name()` for ACME challenge TXT records.
