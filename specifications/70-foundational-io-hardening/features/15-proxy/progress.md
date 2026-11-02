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

## Next

- E2: serialise `ProxyStateStore` read-modify-write behind a mutex, or store one
  record per backend and certificate. Test
  `concurrent_backend_saves_keep_every_update`.
- E3: follow RFC 8555 in `src/acme.rs` (account `Location` as `kid`, carry
  `Replay-Nonce`, async DNS wait). Make the mock CA strict. Three tests.
- E4: move the redirect head buffer, read timeout and accept poll into the
  redirect config. Two tests.
- E5: make `tests/diag_client_tap.rs` a real `forward_http` test or delete it.
- E6: build `h3_local_addr` from a `cfg`-gated expression; verify the
  no-default-features check is warning-free.
