# Spec 82 Progress: Proxy Registration and Registrar

_Last updated: 2026-11-23._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — proxy registration | ✅ 11/11; all tasks green | Moved from spec 83 feature 03 |
| 01 — registrar sidecar | ✅ 10/10; all tasks green | Moved from spec 83 feature 04 |

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


## Feature 00 complete — stage 2 (T7–T11), 2026-11-23

| Task | State | Where |
|---|---|---|
| T7 — enrollment and mTLS listener | ✅ | `src/registration/enroll.rs`, `authority.rs`, `service.rs` |
| T8 — renewal and revocation | ✅ | `enroll.rs` (`renew`, `revoke_host`), `registry.rs` (`drain_host`) |
| T9 — Unix socket transport | ✅ | `src/registration/unix.rs` |
| T10 — control commands | ✅ | `src/control.rs` (`issue-registrar-token`, `revoke-registrar`, rows in `list`) |
| T11 — end to end | ✅ | `tests/registration_e2e_tests.rs` |

The registration authority starts from `RegistrationConfig`: it loads or
generates the registration certificate authority (CA) in `state_dir`, opens the
trust store, loads or makes the enrollment-token key seed in `state_dir` from
the `Ctx` `Env` entropy, and serves the mTLS listener, the server-auth
enrollment listener and the optional Unix socket. The proxy holds it and stops
it on teardown.

Tests: `registration_auth_tests` (8), `registration_e2e_tests` (1),
`registration_registry_tests` (12) and `registration_failure_tests` (11). The
whole crate suite, `cargo fmt --all -- --check` and clippy pass.

Notes:
- `ProxyServer::start` now returns a boxed future, because the start future holds
  the registration authority's future and the trait solver overflows when a test
  calls `.block()` on the unboxed type.
- A `Ctx` without the host `Env` is a typed start error: enrollment needs
  entropy for the token key and the server proof. A host installs it with
  `foundation_core::env::install`.
- The enrollment-token key seed is a 32-byte file, `registrar_token.key`, in
  `state_dir`, mode 0600 on Unix. The first start makes it from the `Env`
  entropy; every later start reads it. A token that the operator issued
  therefore still enrolls after a proxy restart. A test proves it.
- The Unix transport is gated by `cfg(unix)`. A `unix_socket` setting on another
  operating system is a typed start error, not a silent skip.

## What is left

- Feature 01 is complete. The end-to-end test is compiled and skips when no
  Docker daemon answers; the Linux nightly run is the first place that it runs
  against a daemon.
- Open questions of the specification (saved registry across restarts, SNI
  passthrough, shared enrollment in `foundation_auth`) remain open.

## Feature 01 complete — registrar sidecar (T1–T10), 2026-11-23

| Task | State | Where |
|---|---|---|
| T1 — crate, config, binary | ✅ | `backends/foundation_registrar/src/{config.rs,bin/ewe-registrar.rs}` |
| T2 — manifest view and fixtures | ✅ | `src/manifest.rs`, `tests/fixtures/manifest/`, `tests/manifest_tests.rs` |
| T3 — systemd discovery | ✅ | `src/discovery/systemd.rs` |
| T4 — Docker discovery | ✅ | `src/discovery/docker.rs` |
| T5 — prober and health state machine | ✅ | `src/probe.rs`, `tests/probe_tests.rs` |
| T6 — metrics | ✅ | `src/metrics.rs`, `tests/metrics_tests.rs` |
| T7 — registration client | ✅ | `src/client.rs`, `tests/client_tests.rs` |
| T8 — reconcile loop | ✅ | `src/reconcile.rs`, `tests/reconcile_tests.rs` |
| T9 — failure paths | ✅ | `tests/failure_tests.rs`, row 6.11 in `client_tests.rs` |
| T10 — end to end | ✅ | `tests/register_e2e_tests.rs` (gated by `docker-tests`) |

The crate has library `src/lib.rs` and binary `src/bin/ewe-registrar.rs`. It
generates its registration client from
`backends/foundation_proxy/proto` with its own `build.rs` and has no runtime
dependency on `foundation_proxy`.

Tests: 8 config, 8 client, 11 discovery, 11 failure, 9 manifest, 6 metrics, 8
probe and 12 reconcile. The gated end-to-end test ran for real against the
Docker daemon on this machine: install, discovery, probes, registration through
the real proxy, an upgrade with no failed request, and container cleanup. `cargo fmt --all -- --check`,
`cargo clippy -p foundation_registrar --all-targets --no-deps -- -D warnings`,
`cargo build -p foundation_registrar --bin ewe-registrar` and
`python3 scripts/ci/manifest_contract.py --check-copies` pass.

Notes:
- Every limit is a configuration field: `max_manifest_bytes`,
  `detail_max_bytes`, `call_timeout`, `default_report_interval`,
  `backoff_initial`, `backoff_max` and `shutdown_grace`.
- An exec probe runs its process on a dedicated thread, so its timeout fails
  the check and the other probes are not delayed.
- A test can replace every side effect: the clock (`SimulatedClock`), the
  process runner, the Docker calls (`ContainerApi`), the probe executor and the
  registration client.
- `ci/packages.toml` runs the crate on Linux with `runs = ["", "docker-tests"]`.
  macOS and Windows opt out with a reason: they have no Docker socket,
  `systemctl` or `/sys/fs/cgroup`.

### Cross-feature fixes found by the end-to-end test

| Finding | Severity | Change |
|---|---|---|
| The installer pulled with `repo` and `tag`, so the Docker daemon read the name as a URL and failed with "no Host in request URL". | high (blocked T10) | `foundation_installer::DockerEngine::image_pull` now sends `fromImage` and `tag`. |
| The installer stopped the old container at once, before the sidecar could discover the new address, so requests got `502`. | high (blocked T10) | New `InstallerConfig::old_container_grace` (default 15 s, longer than the sidecar's 10 s report interval). The old container stays up while the sidecar discovers and registers the new address. |
| `DockerClient::container_list` returns `serde_json::Value`; no generated summary type exists. | medium | Open issue #769. The discovery reads `Id` and `State` from the value until the generator emits the type. |

## Completion (2026-11-23)

Both features are merged into master. Every test passes. The spec moves to
`specifications/completed/82-proxy-registration-and-registrar`.

Open questions: the saved registry (option B), SNI passthrough and shared
enrollment in `foundation_auth` stay open for later work. Open issues:
#769 (the generated container-list type), #767 (the Unix socket group check)
and #768 (the temporary state directory default).

### The Docker end-to-end test and the CI network (2026-11-23)

The GitHub Actions Linux jobs run the tests inside a `ci` container with the
runner's Docker socket mounted. A `container:` job puts that container on a
private network, so it cannot reach an app container on the daemon's default
bridge. The proxy and the sidecar run in the same process, so they would fail
the same way. The test now checks whether it can reach the app container and
skips with a printed reason when it cannot.

The test passes on a host that runs Docker directly, and in a Linux `ci`
container started without `--network` (verified with the amd64 image and the
host socket). Task T10 is verified on those topologies.
