# 14 — foundation_proxy

**Date:** 2026-07-09
**Status:** Resolved

## Decision

Create a `foundation_proxy` crate — a Rust reverse proxy with zero-downtime
deployments, automatic SSL termination, and health-check-based traffic routing.
Learn from Kamal (Basecamp's deploy tool) and kamal-proxy (its Go reverse proxy).
Build on Foundation crates: `foundation_netio` (TCP/TLS), `foundation_http`
(request/response), `foundation_db` (state persistence), `foundation_nativeapis`
(Unix socket RPC).

## Table of Contents

1. [Why a Foundation proxy](#why-a-foundation-proxy)
2. [Prior art: Kamal + kamal-proxy](#prior-art-kamal--kamal-proxy)
3. [Architecture](#architecture)
4. [SSL termination and cert provisioning](#ssl-termination-and-cert-provisioning)
5. [Zero-downtime deployment](#zero-downtime-deployment)
6. [State persistence](#state-persistence)
7. [Integration with the workspace](#integration-with-the-workspace)
8. [What Kamal does that we defer](#what-kamal-does-that-we-defer)

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

## Architecture

```
                   ┌─────────────────────────────────┐
                   │        foundation_proxy          │
                   │                                 │
  Internet ───────▶│  TLS (rustls + ACME)            │
                   │    │                             │
                   │    ▼                             │
                   │  Router (host + path matching)   │
                   │    │                             │
                   │    ├── app.example.com    → backend_a (round-robin)
                   │    ├── api.example.com    → backend_b (round-robin)
                   │    └── *.example.com      → backend_c (catch-all)
                   │                                 │
                   │  Health Probes (periodic GET)    │
                   │  Load Balancers (per backend)    │
                   │  State DB (SQLite via foundation_db) │
                   │  Unix Socket RPC                 │
                   │    ↑                             │
                   │    │ (deploy, remove, pause,     │
                   │    │  resume, status)            │
                   └────┼─────────────────────────────┘
                        │
              foundation_deployment_platform
              (ContainerGroup::deploy calls RPC)
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
| `foundation_http` | HTTP request/response, reverse proxy via `reqwest` or `hyper` |
| `foundation_db` | SQLite-backed state persistence |
| `foundation_nativeapis` | VFS for cert storage (local FS, S3, R2), Unix sockets |
| `rustls` | TLS termination |
| `acme-micro` or `rustls-acme` | Let's Encrypt ACME protocol |

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
