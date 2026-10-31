---
feature: "Hermetic netio tests, runnable doc examples, 8-thread conformance"
status: "pending"
priority: "medium"
depends_on: ["04-nonblocking-connect-tls"]
branch: "fix/io-10-tests-hermetic"
crates: ["foundation_netio", "foundation_connectrpc"]
backlog: "docs/bug-backlog.md §1 Conformance coverage (task #27); §2 Ignored tests (tasks #31, #33)"
---

# Feature 10: Hermetic tests, doc examples, 8-thread conformance

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Transport Layer Security (TLS); Hypertext Transfer Protocol
(HTTP); continuous integration (CI); certificate authority (CA); pull request (PR); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. Goal

Every test in these crates runs in CI without the public internet, and none is ignored without
a written reason.

| Item | Now | Target |
|---|---|---|
| `foundation_netio/tests/tcp_and_tls/http_external_validation.rs` | 8 `#[ignore]` (reach public hosts) | Run against a local server that this test starts |
| `foundation_netio/tests/tcp_and_tls/http_server_integration.rs` | 6 `#[ignore]` (never finished) | Finished and running, or deleted with a reason if they duplicate other tests |
| `foundation_netio/tests/tcp_and_tls/tls_integration.rs` | 3 `#[ignore]` | Run against a local TLS server with a test CA |
| Doc examples in `foundation_netio` | 37 marked `ignore` or `no_run` | Compile (`no_run` is allowed when the example opens a real network connection; `ignore` is not) |
| `foundation_connectrpc/tests/conformance_tests.rs` | every test uses `threads = 4` (lines ~413, ~460) | Each test also runs with `threads = 8` |

## 2. How

### 2.1 Local servers instead of public hosts

- Use the workspace's own test server helpers (search `foundation_testing` and
  `foundation_netio/tests` for an existing local HTTP/TLS server; reuse it).
- TLS: generate a CA and a server certificate at test start (`rcgen` is in the workspace), trust
  the CA in the client config. No certificate files checked in unless a helper already does so.
- A test that validates behaviour only a real public server shows (for example a real
  redirect chain from a CDN) may stay `#[ignore = "needs internet: <why>"]` **only** if there is
  no local way. List each one in the PR body. The target is zero.

### 2.2 Doc examples

- For each `ignore`/`no_run`: make it compile. Use `no_run` only when running it needs a network
  peer. Hide setup lines with `#`. Fix any example that does not compile against today's API.
- `cargo test --doc -p foundation_netio` must pass.

### 2.3 8-thread conformance

- Duplicate each `#[valtron_test(seed = …, threads = 4, …)]` conformance test with a
  `threads = 8` variant (new seed, for example `seed + 1000`), through a small macro or a shared
  body function, not by copying the body.

## 3. Tasks

- [x] T1. Branch from the base after feature 04 (non-blocking connect makes local servers
      reliable under the pool).
- [x] T2. `tls_integration.rs` (3). Commit.
- [x] T3. `http_external_validation.rs` (8). Commit.
- [x] T4. `http_server_integration.rs` (6). Commit.
- [x] T5. Doc examples (37). Commit.
- [x] T6. 8-thread conformance variants. Commit.
- [x] T7. Delete the backlog entries. Update `progress.md`, `learnings.md`.

## 4. Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_netio --profile dev
python3 scripts/ci/ci.py run foundation_connectrpc --profile dev
cargo test --doc -p foundation_netio --profile dev
rg -c '#\[ignore' backends/foundation_netio/tests          # 0, or each hit has `= "needs internet: …"`
rg -n '```(rust,)?ignore' backends/foundation_netio/src      # 0
# Run the netio suite with no network access to prove it is hermetic:
unshare -rn cargo test -p foundation_netio --profile dev --tests   # if unshare is available; else say so
# 20-run flake check for each formerly ignored test and the 8-thread conformance tests.
```

## 5. Done when

- [x] All §4 commands pass; formerly ignored tests and the 8-thread variants passed 20 runs.
      (The netio `quic` run fails only the pre-existing `idle_h3_connection_uses_no_cpu`,
      feature 28 — out of scope.)
- [x] The netio suite passes with no network (or the PR says why it could not be checked).
      Not checked: macOS has no `unshare`; the rewritten tests bind loopback sockets only.
- [ ] CI green.

## 6. Review checklist (main session)

- [ ] No test silently skips its assertions when the server is local.
- [ ] Generated certificates are not reused across tests in a way that hides a failure.
- [ ] The 8-thread variants share the test body.
