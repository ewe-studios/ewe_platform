# Spec 65 Progress: Agent RPC Serving System

_Last updated: 2026-10-22 (feature 07 design written). Read this first, then `LEARNINGS.md` ("Handoff guide"
at the top) before touching code._

## Status at a glance

| Feature | State | Where |
|---|---|---|
| 00 — foundation (wasm / Workers readiness) | 🔧 6/7 (T7 open) | PR #124 |
| 01 — agent-rpc model server | ✅ 10/10 + GGUF; ⏸ owner review round 2 open (paused, see below) | PR #126 |
| 02 — manifest installer | ⏳ 0/9 | — |
| 03 — proxy registration | ⏳ 0/8 | — |
| 04 — registrar sidecar | ⏳ 0/7 | — |
| 05 — Cloudflare | ⏳ 0/6 | — |
| 06 — wasm async streaming | ✅ 5/5 | PR #124 |
| 07 — pluggable agent-rpc (owner review round 2) | 📐 design done, decisions in, 0/27 tasks | PR #126 branch |
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
- _2026-10-21 update:_ #124 has merged; #126 now targets master and is
  **conflicted** (`foundation_ai/src/agentic/turn.rs`, `docs/bug-backlog.md`)
  after #130 (valtron async-first), #131 (async-only foundation_db stores) and
  #132 (foundation_ai on the async store traits) landed.
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

## Feature 01 — owner review round 2 (paused until the async-storage stream is done)

