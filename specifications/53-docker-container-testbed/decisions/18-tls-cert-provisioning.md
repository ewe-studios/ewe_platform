# 18 — TLS termination + auto-cert provisioning (Stage 2)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

Add TLS termination to `foundation_proxy` with automatic cert provisioning via
ACME (Let's Encrypt) and Cloudflare DNS-01. Cert lifecycle — provision, store,
renew, hot-reload — is managed by a pluggable `CertManager` trait backed by VFS
(`foundation_nativeapis`). The `SslConfig` types already exist in `config.rs`;
this decision defines the runtime machinery that makes them work.

## Why

Stage 1 proxies HTTP only. The proxy must terminate TLS for:
1. **Testbed services** — HTTPS fronting for containers behind the proxy
2. **Zero-downtime deploys** — TLS must survive backend swaps without handshake break
3. **Wildcard certs** — One `*.example.com` cert covers every subdomain (Cloudflare DNS-01)

kamal-proxy does this with Go's `autocert` + CLI flags. We match that with
rustls + pluggable cert managers.

## Architecture

```
ProxyServer::start()
  ├── if ssl enabled:
  │     ├── CertManager::get_cert(domain)
  │     │     ├── VFS hit → return cached cert
  │     │     └── VFS miss/expired → provision new cert
  │     ├── rustls::ServerConfig with cert + key
  │     ├── TlsListener (foundation_netio or foundation_http SSL)
  │     └── spawn CertRenewalTask (background, daily check)
  └── HTTP listener (optional, for redirect or plain-text services)
```

## Types (refining existing skeleton)

```rust
// Already in config.rs — unchanged:
pub enum SslProvider {
    LetsEncrypt { email: String },
    Cloudflare { zone_id: String },
    Static { cert: PathBuf, key: PathBuf },
    None,
}

// New trait — the pluggable cert provisioning interface:
pub trait CertManager: Send + Sync {
    /// Return the current cert pair for `domain`, provisioning if needed.
    fn get_cert(&self, domain: &str) -> Result<CertPair, ProxyError>;
    /// True if the cert expires within 30 days and should be renewed.
    fn needs_renewal(&self, domain: &str) -> bool;
    /// Force renewal (called by background task).
    fn renew_cert(&self, domain: &str) -> Result<CertPair, ProxyError>;
}

pub struct CertPair {
    pub cert_chain: Vec<Certificate>,
    pub private_key: PrivateKey,
    pub not_after: DateTime<Utc>,
}
```

## Implementations

| Manager | Challenge | Cert store | Use case |
|---------|-----------|------------|----------|
| `LetsEncryptCertManager` | ACME HTTP-01 or DNS-01 | VFS | Public-facing, no Cloudflare |
| `CloudflareCertManager` | ACME DNS-01 via Cloudflare API | VFS | Wildcard certs, vm-uncloud pattern |
| `StaticCertManager` | None (reads from disk) | Local FS | Pre-provisioned, CI, internal |

## VFS-backed cert storage

Certs persist across restarts. Backend chosen per environment:

| Environment | VFS backend | Path |
|-------------|------------|------|
| Local dev | `LocalFs` | `~/.foundation/proxy/certs/` |
| CI | `LocalFs` or In-memory | temp dir |
| Production | Cloudflare R2 or S3 | shared bucket |

## Background renewal

```rust
struct CertRenewalTask {
    managers: Vec<Arc<dyn CertManager>>,
    domains: Vec<String>,
    shutdown: Arc<OnSignal>,
}

impl CertRenewalTask {
    fn run(&self) {
        // Every 24h, for each domain, check needs_renewal() → renew_cert() → hot-reload
    }
}
```

Hot-reload: rustls `ServerConfig` uses `Arc<ResolvesServerCert>` — swap the
inner `Arc` and new connections pick up the new cert. No downtime.

## SSL redirect

When TLS is enabled, a plain-HTTP listener on :80 answers `301 Moved Permanently`
to `https://` for every request. Configurable per-service.

## Dependencies

| Crate | Role |
|-------|------|
| `rustls` | TLS termination (already in foundation_netio deps) |
| `acme-micro` or `rustls-acme` | ACME protocol |
| `foundation_nativeapis::vfs` | Cert storage (local FS, R2, S3) |
| `foundation_deployment_cloudflare` | DNS-01 TXT record creation/deletion |
| `foundation_db` | Optional: track cert expiry across restarts |

## Integration with existing code

- `ProxyServer::start()`: if `config.ssl.provider != None`, construct CertManager,
  get cert, build TLS listener, spawn renewal task.
- `ProxyHandler`: no change — it sees plain HTTP after TLS termination.
- Health probes: optional HTTPS probe (connect via TLS, verify cert).
- The `SslConfig` / `SslProvider` types are unchanged. This decision only adds
  the runtime machinery behind them.

## Verification

1. `cargo test -p foundation_proxy --features docker-tests` — proxy starts with
   `SslProvider::Static`, serves HTTPS, test client connects via rustls.
2. Manual: `SslProvider::LetsEncrypt` against a real domain, cert provisioned,
   stored in VFS, survives restart.
3. Renewal: set cert to expire in 1 day, background task renews, hot-reload works.
