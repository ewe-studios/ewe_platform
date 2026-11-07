# Feature 15 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Hypertext Transfer Protocol version 2 (HTTP/2); Automatic
Certificate Management Environment (ACME); Request for Comments (RFC); Secure Sockets Layer
(SSL); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## Landed

- E1 (gRPC health probe leak):
  - `foundation_connectrpc` `H2Transport` gained `read_timeout` (default
    `Duration::ZERO`, disabled) with `DEFAULT_READ_TIMEOUT` and
    `with_read_timeout`. The pump stores `read_timeout` and `last_progress`
    (`Instant::now()` at start). It checks the idle time at the top of each
    `next_status` loop turn, skips `PumpPhase::Handshaking` (which has its own
    deadline), and fails with `TransportError::Timeout` when the peer sends
    nothing for the read timeout. It updates `last_progress` on every successful
    socket read and write. (`src/native/transport/h2.rs`.)
  - `foundation_proxy` `grpc_health_check` opens its `H2Transport` with
    `.with_read_timeout(timeout)`. `probe_grpc_health` is now `pub` and runs
    `block_on_future(grpc_health_check(url, timeout))` inline; the throwaway
    thread and `recv_timeout` backstop are gone. (`src/health.rs`.)
  - Version bumps: `foundation_connectrpc` 0.0.3 -> 0.0.4; `foundation_proxy`
    0.1.2 -> 0.1.3 (new public `probe_grpc_health`); `foundation_proxy` and
    `foundation_deployment_docker` require connectrpc 0.0.4.
  - Test: `foundation_proxy/tests/health_probe_tests.rs`
    `health_probe_of_silent_backend_times_out_and_leaks_nothing` — a raw
    `TcpListener` writes one non-ACK SETTINGS frame then stays silent; 20 probes
    each return `false` in about the probe timeout, and on Linux the thread and
    open-file-descriptor counts return to their start values.
  - Validation under `--profile uat`: connectrpc check green; proxy check green;
    `health_probe_tests` green (6 s); `rpc_health_tests` green;
    `h2_integration_tests` green (7). No new clippy warnings from this change.
  - Backlog 9 "gRPC health probe leaks a thread + H2 connection..." entry deleted.
- E2 (backend-state persistence lost update):
  - `ProxyStateStore` gained a `futures_util::lock::Mutex<()>` `write_lock`. Both
    `save_backend_state` and `save_tls_cert` hold it across their
    load-change-save, so concurrent control-socket writes no longer save over
    each other. The store keeps its one-document shape (old state files stay
    loadable), so a mutex is the smallest fix. (`src/persistence.rs`.)
  - Test: `foundation_proxy/tests/persistence_tests.rs`
    `concurrent_backend_saves_keep_every_update` — 16 threads each save a
    different backend 50 times; the final load holds every backend's last value.
    It fails 3/3 without the lock and passes with it.
  - Validation under `--profile uat`: `persistence_tests` green. No new clippy
    warnings from this change.
  - Backlog 9 "Backend-state persistence can lose an update." entry deleted.
- E3 (ACME RFC 8555 account flow):
  - `AcmeClient` gained a `Session` (`key_id`, `nonce`). `create_account` reads
    the account URL from the newAccount `Location` header and stores it as the
    `kid`; every later JWS signs with the `kid`, never the `jwk`. Each answer's
    `Replay-Nonce` is stored and carried to the next request; a fresh nonce is
    fetched only when none is stored or the CA answers `badNonce`. One
    `post_signed` helper replaced `post_json`/`post_json_typed`/`post_json_raw`.
    The DNS wait in `provision` is an async loop under `dns_wait_timeout` and
    `dns_wait_interval` (`sleep_async`); no `std::thread::sleep`. New public
    builders `with_dns_wait_timeout`/`with_dns_wait_interval` and defaults
    `DEFAULT_DNS_WAIT_TIMEOUT`/`DEFAULT_DNS_WAIT_INTERVAL`. (`src/acme.rs`.)
  - Version bump: `foundation_proxy` 0.1.3 -> 0.1.4 (new public API). No
    consumers to bump (leaf crate).
  - Tests: the mock CA is strict (single-use nonces, `kid` equals the account
    URL, no `jwk` after account creation, `Location` on newAccount). Three new
    tests: `acme_uses_kid_after_account_creation`,
    `acme_reuses_replay_nonce`, and
    `acme_challenge_wait_does_not_block_the_executor` (drives the DNS wait and a
    heartbeat together on one thread; a blocking sleep serialises them to about
    4 s, the async wait overlaps to about 2 s).
  - Validation under `--profile uat`: `acme_provisioning_tests` green (5). No
    new clippy warnings from this change.
  - Backlog 9 "The ACME client does not follow RFC 8555's account flow" entry
    deleted.
