# Progress — Spec 54: foundation_deployment_docker

**Last updated:** 2026-07-12

## Features

| # | Feature | Status | Depends on | Effort |
|---|---------|--------|------------|--------|
| 00 | SendSafeBody Sync | ✅ Complete | — | Small |
| 01 | DynNetClient + PreparedRequestBuilder surface (netio/core primitives) | ✅ Complete | 00 | Medium |
| 02 | Unix socket transport | ✅ Complete | 01 | Small |
| 03 | Generator async fn codegen + regenerate deployment crates | ✅ Complete¹ | 01 | Large |
| 04 | Docker type replication via gen_api | ✅ Complete | 02, 03 | Large |
| 05 | Streaming endpoint handling | ✅ Complete | 01, 04 | Small |
| 06 | BuildKit via connectrpc (P2) | 🟡 In progress | 02, 04 | Large |

> **2026-07-14 validation + HTTP parity closed:** The earlier "bollard parity
> complete" state **did not compile** with `--features docker,buildkit` (a
> premature `}` in `client/images.rs` orphaned three methods; `build_prune` had
> drifted from the regenerated `BuildPruneArgs`). Repaired. Added the 4 remaining
> bollard-0.21 HTTP methods that had generated fns but no ergonomic wrapper:
> `update_container`, `upload_to_container`, `get_container_archive_info`,
> `export_images`. **Full HTTP surface now validated: 99 tests pass** against real
> Docker 29.5.1 (`--profile uat -- --test-threads=1`). Coverage vs bollard 0.21 +
> Docker Engine API v1.53 (107 operationIds): the `generated/` layer has 105 raw
> `*_request` fns (full API incl. P3 domains); the ergonomic `client/` wrapper
> covers **all P0–P1** (containers/images/networks/volumes/system/exec) + extras.
> Intentional wrapper gaps: `attach_container_websocket` (niche WS) and the
> P3-deferred domains (swarm/service/node/task/secret/config/plugin — generated
> fns exist, wrappers deferred per requirements).

