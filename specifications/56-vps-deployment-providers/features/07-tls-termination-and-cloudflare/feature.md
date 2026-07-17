---
feature: "TLS termination proof + Cloudflare cert provider"
description: "Prove the proxy actually terminates HTTPS end-to-end (currently wired but never exercised), and implement SslProvider::Cloudflare — ACME DNS-01 solved through the Cloudflare API for wildcard certs"
status: "not-started"
priority: "high"
phase: 4
depends_on: []
estimated_effort: "medium"
created: 2026-07-17
moved_from: "spec-53 final completeness review (2026-07-17)"
---
# Feature 07: TLS Termination Proof + Cloudflare Cert Provider

> **Moved here from spec-53**, whose final review surfaced these two gaps against
> its decisions 18 and 24. Spec-53 is done; these are pending, and a VPS fronting
> HTTPS ([feature 06](../06-composed-deployables/feature.md)) depends on both — so they
> live here.

## What spec-53's final review found (2026-07-17)

Spec-53's decision 18 promises TLS termination with cert provisioning across
providers.
Two gaps:

### 1. TLS termination is wired but never exercised

`server.rs:169` builds an `SSLAcceptor` from the cert pair and hands it to
`server_config.with_tls(acceptor)`, so termination *is* wired. But **no test ever
makes a TLS connection through the proxy**:

- `acme_provisioning_tests::acme_cert_manager_provisions_a_usable_cert` proves a
  mock CA's cert can *construct* an `SSLAcceptor` — not that a request survives it.
- `acme_provisioning_tests::proxy_server_starts_with_acme_letsencrypt_provider`
  proves the server *starts* with TLS configured.
- `proxy_features_tests::ssl_redirect_301_to_https` configures a static cert but
  only exercises the **plain-HTTP → 301** path.

So "HTTPS in → decrypted → forwarded to a backend → response back out" is
**unproven**. That is the same "wired, not verified" class that left F02/F03/F04
broken behind a "code present" label, and it is the proxy's headline feature.

### 2. `SslProvider::Cloudflare` is not implemented

`tls.rs::build_cert_manager` handles `Static` and `LetsEncrypt`; for
`Cloudflare { zone_id }` it returns an error:

> "Cloudflare Origin CA (zone {zone_id}) not yet supported; provision an Origin
> CA cert and configure it via the Static provider."

Honest (it errors rather than silently doing nothing), but incomplete — and the
message describes the **wrong feature**. Spec-53's decision 18 does not ask for Origin CA:

| Decision 18's table | Means |
|---|---|
| `LetsEncryptCertManager` — ACME HTTP-01 or DNS-01 | ✅ built as `AcmeCertManager`, verified vs a mock CA |
| `CloudflareCertManager` — **ACME DNS-01 via Cloudflare API**, wildcard certs | ❌ not built |

So the Cloudflare provider is *Let's Encrypt certs whose DNS-01 challenge is
solved through the Cloudflare API* — which is how you get a wildcard
`*.example.com` (spec-53 decision 18, "Wildcard certs"). It is unwired, not unbuildable:
every piece exists.

- The hook: `acme::Dns01Setter = Arc<dyn Fn(&str, &str) -> Result<(), String>>`.
  Today the DNS-01 setter is entirely caller-supplied (`ProxyConfig::acme(...)`),
  which is why `LetsEncrypt` is marked "DNS-01 setter is a hook".
- The implementation: `foundation_deployment_cloudflare` already has
  `upsert_dns_record` and `DnsRecordType::Txt` — exactly what a DNS-01 challenge
  needs (spec-53 decision 18 lists this crate as the "DNS-01 TXT record
  creation/deletion" provider).

`SslProvider::Cloudflare { zone_id }` should therefore build an `AcmeCertManager`
whose `dns01_setter` writes the `_acme-challenge` TXT record via that crate.

### 3. Cloudflare DNS ops are untested, and `find_zone` ignores its argument

`foundation_deployment_cloudflare` has 12 tests, all on typed records
(`types_tests` — spec-53 decision 24, which is genuinely covered). Its actual DNS
operations have **none**: `list_dns_records`, `upsert_dns_record`,
`delete_dns_record`, `delete_dns_records_by_name`, `find_zone`,
`bootstrap_domain`.

And `find_zone(&self, _domain: &str)` **ignores `domain`** — it fetches the
configured `zone_id` and reports whether *that* exists, so it answers a different
question than it is asked (`find_zone("someone-elses.com")` returns `true`). Same
silently-dropped-parameter family as F03's `subnet`/`network_alias` and F04's
`memory = "256m"`.

## Scope

1. **TLS termination e2e** — a test that connects with a real TLS client to the
   proxy's HTTPS listener with a self-signed cert, sends a request, and asserts
   the backend's body comes back. Covers `Static`, then the ACME-provisioned cert
   from the mock CA (proving a provisioned cert actually serves traffic, not just
   that it parses).
2. **`SslProvider::Cloudflare`** — build an `AcmeCertManager` with a
   Cloudflare-backed `Dns01Setter` (`upsert_dns_record` of a TXT
   `_acme-challenge.<domain>`), including cleanup of the record after validation.
   Fix the misleading Origin-CA error text.
3. **Cloudflare DNS ops tests** — against a mock Cloudflare API (the ACME tests
   already stand up a mock CA, so the pattern exists).
4. **`find_zone`** — either query zones by name (`GET /zones?name=<domain>`) so it
   answers what it is asked, or take no argument. It must not keep ignoring one.

## Verification plan

- TLS termination and the Cloudflare DNS-01 setter are both testable **locally**:
  self-signed certs for the former, a mock Cloudflare API + the existing mock ACME
  CA for the latter. No account needed.
- A real Let's Encrypt + real Cloudflare zone run is the only part needing
  credentials; do not claim it works until someone has run it.
