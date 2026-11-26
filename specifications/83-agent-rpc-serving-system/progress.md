# Spec 83 Progress: Agent RPC

_Renamed from spec 65 to spec 83 on 2026-11-14, after specs 80 and 81 became its prerequisites._

_Last updated: 2026-11-15 (the fetch transport moved to spec 85; the container control and the
deployment stay in spec 77; feature 06 takes spec 77's `CloudflarePlatform` wrapper, and
`foundation_context` gets no change). Read this
first, then `LEARNINGS.md` (the handoff guide at the top) before touching code._

## Status at a glance

| Feature | State | Where |
|---|---|---|
| 00 — WASM and Workers readiness | 🔧 6/7 (T7 needs Cloudflare credentials) | PR #124 (merged) |
| 01 — agent RPC model server | ✅ 10/10 + GGUF, landed as #126 | `bfff3326d` on master |
| 02 — WASM lazy streaming | ✅ 5/5 | PR #124 (merged) |
| 03 — agent RPC core | ✅ 4/4 | branch `spec83/f03-agent-rpc-core` |
| 04 — native host and DST | 🔎 T17 and T18 done; pull request #909 open. The DST harness uses a scripted registry until #908 part (b) lands | `c9e6f1c85` |
| 05 — Worker host and web client | 🔎 T20, T21, T22 and T26 done; pull request #990 | `973562c5d` |
| 06 — Cloudflare host | ⏳ 0/3 (T1 and T2 replaced by 05; T3 moved to spec 77; takes `&CloudflarePlatform`) | — |
| 07 — pluggable agent RPC landing | ✅ landed (#126 on master); T0 complete | — |
| 08 — key protection at rest | 🔎 K1 to K6 done, the `keys` commands wired; pull request #993 | `b5cddb07b` |

## Feature 04 progress (2026-11-27)

Task T17 is done on branch `spec83/f04-native-host-and-dst`: the native host modules
(`host/native/{mod,config,security,listener,catalog,client}.rs`), two mTLS listeners, the
operator keys with no key generation, and the tests `native_{config,e2e,revocation,shutdown}_tests.rs`.
`foundation_http` `ServerConfig` derives `Clone`.

Task T18 is complete: `foundation_core` `valtron::sim`, `foundation_connectrpc`
`MemNetwork`/`MemTransport`, `foundation_db` `FaultyDocumentStore`, and the new package
`foundation_agent_rpc_dst` with `ScriptedCatalog`, `TestWorld` and the 32-seed determinism
test. The crate `foundation_agent_rpc` gains a `multi` feature, in `default`, so the harness
gets the single-threaded engine.

Issues #903 to #907 are closed by the change. #908 is split: part (a), the native build
without `multi`, is fixed; part (b), the `TestWorld` on the real registry, waits on a
cooperative event stream in `foundation_ai` (`agentic/turn.rs`), and the harness uses a
scripted registry until then. The determinism guarantee holds; the registry code is not
under test.

## One combined pull request (2026-11-27)

Features 04, 05, 06 and 08 are in pull request #1009 together with spec 77. The earlier
separate pull requests are closed. The branch is `integration/spec77-spec83`.

## Branches and PRs

- **#124** (`worktree-65-agent-rpc-serving-system`) merged: feature 02 (old 06), feature 00
  T1 to T6, and spec 68.
- **#126** (`65-f01-agent-rpc`) merged: feature 01 and the master merge of 2026-11-13. The
  branch content equals master except `Cargo.lock`; feature 07 T0 is therefore complete.
- Landed on master: #123 (release-PR job on a branch), #127 (releases on stable; Cranelift only
  via mise), #128 (llama.cpp CUDA links NCCL), and #130 to #132 (valtron async-first, async
  foundation_db stores, foundation_ai on the async store traits).

## History that moved out of this spec

The 2026-11-14 split moved features and their history:

| Old feature | New spec | History |
|---|---|---|
| 02 manifest installer | spec 79 | `../completed/79-manifest-installer/progress.md` |
| 03 proxy registration | spec 82 feature 00 | `../completed/82-proxy-registration-and-registrar/progress.md` |
| 04 registrar sidecar | spec 82 feature 01 | same file |
| 08 env and side effects | spec 84 (`Env`, `IdSource`), spec 80 (transport and migrations) | `../completed/84-context-injection/progress.md`; `../80-platform-prerequisites/progress.md` |
| 09 transport prerequisites | spec 80 feature 00 | `../80-platform-prerequisites/progress.md` |
| 10 migration groups | spec 80 feature 01 | same file |
| 11 foundation auth | spec 81 feature 00 | `../completed/81-foundation-auth/progress.md` |

The design files moved with their features. Spec 78 owns the manifest contract.

The same pass fixed stale references that the renumber left: feature 06 now names feature 05
for the Worker host, feature 05 names feature 06 for Cloudflare deployment, and feature 01
names spec 82 feature 00 for the registrar.