> **2026-07-14 BuildKit (Feature 06) — real proto types landed, Session sidecar
> remaining:** Vendored the Control-service proto graph (control/worker/ops/policy
> + google/rpc/status) under `specs/buildkit/`; `build.rs` runs `buffa-build`
> (protoc + buffa-codegen) under the `buildkit` feature to emit real
> `buffa::Message` types (the same `buffa` runtime our `foundation_connectrpc`
> ProtoCodec uses). `BuildKitClient` now uses `ProcedureCodecs::defaults()`
> (proto + json) with typed `info()` (unary), `solve()` (unary — corrected from
> the scaffold's wrong "bidi"), and `status()` (server-stream) over
> `foundation_connectrpc` `Client<Req,Res>` + `H1Transport` on the Unix socket.
> Generated types round-trip on both proto and JSON wires (5 tests).
> **Remaining for end-to-end local Dockerfile builds:** the `Session` bidi
> sidecar — buildkitd dials *back* into a client-run gRPC server (FileSync/Auth/
> Secrets/SSH) over the Session stream to pull the build context. That needs
> (a) bridging a connectrpc bidi stream into a served `Connection` so our
> `HttpServer` can serve H2/gRPC over it, and (b) the fsutil/filesync proto +
> `DiffCopy`. Large; tracked as the open part of Feature 06.

> **2026-07-12 restructure:** Feature 01 split into **01** (foundation HTTP
> primitives — `foundation_core`/`foundation_netio`/`foundation_connectrpc`) and a
> new **03** (generator emit + regeneration), implementing
> [Decision 05](decisions/05-valtron-task-iterator.md). The generator update
> **must land before Docker generation (Feature 04)**. Former 03/04/05 renumbered
> to 04/05/06. Generated code shape is **`async fn`** (owner-chosen).

## Decision docs

| # | Title | Status |
|---|-------|--------|
| 01 | No bollard dependency | ✅ Written |
| 02 | Docker Engine API spec source (local moby checkout) | ✅ Written |
| 03 | Type replication strategy (gen_api + hand-refine) | ✅ Written |
| 04 | HTTP via DynNetClient + PreparedRequestBuilder | ✅ Written |
| 05 | Valtron TaskIterator → async fn | ✅ Written |
| 06 | BuildKit via foundation_connectrpc | ✅ Written |
| 07 | Unix socket transport (~30 lines) | ✅ Written |
| 08 | Streaming responses (split_exchange) | ✅ Written |
| 09 | SendSafeBody Sync → landed as Feature 00 | ✅ Written |

## Implementation order

### Phase 0: Foundation (Feature 00, 01, 02, 03)
- [x] Feature 00: Make `SendSafeBody: Sync` (~3 files, + Sync on one type alias)
- [x] Feature 01: netio/core HTTP primitives (Uri structured `Query` w/ lossless raw cache, PreparedRequestBuilder query+send+re-export, `build()`→DynNetClient + H1Transport, body_reader split_exchange/send_and_split/collect_exchange, RequestIntro deprecated)
- [x] Feature 02: Unix socket transport in foundation_netio (Connection::connect_unix, ClientConfig.unix_socket, HttpClientBuilder::unix_socket, routing in send + open_exchange paths; functional UnixListener test)
- [x] Feature 03: Generator emits `async fn` + complete type collection; cloudflare regenerated/migrated/green (172→0 errors, 12 tests). ¹Part H regeneration of stripe/supabase/neon/planetscale/prisma/flyio **deferred** (owner-approved) — those are empty skeletal stubs (no deps/lib/features, 0 dependents); scaffolding them is out of scope. CLI extended so they *can* be regenerated once built out.

### Phase 1: Core Docker API (Feature 04, 05)
- [x] Feature 04: Vendor spec, run gen_api, generate types + async fn
- [x] Feature 04: Hand-write `DockerClient` (Unix socket, configurable base_url, version negotiation)
- [x] Feature 04: Ergonomic client wrappers across containers/images/networks/volumes/system/exec (bollard-0.21 HTTP parity for P0–P1 + extras)
- [x] Feature 05: Streaming endpoints (logs, events, stats, pull, push, build) + `LogFrameDecoder` + `JsonLineDecoder`

### Phase 2: Network + Volume (Feature 04 continued)
- [x] Generated + wrapped network/volume types (create/inspect/list/delete/prune/connect/disconnect/update)

### Phase 3: BuildKit (Feature 06) — 🟡 in progress
- [x] Vendor Control-service proto graph (control/worker/ops/policy + google/rpc/status) under `specs/buildkit/`
- [x] Proto codegen via `buffa-build` (protoc + buffa-codegen) in `build.rs` → real `buffa::Message` types
- [x] `BuildKitClient` over the Unix socket on `foundation_connectrpc` `Client<Req,Res>` + `H1Transport`
- [x] `Info` (unary), `Solve` (unary), `Status` (server-stream), `DiskUsage`/`ListWorkers` (unary), `Prune` (server-stream) with `ProcedureCodecs::defaults()` (proto + json)
- [x] gRPC needs HTTP/2 — added `connect_tcp` using `H2Transport` (H1 can't carry gRPC; h2c-over-Unix is a pending transport gap)
- [x] **Validated end-to-end vs real buildkitd v0.31.1** — `Info`/`ListWorkers`/`DiskUsage` pass over gRPC/H2 (testbed: `moby/buildkit` container on TCP; `EWE_BUILDKITD_ADDR`)
- [x] Session-service proto types generated (FileSync/Auth `moby.filesync.v1`, Secrets, SSH, `fsutil.types`)
- [x] **Service code generated via `foundation_connectrpc_codegen`** (per-service): `ControlClient` + FileSync/Auth/Secrets/SSH traits + register fns, wired in `src/buildkit/services.rs`
- [x] **Fixed a real codegen bug**: `generate_services` now emits `+ Send` RPITIT (was `async fn`, non-Send → failed Router's Send bound); streaming outputs boxed to `Pin<Box<dyn Stream + Send>>`
- [x] **Empirically confirmed** via buildkitd testbed: a session-less `dockerfile.v0` Solve returns `could not access local files without session` — so the session server is exactly what's needed
- [x] Service code generated via `foundation_connectrpc_codegen` (Control client + FileSync/Auth/Secrets/SSH traits/register fns); wired in `src/buildkit/services.rs`
- [x] `ClientOptions::with_header` added to connectrpc (custom request headers on streaming calls — for `x-docker-expose-session-*`)
- [x] `Session` sidecar **server built** (`src/buildkit/session.rs`): `DirFileSync` implements FileSync via the fsutil `DiffCopy` protocol (unfold state machine); `SessionServer` serves it on a loopback h2 socket + opens `Control/Session` with the session headers + thread↔pool channel-bridge pump
- [x] **Session now FOUND by buildkitd** — a `dockerfile.v0` Solve gets past "no session" to `ReadEntrypoint`
- [ ] **BLOCKER**: the `Control/Session` bidi closes ~30ms after opening (buildkitd logs "session started"/"session finished: &lt;nil&gt;" back-to-back, before Solve). Verified: my request half goes out correctly (`end_stream=false`, open bidi); buildkit finishes the session without exchanging any bytes. This is a deep connectrpc-H2Transport-bidi ↔ buildkit-session-hijack interaction needing frame-level h2 debugging (does buildkit send the inner-h2 preface? is connectrpc's response path delivering it? why does buildkit's session Run return `nil` in 30ms?).
- [ ] Local Dockerfile build end-to-end via `Solve` + session (blocked on the above)
- [ ] Remaining Control RPCs: build-history wrappers (generated types exist)

## Related specs

- **[Spec 53](../53-docker-container-testbed/)** — Previous bollard attempt
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation (used for BuildKit)
- **[foundation_openapi](../../backends/foundation_openapi/)** — OpenAPI analysis + UnifiedGenerator
- **[foundation_codegentools](../../backends/foundation_codegentools/)** — gen_api binary
- **[/formulas/src.rust/src.Containers/src.moby/](/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/)** — Local moby checkout (specs + protos)
