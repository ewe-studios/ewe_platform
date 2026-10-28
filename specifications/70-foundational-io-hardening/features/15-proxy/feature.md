---
feature: "foundation_proxy: health probe leak, state race, ACME account flow, redirect limits, diagnostic test, warning"
status: "pending"
priority: "high"
depends_on: ["04-nonblocking-connect-tls", "13-deployment-platform"]
branch: "fix/io-15-proxy"
crates: ["foundation_proxy", "foundation_connectrpc"]
backlog: "docs/bug-backlog.md §9 (7 entries; the Docker-helper entry is done in feature 13)"
---

# Feature 15: `foundation_proxy`

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Automatic Certificate Management Environment (ACME); Request for
Comments (RFC); certificate authority (CA); Domain Name System (DNS); Hypertext Transfer
Protocol version 2 (HTTP/2); Transport Layer Security (TLS); pull request (PR); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4. One
commit per entry.

## T0. Check that every entry is still open (table in the PR body)

## E1. The gRPC health probe leaks a thread and an HTTP/2 connection

- **Where:** `src/health.rs` `probe_grpc_health` drives `grpc_health_check` on a throwaway
  thread with a `recv_timeout` backstop. When the backstop trips, the inner `block_on_future`
  and its `H2Transport` pump task are abandoned.
- **Fix:** give `H2Transport` (`foundation_connectrpc`) a read / keepalive timeout on its config,
  so its pump ends on its own. Run the probe as an async call with that timeout (no extra
  thread). The `sleep_async` timer bug was fixed in PR #160, so the backstop is not needed.
- **Tests:** `health_probe_of_silent_backend_times_out_and_leaks_nothing`: a backend that
  accepts and never answers; run 20 probes; assert each ends at the timeout, and the thread count
  (`/proc/self/status` `Threads:`) and open fd count return to the start value.

## E2. Backend-state persistence can lose an update

- **Where:** `src/persistence.rs` `ProxyStateStore::save_backend_state` / `save_tls_cert` do
  load-change-save of the whole state; control-socket connections run on separate threads.
- **Fix:** one record per backend (and per certificate) in the store, so each save writes only its
  own record. If the store format must stay one document, serialise the read-modify-write behind
  a mutex in `ProxyStateStore` and say why.
- **Test:** `concurrent_backend_saves_keep_every_update`: 16 threads each save a different
  backend 50 times; assert all 16 final states are present.

## E3. The ACME client does not follow RFC 8555's account flow

- **Where:** `src/acme.rs`.
- **Problems:** `create_account` returns the newAccount directory URL, not the account URL from
  the `Location` header; `provision` discards it, so `key_id` is never set and later requests are
  signed with `jwk`, not `kid` (RFC 8555 §6.2); a fresh nonce is fetched per request instead of
  using each answer's `Replay-Nonce`; DNS wait is `std::thread::sleep(5s)` inside an `async fn`.
- **Fix:** use `Location` as the account URL and `kid`; carry `Replay-Nonce` from each answer (a
  new nonce request only after `badNonce`); poll the challenge status with an async sleep under a
  configured timeout and interval.
- **Tests:** make the mock CA in `tests/acme_provisioning_tests.rs` strict: it sends `Location`,
  rejects `jwk` after account creation, rejects a reused nonce, and counts nonce requests. Tests:
  `acme_uses_kid_after_account_creation`, `acme_reuses_replay_nonce`,
  `acme_challenge_wait_does_not_block_the_executor` (a ticker task keeps ticking during the wait).

## E4. The SSL redirect listener hard-codes its limits

- **Where:** `src/server.rs` `ssl_redirect_loop`: 4096-byte head buffer, 2 s read timeout, 10 ms
  accept poll; a head longer than one read is cut.
- **Fix:** fields on the redirect config with documented defaults (head size 8 KiB as nginx's
  `large_client_header_buffers` per-line default; read timeout 60 s as nginx
  `client_header_timeout`; say if you keep 2 s and why). Read until the end of the head or the
  limit. Accept on readiness (reuse feature 05's accept wait), not a 10 ms poll.
- **Tests:** `redirect_handles_head_split_over_reads`; `redirect_rejects_head_over_limit` (431).

## E5. `tests/diag_client_tap.rs` asserts nothing

- **Fix:** make it a real test of `forward_http`'s outgoing request: assert `Host`,
  `X-Forwarded-For`, `X-Forwarded-Proto`, `Connection` at the fake upstream and the 200 status.
  Remove `#[ignore]`. If `proxy_integration_tests.rs` already covers this exactly, delete the
  file and say which test covers it.

## E6. `mut h3_local_addr` warns without the `h3` feature

- **Fix:** build it from a `cfg`-gated expression. **Verify:** `cargo check -p foundation_proxy --no-default-features` has no warning.

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_proxy --profile dev
python3 scripts/ci/ci.py run foundation_connectrpc --profile dev
cargo check -p foundation_proxy --no-default-features --all-targets --profile dev
cargo check --workspace --all-targets --profile dev
```

## Done when

- [ ] T0 table; every open entry fixed; backlog entries deleted.
- [ ] New tests fail on the base code and pass now; 20-run flake check passes.
- [ ] No `thread::sleep` in `src/acme.rs`; no throwaway probe thread in `src/health.rs`.
- [ ] CI green.

## Review checklist (main session)

- [ ] ACME flow checked against RFC 8555 §6.2 and §7.2 (nonces) by reading them.
- [ ] The state store change keeps old state files loadable (a migration or a read of the old shape).
