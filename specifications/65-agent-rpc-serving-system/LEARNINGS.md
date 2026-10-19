# LEARNINGS — Agent RPC Serving System

_No implementation yet — this file is seeded at spec creation and updated after
each milestone/task. See `.agents/skills/specifications-management/skill.md`._

## Key finding (creation)

This system is **four single-mission components composed by a manifest and one
live handshake — not a layered stack.** The instinct to make the proxy
"understand" agent-rpc, or to have agent-rpc register itself, or to share a
manifest/secret across components, all reintroduce the coupling the design
explicitly forbids. The two hard seams are: (1) the manifest, read *only* by
installer + registrar, and (2) the registration handshake, between registrar and
proxy. Everything else is independent.

## Decision (2026-10-19)

- **agent-rpc is a per-machine control plane, not a process-per-instance.**
  Load a model once, run many threaded inference sessions, share the loaded
  model across all instances. Instances are lightweight logical state
  (an `AgentSession` + memory + provider config).
- **Two independent auth channels** — client↔agent-rpc (secret token → mTLS,
  owned by agent-rpc) and registrar↔proxy (token → mTLS, owned by registrar +
  proxy). The proxy does not see agent-rpc's client auth.
- **Only installer + registrar read the manifest.** agent-rpc and the proxy do
  not know it exists.
- **Docker delegates to `foundation_deployment_docker`** (pull-by-tag preferred,
  build as fallback). The sidecar is a deployment-time choice (1/bare-metal host
  vs per-container), not an architecture change.

## Load-bearing assumption to verify first (2026-10-19)

The whole "one model, many threads" memory model rests on `foundation_ai`'s
loaded backend (llamacpp GGUF via `gguf_provider`/`infrastructure_llama_cpp`, or
Candle via `candle_llama`) being `Send + Sync` and handable to many
`AgentSession`s. This is **not assumed** — it is the first task of `01-agent-rpc`
(a spike) and, if it fails, the fallback is a per-backend `Arc<Mutex<…>>`
serialization point rather than per-instance copies.

## Inherited gap (2026-10-19)

gRPC `grpc-status` is not yet observable end-to-end because
`foundation_connectrpc`'s H2 transport does not forward trailing HEADERS
(spec 63 T1). agent-rpc's gRPC status assertions are gated on that landing; until
then, Connect and gRPC-Web round-trips are the verifiable RPC paths.

## Corrections absorbed during design (2026-10-19)

- Do **not** frame agent-rpc as process-per-instance (defeats memory sharing).
- Do **not** say components "share" a manifest or secret — only installer +
  registrar read the manifest; the secret is per-channel, not per-system.
- The registrar owns proxy communication; agent-rpc's auth is client-facing only.

## Cloudflare research (2026-10-19, validated against the repo)

"agent-rpc on Cloudflare" is **two artifacts, not one**, and the split is forced
by the platform, not by taste:

- **Full `foundation_ai` cannot compile to a Worker.** `llamacpp`/`candle`
  backends are already `not(target_family = "wasm")`-gated
  (`src/backends/mod.rs`) and are infeasible anyway (C FFI, threads, mmap, ~128 MB,
  no raw TCP). Do not attempt to port them.
- **The agentic layer + cloud providers are nearly wasm-clean.** The only hard
  `cargo check --target wasm32-unknown-unknown` error is `getrandom` pulled by
  `ahash`'s default `runtime-rng` feature (`foundation_ai/Cargo.toml:43` → `ahash
  = "0.8"`). Secondary (verified) fixes: the three cloud providers call the
  native-only `foundation_netio::http::default_http_client()`
  (`openai_provider.rs:392`, `anthropic_messages_provider.rs:484`,
  `openai_responses_provider.rs:537`) instead of the root
  `foundation_netio::default_http_client()` (wasm-aware re-export), and
  `foundation_ai` never enables `foundation_netio/wasm-fetch`.
- **The repo already has the hard parts.** `foundation_netio::wasm::client::
  FetchHttpClient` (behind `wasm-fetch`), `foundation_http::wasm::{serve_cf,
  dispatch_cf}`, and `foundation_connectrpc::shared::Router` (wasm-compatible
  Connect/gRPC-Web handlers; H2/H3 + `ConnectRpcServe` are native-only). The one
  gap on the RPC side is `foundation_connectrpc::shared::transport::wasm` — a
  capabilities stub ("lands in F24").
- **Deployment gaps.** `foundation_deployment_cloudflare` has Workers + API v4 +
  `generated/ai` (Workers AI control plane) but **no Sandboxes/Containers**;
  `ewe_deployables::cloudflare::CloudflareWorker` uploads Worker scripts.

**Decision:** thin WASM agent orchestrator on Workers (model via Workers AI or a
remote agent-rpc); full native model-server agent-rpc on Sandboxes/Containers.
All cross-crate changes are enumerated in `00-foundation/feature.md` §"Changes
required in existing crates" (readiness) and `05-cloudflare/feature.md` (the
target itself).

## Corrections — foundation-first, not deployment-first (2026-10-19)

Two oversteps corrected during re-scoping:

- **Do not fold wasm readiness into a single "Cloudflare deployment" feature.**
  The readiness work (make `foundation_ai` + deps compile/run on wasm) is a
  **foundation** that precedes agent-rpc. It is now `00-foundation`
  (`has_fundamentals: true`), grounded in the completed
  `28-cloudflare-workers-readiness` spec (cf-valtron-counter example,
  ci-wasm-checks, miniflare harness), verified with miniflare locally and on
  Cloudflare before anything else is built.
- **`foundation_installer` must not care about Cloudflare.** Its deploy targets
  are systemd and Docker. Cloudflare deployment (Worker upload, Sandboxes/
  Containers) lives in `foundation_deployment_cloudflare` / `ewe_deployables`.
  The `cloudflare` deploy-target adapter that was added to the installer was
  removed.

## Cross-platform seams (2026-10-19)

Two substrate homes already exist — reuse them, do not invent new types:

- **HTTP substrate — `foundation_netio`.** `DynNetClient` (`Arc<dyn NetClient>`,
  combined `HttpClient + WebSocketConnector`) and the singular `HttpClient` trait
  (`Arc<dyn HttpClient>`). Use the singular trait for HTTP-only providers, the
  combined one only when WebSocket is needed. **No new `DynHttpClient` type.**
- **Non-I/O substrate — `foundation_compact`.** The central home for
  cross-platform primitives: `time` (JS `Performance.now`/`Date.now` on wasm),
  `entropy` (vendored, all targets), `rng` (ChaCha CSPRNG, no external
  rand/getrandom), `ids` (scru128), `crypto`, `send_wrapper`. The `ahash`/
  `getrandom` wasm blocker is fixed by routing `foundation_ai`'s hashing/RNG
  through `foundation_compact::rng`/`entropy` — not by an ad-hoc
  `default-features = false`.
