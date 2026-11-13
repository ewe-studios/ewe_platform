# Spec 82 Progress: Proxy Registration and Registrar

_Last updated: 2026-11-14._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — proxy registration | ⏳ 0/11, design complete | Moved from spec 65 feature 03 |
| 01 — registrar sidecar | ⏳ 0/10, design complete | Moved from spec 65 feature 04 |

## What moved

Feature 03 is now `features/00-proxy-registration`; feature 04 is now
`features/01-registrar-sidecar`. The two features share the registration contract, so they stay
in one spec.

The existing proxy parts that the new code extends:

| Part | File |
|---|---|
| Fixed router | `backends/foundation_proxy/src/router.rs` |
| Health probes | `backends/foundation_proxy/src/health.rs` |
| Control socket | `backends/foundation_proxy/src/control.rs` |
| Persisted state | `backends/foundation_proxy/src/persistence.rs` |
| TLS | `backends/foundation_proxy/src/tls.rs` |

## Next steps

1. Finish the move: add the nine-section story wrapper to both features and update the internal
   feature references (old 02 → 78/79, old 07/11 → 81, old 08 → 80).
2. Run feature 00 T1 (the registration contract) against spec 78's `proxy` and `probes` blocks.
3. Feature 01 can use a fake registration server until feature 00 lands. Its final end-to-end
   task needs spec 79.

## Open questions

- 00: save the registry across restarts; the Unix socket transport; mTLS through the proxy (SNI
  passthrough); shared enrollment in `foundation_auth`; metrics for load balancing.
- 01: restart on liveness failure; Kubernetes probes; more metrics.