## The Cloudflare merge (2026-11-14)

Feature 06 and spec 77 said opposite things about Cloudflare deployment and about the
container surface. Feature 06 put deployment in `foundation_deployment_cloudflare` and added
a `ContainersDeploy` surface. Spec 77 feature 08 deployed by hand with `npx wrangler` and
kept its container API private to the Durable Object. The owner merged both into spec 77
(spec 77 decision D15).

| Item | Before | Now |
|---|---|---|
| Cloudflare deployment | Feature 06 here | Spec 77 feature 08: `CloudflareDeployer`, `DeploymentPlan`, `FoundationCloudflareDeployer` (primary), `WranglerCloudflareDeployer` (fallback) |
| Container control | Feature 06 here (`ContainersDeploy`) | Spec 77 feature 00 section 6.13 and feature 07 section 6.4.1: `ContainerControl`, `ContainerHandle`, `DoContainerOps`, `DoContainerClient` |
| Feature 06 T3 | Add a Sandboxes/Containers surface here | Moved to spec 77 feature 08 |
| Feature 06 name | "Cloudflare Deployment" | "Cloudflare Host" |
| Feature 06 tasks | 6 | 3 (T4, T5, T6) |
| Feature 06 files in `foundation_deployment_cloudflare` | `src/`, `tests/containers_tests.rs` | None. The crate is used, not changed. |

Feature 06 keeps its agent-rpc work: the thin wasm orchestrator on Workers (built on feature
05 T21), the F24 fetch transport for a remote native server (built on feature 05 T19), the
agent-rpc `DeploymentPlan`, and the native server in a Cloudflare container.

Its `depends_on` now includes spec 77.

## Feature 01 — agent RPC model server

| Task | Status | Commit |
|---|---|---|
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
| Docker `buildkit` buffa pin | ✅ | `47af8d9ba` |
| GGUF `ModelSource` + llama.cpp shareability tests | ✅ | `9f10b5011` |
| Signed, expiring bootstrap secret + SQL trusted-client store | ✅ | in #126 |

Tests: `PROTOC=~/.local/share/mise/installs/protoc/36.2/bin/protoc cargo test --profile uat -p
foundation_agent_rpc` (candle default), and
`... --no-default-features --features gguf --test gguf_share_tests`.

The signed-secret code moved to spec 81. Feature 03 uses `foundation_auth` for it. The old
feature 01 wiring (enrollment service, listeners) stays here.

## Feature 00 — wasm readiness

| Task | Status |
|---|---|
| T1 — `foundation_ai` wasm-clean (getrandom/ahash removed) | ✅ |
| T2 — dependency-crate wasm audit | ✅ |
| T3 — wasm32 CI compile gate | ✅ |
| T4 — Workers AI provider (SSE without `[DONE]`) | ✅ |
| T5 — examples `cf-agent-loop`, `cf-cloud-provider` | ✅ |
| T6 — both examples green under Miniflare | ✅ |
| T7 — `wrangler deploy` + live smoke test | ⏳ needs a Cloudflare account and `OPENROUTER_API_KEY` |

## Feature 02 — WASM lazy streaming (old 06)

| Task | Status | Commit |
|---|---|---|
| T1 — parking `into_future_stream` bridge + pure-lens rename | ✅ | `59c4ef14d` |
| T2 — `run_future` → `block_on_collect`, single-threaded `FutureIterator` fix | ✅ | `35d27d89f` |
| T3 — lazy wasm `send_sse` + `SseProgress::Failed` | ✅ | `4ce7ad6e5` |
| T4 — `Failed` → `ModelState::Error`, wasm `sleep` no-op | ✅ | `89db93212` |
| T5 — `cf-cloud-provider` streams live under `wrangler dev --local` | ✅ | `0e33260b7`, `f69a727eb` |

## Feature 07 — pluggable agent RPC landing

The old feature 07 held only task T0: merge master into `65-f01-agent-rpc` and adapt to the async
APIs. It is complete; #126 is on master and the branch content equals master except `Cargo.lock`.
The shared design of the pluggable server is `agent-rpc-architecture.md`.

## Feature 03 — agent RPC core

The transport-agnostic core is in `src/core/`. Branch `spec83/f03-agent-rpc-core`.

| Task | Status | What landed |
|---|---|---|
| T13 — new `agent.proto`, selection rules | ✅ | `GetCapabilities`, `RefreshAccess`, model kinds and devices, paging, the `BootstrapPayload` URL form; `ModelRequest::from_wire`, `select`, `InstanceSpec::from_wire`; `core::bootstrap::decode`; the legacy feature-01 host adapted to the new wire fields |
| T14 — model catalog | ✅ | `ModelCatalog`, `ModelCapabilities`, `ModelHandle` with kind and device, `RemoteCatalog`; the load-once `LocalCatalog` stays with T17 |
| T15 — instance registry | ✅ | `InstanceRegistry` trait, `Scope`, `ScopePolicy`, `Caller`, `RegistryConfig`, `TurnLimits`, `LocalInstanceRegistry`, `SessionStores`, `guarded_turn` with the idle timeout |
| T16 — service and enrollment | ✅ | `core::service` handlers over the traits, `GetCapabilities`; `core::enroll` with client keys, CSRs, server proofs and `RefreshAccess` |

