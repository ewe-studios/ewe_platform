# Progress — Spec 54: foundation_deployment_docker

**Last updated:** 2026-07-12

## Features

| # | Feature | Status | Depends on | Effort |
|---|---------|--------|------------|--------|
| 00 | SendSafeBody Sync | 🔴 Planned | — | Small |
| 01 | DynNetClient + PreparedRequestBuilder alignment | 🔴 Planned | 00 | Medium |
| 02 | Unix socket transport | 🔴 Planned | 01 | Small |
| 03 | Type replication via gen_api | 🔴 Planned | 01, 02 | Large |
| 04 | Streaming endpoint handling | 🔴 Planned | 01, 03 | Small |
| 05 | BuildKit via connectrpc (P2) | 🔴 Planned | 02, 03 | Large |

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

### Phase 0: Foundation (Feature 00, 01, 02)
- [ ] Feature 00: Make `SendSafeBody: Sync` (~3 files, + Sync on one type alias)
- [ ] Feature 01: Update code generator + all deployment crates for DynNetClient
- [ ] Feature 02: Unix socket transport in foundation_netio (~30 lines)

### Phase 1: Core Docker API (Feature 03, 04)
- [ ] Feature 03: Vendor spec, run gen_api, generate types + async fn
- [ ] Feature 03: Hand-write `DockerClient` (Unix socket, auth, version negotiation)
- [ ] Feature 03: Hand-write `Deployable` for `DockerContainer`
- [ ] Feature 04: Hand-write streaming endpoints (logs, events, stats, pull, build)
- [ ] Feature 04: Implement `LogFrameDecoder` (~50 lines)

### Phase 2: Network + Volume (Feature 03 continued)
- [ ] Refine generated network/volume types
- [ ] Hand-write `Deployable` for `DockerNetwork`, `DockerVolume`

### Phase 3: BuildKit (Feature 05)
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