- E4 (SSL redirect limits are config, not constants):
  - `ProxyConfig` gained `redirect_head_limit` (4096 bytes),
    `redirect_read_timeout` (2 s) and `redirect_accept_poll` (10 ms) with
    documented defaults and builders `redirect_head_limit`,
    `redirect_read_timeout`, `redirect_accept_poll`. (`src/config.rs`.)
  - `ssl_redirect_loop` reads the head until its blank line or the head limit
    (a head split over reads is no longer cut), answers 431 for a head past the
    limit, sets the accepted socket blocking so the read timeout governs, and
    parks the accept loop on listener readiness (`FdReadiness`) instead of a
    fixed sleep on unix. (`src/server.rs`.)
  - Version bump: `foundation_proxy` 0.1.4 -> 0.1.5 (new public config fields
    and builders). No consumers to bump (leaf crate).
  - Tests: `redirect_handles_head_split_over_reads` and
    `redirect_rejects_head_over_limit` in `tests/proxy_features_tests.rs`.
  - Validation under `--profile uat`: `proxy_features_tests` green (6). No new
    clippy warnings from this change.
  - Backlog 9 "The SSL redirect listener hard-codes its limits" entry deleted.
- E5 (`forward_http` outgoing request is a real test):
  - `tests/diag_client_tap.rs` (a print-only `#[ignore]`d diagnostic) became
    `tests/forward_http_outgoing_tests.rs`. It starts a fake upstream, sends a
    request whose headers pass through the real `forward_request_headers`, and
    asserts the upstream received the original `Host`, an appended
    `X-Forwarded-For`, `X-Forwarded-Proto`, and `Connection: close`, plus a 200.
  - No public API change, so no version bump.
  - Validation under `--profile uat`: `forward_http_outgoing_tests` green. No
    new clippy warnings from this change.
  - Backlog 9 "`tests/diag_client_tap.rs` is a 'TEMPORARY diagnostic'..." entry
    deleted.
- E6 (warning-free without default features):
  - `h3_local_addr` is now declared by a `cfg`-gated pair: `mut` only under the
    `quic` feature, a plain `None` binding otherwise. The `unused_mut` warning
    is gone. (`src/server.rs`.)
  - Removed a dead `let start = Instant::now()` in `ProxyHandler::serve`
    (`unused variable` warning). (`src/handler.rs`.)
  - No public API change, so no version bump.
  - Validation: `cargo check -p foundation_proxy --no-default-features`
    (dev) is warning-free; the same check under `--profile uat` has no warning
    that points at `foundation_proxy` source (only the workspace-wide
    `fullfp16` target-feature warning, tracked under foundation_ai §10).
  - Backlog 9 "The `mut` on `h3_local_addr` warns without the `h3` feature"
    entry deleted.

## Done

E1 through E6 all landed, each in its own commit. The proxy feature 15 work is
complete.

## Linux CI

`scripts/ci/linux.sh foundation_connectrpc` and `scripts/ci/linux.sh
foundation_proxy` both pass on `ewestudios/ewe-platform-ci:latest` (linux/amd64,
`--profile uat`): connectrpc 477 tests passed (2 ignored), proxy 117 tests
passed. The first run failed (exit 125) because the host disk was full (the
OrbStack virtual machine cargo volume); after the disk was cleared and the image
and volume were rebuilt, both runs are green.