The enrollment design assumed client-key primitives that spec 81 did not ship. This
change adds them to `foundation_auth`: `ClientPublicKey` (`secret`), `ClientKeyRecord`,
three `TrustStore` methods (`record_client_key`, `client_key`, `consume_refresh_nonce`)
and the `TrustedClients` wrappers. `foundation_auth` is 0.0.16.

Tests: `model_selection_tests`, `proto_compat_tests`, `model_catalog_tests`,
`registry_contract_tests`, `handler_tests`, `enroll_tests`. The legacy certificate service
keeps its typed refusal of `RefreshAccess`; feature 04 replaces that host.

## Current work and next steps

1. **Feature 04 (native host and DST)** is next. It builds `host::native` (including
   `LocalCatalog`) on this core. When it is done, the proof of concept works.
2. **Feature 05 (Worker host and web client)** follows 03.
3. **Feature 06 (Cloudflare)** follows 05. Its old T1 and T2 are replaced by feature 05 T19 and
   T21; T3 to T6 stay here.
4. **Feature 08 (key protection)** is last.
5. **Feature 00 T7** when Cloudflare credentials exist.

## Deliberately not done

- **gRPC over TLS.** There is no h2-over-TLS client; `H2Transport` is h2c only. Tracked in
  `docs/bug-backlog.md`.
- **mTLS through the proxy.** A proxy that ends TLS cannot pass a client certificate. A direct
  connection uses mTLS; through the proxy, the client uses the signed secret. SNI passthrough is
  spec 82 open question 3.
- **Wasm runtime suites in CI.** The owned and bindgen suites run by hand (spec 68).
- **The `foundation_proxy` end-to-end intermittent hang.** Branch `fix/proxy-e2e-hang` exists,
  unfinished. See `LEARNINGS.md`.

## Merge of master into `65-f01-agent-rpc` (2026-11-13)

The merge is resolved and landed. What it taught:

- `foundation_connectrpc::Ctx` is now `RpcCtx { ctx, request }`.
- `foundation_db` uses `QueryStore`, and the async methods lost their `_async` suffix.
- `foundation_http` split `ServerApp` and `Serve` into `native::{app,serve}` and takes
  `&Arc<ShutdownSignal>`.
- `foundation_ai` is async all the way; `AgentSession` methods return `'static` futures.
- The merge left two `[packages.foundation_agent_rpc]` tables in `ci/packages.toml`; a second
  table for one key is a TOML error. It is fixed.
- CI denies clippy warnings per package. Twelve warnings were fixed.
- The `gguf` test entry failed twice: first on the `LlamaBackends` constructor rename, then on
  worker starvation (#551). The call deadline is now 120 s.

## 2026-11-14 split

The owner split the spec into five focused specifications and narrowed this one to agent RPC. No
code changed. See `LEARNINGS.md` "Spec split (2026-11-14)" for the reasons and the boundary
rules.

## Documentation corrections (2026-11-15)

No code changed.

- **`SealFuture` (feature 08).** The alias was `Send` on native and `!Send` on wasm, so the
  `KeySealer` signature changed shape per target. It is one `Send` alias on every target now,
  and a wasm sealer wraps its future in `foundation_compact::SendWrapper`.
- **Architecture section 3.2.** `foundation_core` does not own `Env` or `Ctx`; both are in
  `foundation_context`, which now has its own row. `foundation_auth` is no longer called
  target-neutral: `client_auth::cloudflare` is wasm only and `client_auth::mtls` is native only.
- Feature 06 takes `&CloudflarePlatform`. The wrapper carries no HTTP client: a call holds a
  `DynNetClient`, and an RPC call is `H1Transport` over it (spec 77 section 6.0).

Issue #556 records the set across the split specifications. It is closed.

## Windows gguf link (2026-11-24)

`3918d563e` fixes the link of the `gguf` test on `x86_64-pc-windows-msvc` (#772).

- `patches/esaxx-rs` vendors `esaxx-rs` 0.1.10 without its forced static MSVC
  runtime. The upstream note is in `patches/esaxx-rs/UPSTREAM.md`.
- `infrastructure/llama-bindings` no longer forces llama.cpp to the static
  runtime; the `msvc_static_runtime.cmake` fragment is deleted.
- Verified locally: the patched `esaxx-rs` builds on macOS, and
  `cargo check -p infrastructure_llama_bindings` passes. CI run `45340309029`
  passes its `Windows 1/1` shard, so the gguf test links and runs on Windows.
  #772 is closed.
