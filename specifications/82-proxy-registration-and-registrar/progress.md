# Spec 82 Progress: Proxy Registration and Registrar

_Last updated: 2026-11-23._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — proxy registration | 🔨 6/11; stage 1 (T1–T6) green | Moved from spec 83 feature 03 |
| 01 — registrar sidecar | ⏳ 0/10, design complete | Moved from spec 83 feature 04 |

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

## Landed — feature 00 stage 1 (T1–T6), 2026-11-23

| Task | State | Where |
|---|---|---|
| T1 — proto and generated types | ✅ | `proto/ewe/proxy/registration/v1/registration.proto`, `build.rs`, `src/registration/proto.rs` |
| T2 — `RouterHandle` | ✅ | `src/router.rs`, `src/state.rs` |
| T3 — registry and sessions | ✅ | `src/registration/registry.rs` |
| T4 — leases and sweeper | ✅ | `src/registration/registry.rs`, `src/registration/mod.rs` |
| T5 — rotation rule | ✅ | registry snapshot + `BackendRuntime::is_eligible` |
| T6 — address policy | ✅ | `src/registration/policy.rs`, `src/registration/config.rs` |

Tests: `registration_registry_tests` (12 pass) and `registration_failure_tests`
(11 pass) cover rows 7.1–7.9, 7.12 and 7.13 with a simulated clock. The whole
crate test suite, `cargo fmt --all -- --check` and clippy pass.

`ProxyConfig` gained `registration: Option<RegistrationConfig>`, and
`foundation_proxy` went from 0.1.9 to 0.2.0 because the public API grew. No
workspace crate depends on `foundation_proxy`.

## Next — feature 00 stage 2 (T7–T11)

1. T7: enrollment and the mutual-TLS registration listener over `foundation_auth`.
2. T8: certificate renewal and revocation.
3. T9: the Unix socket transport with its permission check.
4. T10: control socket `issue-registrar-token`, `revoke-registrar`, registered rows in `list`.
5. T11: the end-to-end test (register, route, redeploy, proxy restart).

The `server_proof` of `EnrollResponse` waits on a public signer in
`foundation_auth` (see `LEARNINGS.md`). Feature 01 can start against the landed
contract now.

## Open questions

- 00: save the registry across restarts; the Unix socket transport; mTLS through the proxy (SNI
  passthrough); shared enrollment in `foundation_auth`; metrics for load balancing.
- 01: restart on liveness failure; Kubernetes probes; more metrics.