_Recorded 2026-10-21._ The owner asked to finish the async-storage migration
(spec 50: #130, #131, #132 merged; `db/async-callers` in progress) before
coming back to agent-rpc. The plan below is agreed and ready to start. Answers
to the first four review questions are in the PR comment
https://github.com/ewe-studios/ewe_platform/pull/126#issuecomment-6316012815.

**Guiding direction (owner):** use traits so everything is pluggable, giving
one RPC surface that runs on bare metal, Docker and Cloudflare Workers.

Work, in order:
1. **Merge master** into `65-f01-agent-rpc` (merge commit, no rebase) and adapt
   to the async APIs. These include async `StorageProvider::new`/Turso
   constructors, `Blocking<S>` only at sync boundaries, async
   `AgentSessionBuilder::build()` / `AgentSession::end()`, and whatever
   `db/async-callers` changes once it lands.
2. **`mark_seen` out of the TLS verifier.** `TrustedClientVerifier` stays a pure
   sync check, since rustls's trait is sync and the check reads the in-memory
   index. Last-seen is recorded with `.await` in the async connection path after
   the handshake, from the peer certificate, and errors are propagated.
3. **Auth moves into `foundation_auth`, built on `foundation_db`** (owner, review
   on `auth.rs` L142):
   - the CA and CSR issuance;
   - `SigningKey` / `SignedSecret` / `BootstrapSecrets`;
   - the trusted-client store;
   - the mTLS verifier.
   
   agent-rpc keeps only its wiring (enrollment service, listeners).
4. **A `CaStore` provider trait** (owner, `auth.rs` L213). It loads or generates,
   then persists, the CA and signing key through any backend: filesystem,
   memory, JSON files, Turso/libsql, D1/KV/R2. One generic impl goes over
   foundation_db's async traits, plus a filesystem impl for `from_state_dir`.
   Encrypt private keys at rest.
5. **A pluggable `ClientAuth` provider** replacing the `Auth` enum. Handlers see
   only `AuthInfo`, and it reuses `foundation_connectrpc`'s `AuthFunc` /
   `AuthzInterceptor` where they fit. Providers:
   - **Signed secret (default)**, from env with no filesystem. This is **our signed
     JSON format, not a JWT.** `SignedSecret` is base64url JSON
     `{v, alg, public_key, secret, valid_from, valid_until, signature}`, with the
     signature over a domain-separated, length-prefixed encoding. Together with
     `BootstrapPayload` (`addr`, `bootstrap_addr`, `tls_fingerprint_sha256`,
     `token`), it carries the start/end window, the signature, endpoint info, and
     the public key the other side verifies with. Keep the wire format
     backward-compatible.
     - Split verify (public key only) from sign (private key), so a host such
       as a Worker can verify with just the public key.
     - Verify the signature, the window, and single use via the store.
   - **mTLS + trusted-client store** (TLS layer, native).
   - JWT via foundation_auth: optional extra, never the default.
   - Insecure plaintext: loopback/dev only.
6. **Core/host split.** A transport-agnostic core holds:
   - the service on connectrpc's `Router`;
   - an `InstanceRegistry` trait;
   - model access behind a provider trait;
   - `ClientAuth`.
   
   The native host (TCP/TLS listeners, local models, Turso) sits on top. The
   Workers host isn't built in this PR; it gets a new feature doc under
   `features/`, covering:
   - a fetch entry into the same `Router`, speaking Connect and gRPC-Web (not
     full gRPC, which needs trailers);
   - remote model providers;
   - one Durable Object per instance;
   - D1;
   - `ClientAuth` via the signed secret (public key from config), JWT, or
     Cloudflare mTLS (`request.cf.tlsClientAuth`);
   - how a URL-based host fills `BootstrapPayload`'s endpoint fields.
7. **Model capabilities + instance kind** (owner, proto L52):
   - a `GetCapabilities` RPC returning the supported kinds (`API`, `CANDLE`,
     `GGUF`, with room for `VIDEO` / `AUDIO`; zero = `UNSPECIFIED`) and their
     devices;
   - the create-instance request carries kind and device, validated against
     capabilities, with a Connect error for unsupported kinds.
8. **Candle device** (owner, `model.rs` L61): `ModelSource::Candle` gets a
   CPU / CUDA / Metal choice like `GgufDevice`, feature-gated.

Also open from the earlier review: the Postgres question. foundation_db has no
Postgres backend; the trusted-client SQL is portable except for `?`
placeholders. It's unscheduled.

## Feature 07 — Pluggable agent RPC (design written 2026-10-22)

The owner's review round 2 on #126 is now a full design in
`features/07-pluggable-agent-rpc/`. It is written in ASD-STE100 Simplified Technical English
(STE), with Mermaid diagrams. Files:

- `feature.md`: overview, glossary, architecture, owner directions, breaking changes,
  migration, tests, task list T0 to T22, open questions.
- `start.md`: how an agent starts.
- `env.md`: side effects and the `Env` bundle in valtron's `Ctx`.
- `auth.md`: auth moved into `foundation_auth` (signed secrets, key store, certificate
  authority, trusted clients, mutual TLS, client auth providers).
- `registry-and-models.md`: model catalog, kinds and devices, instance registry, Durable
  Objects.
- `hosts.md`: native host, Worker host, web client, native client.
- `proto.md`: wire contract changes.
- `deterministic-simulation-testing.md`: deterministic simulation testing in plain words.

Owner decisions (2026-10-22), recorded in `feature.md` section 11: per-caller instance scope on
the Worker; version 2 secrets by default; private keys always encrypted at rest
(`key-protection.md`); `JwtAuth` ships; destroyed instances are archived to R2; model API keys
are Worker or Secrets Store secrets; Postgres is a separate later feature; directory migrations
are task T25 (`migrations.md`); work starts after the rename PR merges. Three small open questions
remain in `feature.md` section 12.

Next: Then task T0 (merge
`master` into this branch) starts the work. The plan above ("owner review round 2") is now
replaced by feature 07.

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

1. After the async-storage stream (spec 50) is done: work through "Feature 01 —
   owner review round 2" above on #126, then get #126 green and merged.
2. Features 02 → 05 in dependency order (02 and 03 independent; 04 needs 02 + 03;
   05 needs 00 + 01).
3. F00 T7 when credentials exist.

_Created: 2026-10-20 · Rewritten for handoff: 2026-10-21_
