# 14 — foundation_proxy

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Create a `foundation_proxy` crate — a Rust reverse proxy with zero-downtime
deployments, automatic SSL termination, and health-check-based traffic routing.
Learn from Kamal (Basecamp's deploy tool) and kamal-proxy (its Go reverse proxy).
Build on Foundation crates: `foundation_netio` (TCP/TLS), `foundation_http`
(request/response), `foundation_db` (state persistence), `foundation_nativeapis`
(Unix socket RPC, VFS cert storage), and `foundation_deployment_cloudflare`
(Cloudflare API v4 client for DNS + wildcard TLS).

## Table of Contents

1. [Why a Foundation proxy](#why-a-foundation-proxy)
2. [Prior art: Kamal + kamal-proxy](#prior-art-kamal--kamal-proxy)
3. [Three config paths](#three-config-paths)
4. [Architecture](#architecture)
5. [SSL termination and cert provisioning](#ssl-termination-and-cert-provisioning)
6. [Cloudflare integration: DNS + TLS + wildcard certs](#cloudflare-integration-dns--tls--wildcard-certs)
7. [Zero-downtime deployment](#zero-downtime-deployment)
8. [State persistence](#state-persistence)
9. [Integration with the workspace](#integration-with-the-workspace)
10. [What Kamal does that we defer](#what-kamal-does-that-we-defer)

---

## Why a Foundation proxy

The workspace needs a reverse proxy for:

1. **Testbed service exposure** — When running multi-container test scenarios
   (app + database + cache), the proxy routes external traffic to the
   correct container. This is the dockurr web viewer pattern (port 8006
   tunneled over SSH) generalized.

2. **Zero-downtime deploys** — When `foundation_deployment` rolls out a new
   container version, the proxy holds in-flight requests on the old version
   while the new one becomes healthy, then atomically swaps.

3. **SSL termination** — Automatic cert provisioning (Let's Encrypt ACME,
   Cloudflare DNS-01) for test and staging environments. Certs stored in
   VFS (`foundation_nativeapis` — local FS, S3, Cloudflare R2).

4. **Health-check-based routing** — Only route traffic to healthy backends.
   Periodic health probes. Pause/resume individual backends.

---

## Prior art: Kamal + kamal-proxy

Kamal is Basecamp's zero-downtime deployment tool. The proxy component
(`kamal-proxy`) is a standalone Go binary deployed as a Docker container.

### kamal-proxy architecture (what we learn from)

| Component | Go source | Foundation equivalent |
|-----------|-----------|----------------------|
| HTTP/HTTPS/HTTP3 server | `server.go` | `foundation_netio` TLS listener |
| Request routing | `router.go` | `foundation_http` request matching |
| Service definition | `service.go` | `ServiceConfig` struct |
| Target health checks | `health_check.go` | `HealthProbe` with configurable interval/path |
| Load balancer | `load_balancer.go` | Round-robin across healthy targets |
| State persistence | `router.go` (JSON file) | `foundation_db` (SQLite or similar) |
| Unix socket RPC | `commands.go` | `foundation_nativeapis` Unix socket |
| TLS (ACME) | `cert.go` | `rustls-acme` or `acme-micro` |
| Middleware chain | `logging`, `buffer`, `error_page` | `tower`-style middleware stack |
| Canary rollouts | `rollout_controller.go` | Percentage-based traffic splitting |
| Writer affinity | `load_balancer.go` (cookies) | Cookie-based session stickiness |

### Kamal deploy YAML (the configuration we mirror)

```yaml
service: app
image: dhh/app
servers:
  web:
    - 1.1.1.1
    - 1.1.1.2
  workers:
    - 1.1.1.3: site1
env:
  clear:
    REDIS_URL: redis://x/y
  tags:
    site1:
      SITE: site1
proxy:
  ssl: true
  host: app.example.com
accessories:
  mysql:
    image: mysql:5.7
    host: 1.1.1.3
    port: 3306
    directories:
      - data:/var/lib/mysql
```

Our Rust equivalent is `ContainerServiceDefinition` (decision 01), which
already expresses the app + accessory container topology. `foundation_proxy`
adds the `proxy:` layer — SSL, host routing, health checks.

---

## Three config paths

`foundation_proxy` exposes three paths for defining the proxy configuration.
All three converge on the same runtime types (`ProxyConfig`, `ServiceConfig`,
`SslConfig`) — the difference is when and how the config is constructed.

### Path 1: `proxy!` macro — compile-time, baked into the binary

```rust
use foundation_proxy::proxy;

fn main() {
    let config = proxy! {
        domain: "example.com",
        ssl: lets_encrypt { email: "admin@example.com" },
        services: {
            windows_viewer: {
                host: "windows.example.com",
                backends: ["http://localhost:8006"],
            },
            app: {
                host: "app.example.com",
                backends: ["http://localhost:3000"],
                health_check: "/up",
            },
        },
    };

    config.start().await?;
}
```

- Single binary, zero external files. Deploy and run.
- Config is type-checked at compile time — invalid service names, missing
  required fields, wrong types are caught by the compiler.
- Equivalent to vm-uncloud recipes baked into a Docker image.
- **Protocol inference:** The protocol is derived from the backend URL scheme:
  `http://`/`https://` → HTTP reverse proxy (headers, WebSocket upgrade,
  health checks, cookies, buffering); `tcp://` → raw TCP passthrough (RDP,
  VNC, dockurr noVNC — byte-level streaming, no HTTP semantics). No separate
  `proto` field needed.

### Path 2: Programmatic builder — runtime, dynamic

```rust
use foundation_proxy::{ProxyConfig, ServiceConfig, SslConfig, HealthCheckConfig};

fn main() {
    let domain = std::env::var("PROXY_DOMAIN").unwrap();
    let public_ip = std::env::var("PUBLIC_IP").unwrap();

    let config = ProxyConfig::new(&domain, public_ip)
        .ssl(SslConfig::lets_encrypt("admin@example.com"))
        .service(ServiceConfig::new("app", "app.example.com")
            .backend("http://localhost:3000")
            .health_check("/up", Duration::from_secs(5)))
        .service(ServiceConfig::new("windows", "windows.example.com")
            .backend("http://localhost:8006"))
        .build();

    config.start().await?;
}
```

- Full Rust expressiveness — read env vars, query APIs, compute at runtime.
- Same `ProxyConfig` struct as the macro produces. No special types.
- Good for CI, dynamic environments, multi-tenant setups.

### Path 3: File-based — `proxy.toml`, reloadable

```toml
# proxy.toml
domain = "example.com"
public_ip = "1.2.3.4"

[ssl]
lets_encrypt = { email = "admin@example.com" }

[[services]]
name = "app"
host = "app.example.com"
backends = ["http://localhost:3000"]
health_check = "/up"

[[services]]
name = "windows"
host = "windows.example.com"
backends = ["http://localhost:8006"]
```

```rust
use foundation_proxy;

fn main() {
    // Load from a file beside the binary (or specified via clap)
    let config = foundation_proxy::load_file("./proxy.toml")?;
    config.start().await?;
}
```

- Update config without rebuilding — just edit `proxy.toml` and restart.
- Clap argument for path: `proxy --config ./proxy.toml`. If no argument
  provided, defaults to `./proxy.toml` in the current directory.
- Same `ProxyConfig` struct on the other side — deserialized via `toml` crate.
  Validation is identical to paths 1 and 2 (same struct, same invariants).

### Comparison

| Path | Config lives in | Rebuild to change? | Type-checked at compile time? | Best for |
|------|----------------|---------------------|------------------------------|----------|
| Macro | Rust source code | Yes | Yes | Single-purpose deploy, CI |
| Programmatic | Rust source code | Yes | Yes | Dynamic config, multi-tenant |
| File (toml) | Sidecar file | No (restart only) | At load time (runtime validation) | Ops-managed, shared infra |

### Implementation note

All three paths produce the same `ProxyConfig` struct. The macro desugars to
the builder pattern. The file path deserializes via `serde` (TOML). Validation
happens once, at `ProxyConfig::build()`, regardless of which path was used:

```rust
impl ProxyConfig {
    /// Validate and resolve the config. Called by all three paths.
    pub async fn start(self) -> Result<ProxyServer, ProxyError> {
        // 1. Validate: no duplicate hosts, backends are reachable, SSL config is valid
        // 2. Bootstrap Cloudflare DNS (if using Cloudflare provider)
        // 3. Provision or load TLS certs
        // 4. Start the router + health probes
        // 5. Return running ProxyServer handle
    }
}
```

---

## Architecture

```
                 ┌─────────────────────────────────────────────────┐
                 │              foundation_proxy                    │
                 │                                                 │
  Internet ──────┤                                                 │
                 │                                                 │
                 │  ┌─────────────────────────────────────────┐    │
                 │  │ CloudflareClient (zone: example.com)        │    │
                 │  │   A *.example.com → 1.2.3.4              │    │
                 │  │   TXT _acme-challenge.* → (ephemeral)    │    │
                 │  └─────────────────────────────────────────┘    │
                 │                      │                          │
                 │  ┌───────────────────▼──────────────────────┐    │
Internet ───────▶│  │ TLS (rustls + CertManager)               │    │
:443             │  │   CloudflareAcmeCertManager              │    │
                 │  │     → ACME DNS-01 via Cloudflare API      │    │
                 │  │     → VFS cert store (local/R2/S3)        │    │
                 │  │     → Background renewal + hot-reload     │    │
                 │  └───────────────────┬──────────────────────┘    │
                 │                      │                          │
                 │  ┌───────────────────▼──────────────────────┐    │
                 │  │ Router (host + path matching)             │    │
                 │  │   ├── windows.example.com → dockurr:8006 │    │
                 │  │   ├── app.example.com      → backend_a   │    │
                 │  │   └── *.example.com        → backend_c   │    │
                 │  └───────────────────┬──────────────────────┘    │
                 │                      │                          │
                 │  Health Probes   Load Balancers   State DB      │
                 │  Unix Socket RPC (deploy, remove, pause, ...)   │
                 │    ↑                                             │
                 └────┼─────────────────────────────────────────────┘
                      │
        foundation_deployment_platform
        (ContainerGroup calls proxy RPC on deploy)
```

### Key types

```rust
/// A service fronted by the proxy.
pub struct ServiceConfig {
    pub name: String,
    pub host: String,                // e.g. "app.example.com"
    pub path_prefix: Option<String>, // e.g. "/api"
    pub ssl: SslConfig,
    pub health_check: HealthCheckConfig,
    pub backends: Vec<BackendTarget>,
}

pub struct SslConfig {
    pub enabled: bool,
    pub provider: SslProvider,
}

pub enum SslProvider {
    /// Let's Encrypt via ACME (HTTP-01 or DNS-01 challenge)
    LetsEncrypt { contact_email: String, challenge: AcmeChallenge },
    /// Cloudflare DNS-01 (uses CLOUDFLARE_API_TOKEN)
    Cloudflare { zone_id: String },
    /// Static cert + key (files or bytes)
    Static { cert: VfsPath, key: VfsPath },
}

pub enum AcmeChallenge { Http01, Dns01 }

pub struct HealthCheckConfig {
    pub path: String,            // e.g. "/up"
    pub interval: Duration,      // e.g. 5s
    pub timeout: Duration,       // e.g. 2s
    pub healthy_threshold: u32,  // consecutive successes to mark healthy (default 2)
    pub unhealthy_threshold: u32,// consecutive failures to mark unhealthy (default 3)
}

pub struct BackendTarget {
    pub url: String,             // e.g. "http://container:3000"
    pub weight: u32,             // for weighted round-robin (default 1)
    pub max_connections: u32,    // connection limit
}
```

---

## SSL termination and cert provisioning

`foundation_proxy` uses `rustls` for TLS termination. Cert provisioning is
pluggable:

| Provider | Challenge | Use case |
|----------|-----------|----------|
| **Let's Encrypt (ACME)** | HTTP-01 or DNS-01 | Public-facing services with a domain |
| **Cloudflare DNS-01** | API token | Services behind Cloudflare (vm-uncloud pattern) |
| **Static** | File/VFS | Self-signed or pre-provisioned certs for internal/CI use |

Certs are stored in VFS (`foundation_nativeapis::vfs`):
- Local filesystem — dev and CI
- Cloudflare R2 — production (vm-uncloud's R2 state backend pattern)
- S3 — AWS deployments
- In-memory — ephemeral test certs

The `CertManager` trait provides a unified interface:

```rust
pub trait CertManager: Send + Sync {
    async fn get_cert(&self, domain: &str) -> Result<CertPair>;
    async fn renew_cert(&self, domain: &str) -> Result<CertPair>;
    async fn needs_renewal(&self, domain: &str) -> Result<bool>;
}
```

`LetsEncryptCertManager` uses `acme-micro` (or `rustls-acme`) for the ACME
protocol. `CloudflareCertManager` uses the Cloudflare API for DNS-01
challenges. `StaticCertManager` reads from VFS paths.

---

## Cloudflare integration: DNS + TLS + wildcard certs

This is the central pattern learned from vm-uncloud: a single Cloudflare zone
manages DNS for all services, and wildcard domains provide instant subdomain
resolution with zero per-service DNS setup.

### Why Cloudflare

vm-uncloud uses Cloudflare for three things:

1. **DNS hosting** — The apex domain's nameservers point to Cloudflare. All
   DNS records (A, CNAME, TXT) are managed via the Cloudflare API.
2. **Single wildcard `*.domain.com` A record** — One record resolves every
   subdomain to the proxy's IP. No per-service DNS changes. A new service at
   `redis.example.com` resolves instantly because `*` already covers it.
3. **DNS-01 TLS challenge** — Let's Encrypt (and Cloudflare Origin CA) can
   validate domain ownership by writing a `_acme-challenge` TXT record via the
   Cloudflare API. This is the only way to get wildcard certs
   (`*.example.com`) from Let's Encrypt — HTTP-01 can't do wildcards.

### The wildcard DNS model

```
Cloudflare Zone: example.com
  ├── A     example.com        → 1.2.3.4   (optional apex)
  ├── A     *.example.com      → 1.2.3.4   (wildcard — covers EVERYTHING)
  └── TXT   _acme-challenge.*  → (ephemeral, created/renewed during cert issuance)
```

```
Request:  windows.example.com  ─→  *.example.com resolves to 1.2.3.4
Request:  app.example.com      ─→  *.example.com resolves to 1.2.3.4
Request:  anything.example.com ─→  *.example.com resolves to 1.2.3.4
```

No per-service DNS records. A new service is a single line of proxy config:

```rust
proxy.register(ServiceConfig {
    name: "my-new-service".into(),
    host: "my-new-service.example.com".into(),  // instantly resolves via wildcard
    backends: vec![...],
    ..Default::default()
}).await?;
```

### Cloudflare API integration

`foundation_proxy` manages Cloudflare DNS via the **existing
`foundation_deployment_cloudflare` crate** — transitioned from auto-generated
stubs to a hand-maintained crate with proper domain types (decision 15).
The `CloudflareClient` struct provides auth management, type-safe DNS record
CRUD, and higher-level operations (`upsert_record`, `bootstrap_domain`).

The API token needs exactly one permission: `Zone:DNS:Edit` for the target
zone — scoped, not an account-wide token. This matches vm-uncloud's token
scoping.

```rust
use foundation_deployment_cloudflare::{CloudflareClient, DnsRecord, DnsRecordType};

let cf = CloudflareClient::from_env()?;
let zone = cf.find_zone("example.com").await?.unwrap();

// Upsert wildcard A record — idempotent
cf.upsert_dns_record(&zone.id, &DnsRecord {
    name: "*.example.com".into(),
    r#type: DnsRecordType::A,
    content: "1.2.3.4".into(),
    ttl: 60,
    proxied: false,  // DNS-only for ACME
    ..Default::default()
}).await?;

// Create ACME challenge TXT record
let challenge_name = "_acme-challenge.example.com";
cf.upsert_dns_record(&zone.id, &DnsRecord {
    name: challenge_name.into(),
    r#type: DnsRecordType::Txt,
    content: "expected-token-value".into(),
    ttl: 60,
    proxied: false,
    ..Default::default()
}).await?;

// Clean up after ACME validation
cf.delete_dns_records_by_name(&zone.id, challenge_name, DnsRecordType::Txt).await?;
```

All DNS operations go through `CloudflareClient` — no raw request builders,
no manual auth injection. Type-safe `DnsRecord` structs replace the
auto-generated `HashMap<String, Value>`. See decision 15 for the full type
hierarchy and implementation plan.

### DNS-01 challenge flow (Let's Encrypt via Cloudflare)

The `CloudflareAcmeCertManager` implements the ACME DNS-01 challenge by
creating/deleting TXT records via `CloudflareClient` (from
`foundation_deployment_cloudflare`):

```
1. ACME order created for *.example.com
   → Let's Encrypt returns challenge token + expected TXT value

2. Create TXT record: _acme-challenge.example.com → "expected-value"
   cloudflare.upsert_record("TXT", "_acme-challenge.example.com", value, ttl: 60)
   → Wait for propagation (60s TTL, but Cloudflare API is near-instant)

3. Notify Let's Encrypt: challenge is ready
   → Let's Encrypt queries _acme-challenge.example.com TXT via public DNS
   → Verifies the value matches → issues *.example.com certificate

4. Delete TXT record: cleanup challenge record
   cloudflare.delete_record(record_id)

5. Store cert in VFS: cert chain + private key → VFS path
   → rustls loads cert from VFS on startup + renewal
```

```rust
pub struct CloudflareAcmeCertManager {
    dns: CloudflareClient,
    acme: AcmeClient,               // acme-micro or rustls-acme
    cert_store: Box<dyn CertStore>,  // VFS-backed
}

impl CertManager for CloudflareAcmeCertManager {
    async fn get_cert(&self, domain: &str) -> Result<CertPair> {
        if let Some(cert) = self.cert_store.load(domain).await? {
            if !self.needs_renewal(domain).await? {
                return Ok(cert);
            }
        }

        // Order a new wildcard cert via ACME DNS-01
        let order = self.acme.new_order(domain).await?;
        let challenge = order.dns01_challenge()?;

        // Write the challenge TXT record
        let record_id = self.dns.upsert_record(
            DnsRecordType::TXT,
            &challenge.record_name(),
            &challenge.record_value(),
            60,         // short TTL for fast propagation
            false,      // NOT proxied — ACME needs the raw TXT value
        ).await?;

        // Let ACME validate
        self.acme.complete_challenge(&challenge).await?;
        let cert = order.finalize().await?;

        // Clean up challenge record
        let _ = self.dns.delete_record(&record_id).await;

        // Persist
        self.cert_store.save(domain, &cert).await?;
        Ok(cert)
    }

    async fn needs_renewal(&self, domain: &str) -> Result<bool> {
        match self.cert_store.load(domain).await? {
            None => Ok(true),
            Some(cert) => {
                // Renew when within 30 days of expiry
                let expiry = cert.not_after()?;
                Ok(expiry < Utc::now() + Duration::days(30))
            }
        }
    }
}
```

### Cloudflare Origin CA (alternative to Let's Encrypt)

Cloudflare provides its own CA that issues certificates trusted ONLY behind
Cloudflare's reverse proxy (orange-cloud mode). These certs have longer
lifetimes (up to 15 years vs Let's Encrypt's 90 days) and don't require
ACME challenges — just an API call with the Origin CA key:

```rust
pub enum SslProvider {
    /// Let's Encrypt via ACME DNS-01 (wildcard support, 90-day certs)
    LetsEncrypt { contact_email: String },
    /// Cloudflare Origin CA (15-year certs, Cloudflare-trusted only)
    CloudflareOriginCa { api_key: String },
    /// Static cert + key
    Static { cert: VfsPath, key: VfsPath },
}
```

Origin CA is simpler but the cert only works when Cloudflare is the
front-end (orange-cloud proxy). For direct connections (no Cloudflare
proxy), Let's Encrypt is required.

### Apex + wildcard DNS setup (one time)

On first run, or via `foundation_proxy init --domain example.com`, the proxy
ensures the DNS records exist:

```rust
// bootstrap_domain lives on CloudflareClient (decision 15).
// Called once at proxy startup:
cf.bootstrap_domain("example.com", "1.2.3.4", setup_apex: false).await?;
```

The result is identical to vm-uncloud's OpenTofu output: one wildcard
`*.example.com` A record, TTL 60, DNS-only (grey cloud), created once and
never touched again.

### Token scoping and security

Following vm-uncloud's discipline: the Cloudflare API token has the minimum
scope needed — `Zone:DNS:Edit` for the specific zone — not an account-wide
token. The token comes from the environment (`CLOUDFLARE_API_TOKEN`) or a local
secrets store, never from committed files:

```rust
// CloudflareClient reads credentials from env (decision 15).
// CLOUDFLARE_API_TOKEN: scoped Zone:DNS:Edit — never committed to files.
// CLOUDFLARE_ZONE_ID: from Cloudflare dashboard sidebar.
let cf = CloudflareClient::from_env()?;
```

### Integration with proxy startup

```
foundation_proxy start --domain example.com
  │
  ├── 1. CloudflareClient::bootstrap_domain("example.com", <public_ip>)
  │      → Ensures *.example.com A record exists (idempotent)
  │
  ├── 2. CloudflareAcmeCertManager::get_cert("*.example.com")
  │      → Checks VFS for existing cert
  │      → If missing or expiring: ACME DNS-01 challenge via Cloudflare TXT
  │      → Stores new cert in VFS (R2 for production, local FS for dev)
  │
  ├── 3. rustls loads cert from VFS
  │      → Starts TLS listener on :443
  │
  ├── 4. Router starts, health probes connect, state DB loads
  │      → Ready for traffic
  │
  └── Background: CertRenewalTask (daily cron)
         → For each managed domain, check needs_renewal()
         → Renew if within 30 days of expiry
         → Hot-reload cert into rustls (no downtime)
```

### VFS-backed cert storage

Certs persist across proxy restarts via VFS. The backend is chosen per
environment:

| Environment | VFS Backend | Rationale |
|-------------|------------|-----------|
| Local dev | `LocalFs("~/.foundation/proxy/certs/")` | Zero setup |
| CI | `LocalFs` or In-memory | Ephemeral, test certs |
| Hetzner single node | `LocalFs("/var/lib/foundation-proxy/certs/")` | Simple, fast |
| Production (multi-node) | `Cloudflare R2` | Durable, shared across nodes (vm-uncloud's R2 state pattern) |
| AWS | `S3` | Native, shared |

```rust
use foundation_nativeapis::vfs::{Vfs, LocalFs, R2Backend, S3Backend};

fn cert_store() -> Box<dyn CertStore> {
    match env("PROXY_ENV").unwrap_or("local") {
        "production" => Box::new(R2CertStore::new(
            R2Backend::from_env()?,
            "proxy-certs",
        )),
        "aws" => Box::new(S3CertStore::new(
            S3Backend::from_env()?,
            "foundation-proxy-certs",
        )),
        _ => Box::new(LocalCertStore::new(
            dirs::data_dir().join("foundation-proxy/certs"),
        )),
    }
}
```

This is the same pattern as vm-uncloud's R2 terraform state backend —
choose the VFS backend based on environment, swap transparently. The
`CertStore` trait doesn't care where the bytes live.

### What we get from this design

| Capability | Mechanism | vm-uncloud equivalent |
|------------|-----------|----------------------|
| DNS for wildcard domains | `CloudflareClient::bootstrap_domain()` | OpenTofu `cloudflare_dns_record` |
| Wildcard TLS cert | `CloudflareAcmeCertManager` (ACME DNS-01) | Caddy `tls { dns cloudflare }` |
| Cert persistence across restarts | VFS (local FS / R2 / S3) | Caddy's `/data` volume |
| Cert auto-renewal | Background cron task, hot-reload | Caddy's built-in cert maintenance |
| One-time DNS setup | `upsert_record` idempotent | OpenTofu plan/apply idempotency |
| Secrets never on disk | `CLOUDFLARE_API_TOKEN` env var, scoped | `fnox exec` keychain injection |

---

## Zero-downtime deployment

The deployment sequence mirrors Kamal's:

1. **Start new container** — `ContainerGroup::start(vec![new_version])` with
   a unique container name (version-tagged).
2. **Health probe** — `foundation_proxy` health-checks the new container at
   its `/up` endpoint (configurable path, interval, threshold).
3. **Register backend** — Once healthy, the new container is added as a
   `BackendTarget` with `weight: 0` (no traffic yet).
4. **Gradual rollout** — Weight is incremented from 0 → 100 over the
   rollout period (canary deploy). Configurable percentage steps.
5. **Drain old backend** — Old backends are marked for draining. In-flight
   requests complete (up to `drain_timeout`). New requests go only to the
   new backend.
6. **Remove old backend** — Old container stopped and removed.

### Atomic swap

For instant cutover (no gradual rollout), the load balancer supports an
atomic swap:

```rust
proxy.replace_backends("app", old_backends, new_backends).await?;
```

This acquires a write lock on the service's routing table, validates the
new backends are healthy, atomically replaces the backend list, and drains
the old backends. In-flight requests on old backends are tracked with a
reference count — the swap doesn't complete until all in-flight requests
finish or `drain_timeout` expires.

### Writer affinity

For stateful applications, writer affinity cookies ensure a client that
performs a write (POST/PUT/PATCH/DELETE) is routed to the same backend for
subsequent requests within the session TTL:

```rust
proxy_config = ServiceConfig {
    writer_affinity: Some(WriterAffinityConfig {
        cookie_name: "kamal_writer".into(),
        ttl: Duration::from_secs(300),
    }),
    ..
};
```

---

## State persistence

`foundation_proxy` persists its routing state via `foundation_db`:

```sql
-- Services
CREATE TABLE services (
    name TEXT PRIMARY KEY,
    host TEXT NOT NULL,
    path_prefix TEXT,
    ssl_config TEXT NOT NULL,  -- JSON
    health_check_config TEXT NOT NULL  -- JSON
);

-- Backends
CREATE TABLE backends (
    id TEXT PRIMARY KEY,
    service_name TEXT NOT NULL REFERENCES services(name),
    url TEXT NOT NULL,
    weight INTEGER NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'active',  -- active, draining, paused
    FOREIGN KEY (service_name) REFERENCES services(name)
);
```

State survives proxy restarts — on startup, the proxy reads the DB and
reconnects health probes to known backends. This matches kamal-proxy's JSON
state file pattern.

---

## Integration with the workspace

`foundation_proxy` depends on:

| Crate | Role |
|-------|------|
| `foundation_netio` | TCP/TLS listener, Unix socket RPC |
| `foundation_http` | HTTP request/response, reverse proxy |
| `foundation_db` | SQLite-backed state persistence |
| `foundation_nativeapis` | VFS for cert storage (local FS, S3, R2), Unix sockets |
| `foundation_deployment_cloudflare` | Cloudflare API v4 client (DNS record CRUD, zones, certs) — auto-generated, feature-gated |
| `rustls` | TLS termination |
| `acme-micro` or `rustls-acme` | Let's Encrypt ACME protocol |

The `CloudflareClient`, `DnsRecord`, and `DnsRecordType` types live in
`foundation_deployment_cloudflare` (decision 15 — transitioned from auto-generated
to hand-maintained). `foundation_proxy` depends on that crate directly — no
intermediate wrapper needed.

`foundation_deployment_platform` integrates with `foundation_proxy` for:
- Registering containers as backends after `ContainerGroup::start`
- Health-check-driven deploy orchestration
- Container drain and removal on `ContainerGroup::drop`

---

## What Kamal does that we defer

Kamal is a full deployment orchestrator. `foundation_proxy` is just the proxy.
We defer these Kamal responsibilities to other crates:

| Kamal feature | Deferred to |
|---------------|-------------|
| Image building + pushing to registry | `foundation_deployment_platform` (image management) |
| Container lifecycle on remote hosts | `foundation_deployment_platform` (bollard SSH) |
| SSH host provisioning | `foundation_sshkit` |
| Deploy locking | `foundation_db` (distributed lock) |
| Pre/post deploy hooks | `foundation_shell` (script execution) |
| Accessory containers (DB, cache) | `ContainerServiceDefinition` (decision 01) |
| YAML config parsing | `serde_yaml` + `ContainerServiceDefinition` |
