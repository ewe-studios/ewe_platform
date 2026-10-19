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
`AgentSession`s. This is **not assumed** — it is the first task of `00-agent-rpc`
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
