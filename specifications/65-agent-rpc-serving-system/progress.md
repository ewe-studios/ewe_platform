# Spec 65 Progress: Agent RPC Serving System

_Last updated: 2026-10-21. Read this first, then `LEARNINGS.md` ("Handoff guide"
at the top) before touching code._

## Status at a glance

| Feature | State | Where |
|---|---|---|
| 00 — foundation (wasm / Workers readiness) | 🔧 6/7 (T7 open) | PR #124 |
| 01 — agent-rpc model server | ✅ 10/10 + GGUF; 🔧 bootstrap-secret redesign in progress | PR #126 |
| 02 — manifest installer | ⏳ 0/9 | — |
| 03 — proxy registration | ⏳ 0/8 | — |
| 04 — registrar sidecar | ⏳ 0/7 | — |
| 05 — Cloudflare | ⏳ 0/6 | — |
| 06 — wasm async streaming | ✅ 5/5 | PR #124 |
| (spec 68 — valtron wasm runtime tests) | ✅ owned path 10/10, bindgen path 5/5 | PR #124 (#125 merged into it) |

## Branches and PRs (how the work is stacked)

```
master
 └─ #124  worktree-65-agent-rpc-serving-system   (F06, F00 T1–T6, spec 68)
     │     includes #125 (compact host-ABI backend, merged into this branch)
     └─ #126  65-f01-agent-rpc                     (F01 + GGUF + bootstrap redesign)
```

- **#124** is open against master. It has master merged in (NCCL fix #128 and
  stable releases #127 included). Review asks addressed: async model-metadata
  lookup on wasm (`4eedff5e0`), runtime timer disposal (`47792e61e`).
- **#126** targets #124's branch. When #124 merges, retarget #126 to master.
- Landed on master already: #123 (release-PR job on a branch), #127 (releases on
  stable; Cranelift only via mise env), #128 (llama.cpp CUDA links NCCL).

## Feature 01 — Agent RPC Model Server (`65-f01-agent-rpc`)

| Task | Status | Commit |
|------|--------|--------|
| T1 — shareability spike (candle: shared weights, per-generation KV) | ✅ | `5ab440281` |
| T2 — crate scaffold, `AgentRpcError` ↔ wire codes | ✅ | `82141d432` |
| T3 — `ModelHandle` (load once, `Arc`) | ✅ | `9f126086c` |
| T4 — `InstanceRegistry` | ✅ | `5d53b2788` |
| T5 — generated `AgentService` (+ codegen fixes) | ✅ | `319b71153` |
| T6 — unary round-trip over Connect | ✅ | `563a146da` |
| T7 — streaming round-trip, proven unbuffered | ✅ | `c46ee3e4b` |
| T8 — client auth (token → client cert → mTLS) | ✅ | `ca7b4d923` |
| T9 — bootstrap payload (base64url + QR) | ✅ | `db79cfcdd` |
| T10 — Connect / gRPC / gRPC-Web end to end | ✅ | `f734645e3` |
| docker `buildkit` buffa pin (found during review) | ✅ | `47af8d9ba` |
| GGUF `ModelSource` + llama.cpp shareability tests | ✅ | `9f10b5011` |
| Signed, expiring bootstrap secret + SQL trusted-client store | 🔧 in progress | see "Current work" |

Tests: `PROTOC=~/.local/share/mise/installs/protoc/36.2/bin/protoc cargo test --profile uat -p foundation_agent_rpc`
(candle default) and `... --no-default-features --features gguf --test gguf_share_tests`.

### Current work: bootstrap secret redesign (owner's design, 2026-10-21)

Replaces T8/T9's random single-use token with a **server-signed, time-boxed
secret**, and persists enrolled clients:

1. **Signing key** (used only for bootstrap secrets): Ed25519 by default. Loaded
   from the path in an env var when set; otherwise from a default location,
   generated there if missing. A lost key is regenerated on startup, which
   invalidates every outstanding secret (by design).
2. **Secret** = random bytes + `valid_from`/`valid_until`, signed by that key.
   Shared form: base64 of a JSON dict `{public_key, signature, secret,
   valid_from, valid_until}` (the QR code embeds the same string). Verification
   rejects a bad signature, a key mismatch, or a time outside the window.
3. **Trusted clients** that complete enrollment + mTLS are stored through
   `foundation_db`'s SQL storage (SQLite by default; any foundation_db SQL
   backend). The server consults the store on connect; clients can be revoked.

