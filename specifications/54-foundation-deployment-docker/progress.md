# Progress — Spec 54: foundation_deployment_docker

**Last updated:** 2026-07-12

## Features

| # | Feature | Status | Depends on | Effort |
|---|---------|--------|------------|--------|
| 00 | SendSafeBody Sync | ✅ Complete | — | Small |
| 01 | DynNetClient + PreparedRequestBuilder surface (netio/core primitives) | ✅ Complete | 00 | Medium |
| 02 | Unix socket transport | 🔴 Planned | 01 | Small |
| 03 | Generator async fn codegen + regenerate deployment crates | 🔴 Planned | 01 | Large |
| 04 | Docker type replication via gen_api | 🔴 Planned | 02, 03 | Large |
| 05 | Streaming endpoint handling | 🔴 Planned | 01, 04 | Small |
| 06 | BuildKit via connectrpc (P2) | 🔴 Planned | 02, 04 | Large |

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
- [ ] Feature 02: Unix socket transport in foundation_netio (~30 lines)
- [ ] Feature 03: Generator emits `async fn`; regenerate + migrate cloudflare/stripe/supabase/neon/planetscale/prisma/flyio

### Phase 1: Core Docker API (Feature 04, 05)
- [ ] Feature 04: Vendor spec, run gen_api, generate types + async fn
- [ ] Feature 04: Hand-write `DockerClient` (Unix socket, auth, version negotiation)
- [ ] Feature 04: Hand-write `Deployable` for `DockerContainer`
- [ ] Feature 05: Hand-write streaming endpoints (logs, events, stats, pull, build)
- [ ] Feature 05: Implement `LogFrameDecoder` (~50 lines)

### Phase 2: Network + Volume (Feature 04 continued)
- [ ] Refine generated network/volume types
- [ ] Hand-write `Deployable` for `DockerNetwork`, `DockerVolume`

### Phase 3: BuildKit (Feature 06)
- [ ] Vend 17 proto files from local moby checkout
- [ ] Proto codegen via `foundation_connectrpc_codegen`
- [ ] Implement `BuildKitClient` over Unix socket
- [ ] `Solve` (bidi), `Status` (server-stream), `Info` (unary)
- [ ] Inline Dockerfile support
- [ ] `DiskUsage`, `Prune`, `FileSend`, `AuthProvider/Credentials`

## Related specs

- **[Spec 53](../53-docker-container-testbed/)** — Previous bollard attempt
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation (used for BuildKit)
- **[foundation_openapi](../../backends/foundation_openapi/)** — OpenAPI analysis + UnifiedGenerator
- **[foundation_codegentools](../../backends/foundation_codegentools/)** — gen_api binary
- **[/formulas/src.rust/src.Containers/src.moby/](/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/)** — Local moby checkout (specs + protos)
