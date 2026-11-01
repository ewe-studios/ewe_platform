---
feature: "DNS resolution no longer blocks the HTTP client task"
status: "pending"
priority: "high"
depends_on: ["04-nonblocking-connect-tls"]
branch: "fix/io-36-netio-async-dns"
crates: ["foundation_netio"]
backlog: "docs/bug-backlog.md §2 (1 entry)"
---

# Feature 36: Async DNS resolution in the HTTP client

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Domain Name System (DNS); Hypertext Transfer Protocol
(HTTP); pull request (PR); AeroSpace and Defence Industries Association of Europe (ASD), the
owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `foundation_netio/src/shared/client/dns.rs` `SystemDnsResolver::resolve` (line
  130) calls `to_socket_addrs()` (line 137), a blocking system call. The caller is
  `HttpConnectionPool::resolve_host` (`foundation_netio/src/http/connection.rs` line 692),
  which the request task runs (`foundation_netio/src/http/tasks/request_redirect.rs` line 611)
  before the non-blocking connect (feature 04).
- **Confirm:** read the call chain above and note that `DnsResolver::resolve`
  (`dns.rs` line 38) is synchronous. A slow resolver holds a valtron worker for the whole
  lookup. The entry is still open.

## E1. Make the resolver seam async

- **Where:** `foundation_netio/src/shared/client/dns.rs` `DnsResolver` trait (line 23).
- **Fix:** make `resolve` an async method (`async fn` or a returned boxed future, so the
  trait stays object-safe for `BoxedDnsResolver`). Update the four implementors
  (`SystemDnsResolver`, `StaticSocketAddr`, `CachingDnsResolver`, `MockDnsResolver`) and the
  `Arc<T>` blanket impl (line 48) to match. The `SystemDnsResolver` body below (E2) runs the
  blocking lookup on a blocking boundary; the other resolvers stay cheap and await nothing.
  Record which object-safety technique you use (`async fn` via `async_trait`, or a manual
  `Pin<Box<dyn Future + Send>>` return) in the PR body.
- **Tests:** `foundation_netio/tests/` (registered in `tests/**/mod.rs`):
  `dns_mock_resolver_resolves_async` — `MockDnsResolver::with_response("example.test", …)`
  returns the configured addresses when awaited, with no real network. Keep any existing
  resolver tests updated to the async signature.

## E2. Offload the system lookup with a configured timeout

- **Where:** `foundation_netio/src/shared/client/dns.rs` `SystemDnsResolver::resolve` (line
  130) and the client timeout config `foundation_netio/src/shared/http/timeout.rs` (the
  config behind `TimeoutCalculator`, which already holds `connect_timeout`).
- **Fix:** `to_socket_addrs()` is a blocking system call and standard Rust has no async
  `getaddrinfo`. Run it on a blocking boundary (a bounded worker/offload seam), not on the
  valtron worker. Add a DNS timeout to the client timeout config as a field with a builder
  method and a documented default. Name the source: reuse the existing `connect_timeout` if
  one default is enough, otherwise default to 30 s like nginx's `resolver_timeout`. Per rule
  8, the timeout is configuration, not a constant. The resolver seam stays injectable (rule
  11): tests swap in a mock, and the real system resolver is built only at the host entry
  point.
- **Tests:** `dns_slow_resolver_does_not_block_the_worker` — a resolver whose `resolve`
  returns only after a delay; while a task awaits it, a separate ticker task keeps
  advancing, which proves the worker was not held. `dns_resolution_timeout_is_enforced` — a
  resolver that never returns; the await fails with the configured timeout. Both use the
  deterministic mock seam and no real network.

```mermaid
flowchart LR
    A[request task] --> B[HttpConnectionPool::resolve_host]
    B --> C[DnsResolver::resolve async]
    C --> D[system resolver on blocking boundary]
    C --> E[mock resolver in tests]
    D --> F[configured DNS timeout]
```

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] `DnsResolver::resolve` is async and no caller blocks on it.
- [ ] The system lookup runs on a blocking boundary with a configured timeout (rule 8).
- [ ] `dns_mock_resolver_resolves_async`, `dns_slow_resolver_does_not_block_the_worker` and
      `dns_resolution_timeout_is_enforced` pass.
- [ ] The backlog entry is deleted.
- [ ] CI green.

## Review checklist (main session)

- [ ] The resolver seam is injectable and tested with the mock, not real DNS.
- [ ] The DNS timeout default names a source in its doc comment.
- [ ] No `to_socket_addrs()` (or other blocking lookup) remains on the valtron worker.
- [ ] wasm is unaffected (`NoopDnsResolver` and the browser path still work).