**State at pause (WIP commit on `65-f01-agent-rpc`):** code written and
`cargo build` clean; the new/updated tests are written but **not yet run**
(the run was interrupted by a full disk, then work paused for #124 review).
Next agent: run `cargo test --profile uat -p foundation_agent_rpc --test
secret_tests --test trust_tests --test auth_tests --test bootstrap_tests
--test auth_log_tests --test error_tests` (with `PROTOC` set) and fix what
fails; then `-p foundation_db --test turso_storage_tests` (passed: 7/7).

What landed:
- `src/secret.rs` — `SigningKey` (ring; Ed25519 generated, operator RSA
  PKCS#8 accepted), `SigningKeyConfig` (env `EWE_AGENT_RPC_SIGNING_KEY` =
  inline PEM or path, else `<state>/bootstrap-signing-key.pem`, generated
  0600 when missing; corrupt file = error, never overwritten),
  `SignedSecret` (base64url JSON `{v, alg, public_key, secret, valid_from,
  valid_until, signature}`; signature over a domain-separated,
  length-prefixed encoding, not the JSON), `BootstrapSecrets` (issue /
  verify; default validity 24 h = kubeadm token TTL).
- `src/trust.rs` — `TrustedClientStore` over any `foundation_db`
  `AsyncQueryStore` (`open_sqlite` = Turso/SQLite): clients table +
  per-secret use counts (single statement upsert, pruned after expiry);
  in-memory index for the sync TLS verifier.
- `src/auth.rs` — `BootstrapToken` removed; `MtlsConfig::new(ca, secrets,
  trusted)` / `from_state_dir(dir).await` (CA persisted as
  `ca-cert.pem`/`ca-key.pem`); async `EnrollmentService::enroll_client`
  (verify → sign CSR → spend use → record); `TrustedClientVerifier` wraps
  WebPKI and refuses certs not in the store / revoked / expired, writing
  last-seen in the background via `valtron::send`.
- `src/server.rs` — secure mode is `with_mtls(config)` alone;
  `RunningServer::bootstrap_payload()` issues a fresh secret.
- `src/client.rs` — `connect_with_payload` checks signature + window locally.
- `foundation_db` — async `TursoStorage::open` (no block-on inside tasks).
- Tests: `secret_tests`, `trust_tests` (new); `auth_tests`,
  `bootstrap_tests`, `auth_log_tests` rewritten. RSA fixture in
  `tests/fixtures/test-only-rsa-signing-key.pem`.

Known gaps to record in `docs/bug-backlog.md` when resuming: the server
certificate is regenerated on every `serve`, so outstanding payloads stop
working after a restart (pinned fingerprint changes) — persist the server
identity in the state dir; revocation refuses new handshakes but does not
drop live keep-alive connections.

## Feature 00 — foundation (`worktree-65-agent-rpc-serving-system`)

| Task | Status |
|------|--------|
| T1 — `foundation_ai` wasm-clean (getrandom/ahash removed) | ✅ |
| T2 — dependency-crate wasm audit | ✅ |
| T3 — wasm32 CI compile gate | ✅ |
| T4 — Workers AI provider (SSE without `[DONE]`) | ✅ |
| T5 — examples `cf-agent-loop`, `cf-cloud-provider` | ✅ |
| T6 — both examples green under Miniflare (`foundation_toolings/tests/cf_examples.rs`, `--features cf-worker-tests`) | ✅ |
| T7 — `wrangler deploy` + live smoke test | ⏳ needs a Cloudflare account + `OPENROUTER_API_KEY` |

## Feature 06 — WASM async streaming

| Task | Status | Commit |
|------|--------|--------|
| T1 — parking `into_future_stream` bridge + pure-lens rename | ✅ | `59c4ef14d` |
| T2 — `run_future` → `block_on_collect`, single-threaded `FutureIterator` fix | ✅ | `35d27d89f` |
| T3 — lazy wasm `send_sse` + `SseProgress::Failed` | ✅ | `4ce7ad6e5` |
| T4 — `Failed` → `ModelState::Error`, wasm `sleep` no-op | ✅ | `89db93212` |
| T5 — `cf-cloud-provider` streams live under `wrangler dev --local` | ✅ | `0e33260b7`, `f69a727eb` |

The two T5 hangs (sync `get_model` lookup blocking the isolate; `Wait`
busy-looping) and their fixes are in LEARNINGS "06 T5".

## Spec 68 — valtron wasm runtime tests (lives on #124)

- **Bindgen tier** (`js-wasmbindgen`, what Workers run): `wasm-testbed test
  bindgen-deno ./backends/foundation_core --features js-wasmbindgen,std
  --filter wasm_js_yield_integration` → 5/5.
- **Owned tier** (`js-foundation-wasm`, no wasm-bindgen): `wasm-testbed deno
  backends/foundation_core/integration/wasm_tests` → 10/10. Unblocked by:
  foundation_compact's `host-abi` backend; a `crypto.getRandomValues` shim in the
  embedded Deno; the `#[wasm_test]` runner disposing each case's timers.
- Not done: browser run (Playwright); these suites are not in CI yet (backlog).

## What is deliberately not done / open

- F00 T7 (live deploy) — needs credentials.
- F01 gaps (backlog + LEARNINGS): gRPC over TLS (no h2-over-TLS client); valtron
  worker starvation by blocking h1 serve (run agent-rpc with ≥ 8 workers);
  mTLS through the proxy needs passthrough (F03).
- Wasm runtime suites not in CI; `foundation_proxy` e2e intermittent hang
  (agent branch `fix/proxy-e2e-hang` exists, unfinished — see LEARNINGS).

## Next steps (in order)

1. Finish the bootstrap-secret redesign on #126 (above), then get #124 and #126
   merged (retarget #126 to master after #124).
2. Features 02 → 05 in dependency order (02 and 03 independent; 04 needs 02 + 03;
   05 needs 00 + 01).
3. F00 T7 when credentials exist.

_Created: 2026-10-20 · Rewritten for handoff: 2026-10-21_
