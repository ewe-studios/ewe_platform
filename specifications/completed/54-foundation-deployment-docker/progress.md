# Progress — Spec 54: foundation_deployment_docker

**Last updated:** 2026-07-15

## Features

| # | Feature | Status | Depends on | Effort |
|---|---------|--------|------------|--------|
| 00 | SendSafeBody Sync | ✅ Complete | — | Small |
| 01 | DynNetClient + PreparedRequestBuilder surface (netio/core primitives) | ✅ Complete | 00 | Medium |
| 02 | Unix socket transport | ✅ Complete | 01 | Small |
| 03 | Generator async fn codegen + regenerate deployment crates | ✅ Complete¹ | 01 | Large |
| 04 | Docker type replication via gen_api | ✅ Complete | 02, 03 | Large |
| 05 | Streaming endpoint handling | ✅ Complete | 01, 04 | Small |
| 06 | BuildKit via connectrpc (P2) | ✅ Complete | 02, 04 | Large |
| 07 | TLS transport (mTLS over TCP) | ✅ Complete | 02 | Small |
| 08 | SSH transport (`ssh://` via `dial-stdio`) | ✅ Complete | 02 | Medium |

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

> **2026-07-14 (later) BuildKit session UNBLOCKED — end-to-end dockerfile build
> green:** the session-death root cause was a double-wrapped H2 PING ACK in
> `foundation_netio`'s `H2Channel` (connection-fatal `FRAME_SIZE_ERROR` for
> grpc-go), plus connectrpc's Connect handler claiming bare `application/grpc`
> and 415-ing gRPC calls. Both fixed (commit 828ddae91);
> `build_dockerfile_end_to_end` passes against buildkitd v0.31.1. Remaining
> Feature 06 work tracked in Phase 3 below. Full investigation writeup:
> `backends/foundation_deployment_docker/specs/buildkit/README.md`.

> **2026-07-14 BuildKit (Feature 06) — real proto types landed:** Vendored the
> Control-service proto graph (control/worker/ops/policy
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

### Phase 2b: Deployable provider integration — ✅ complete (2026-07-15)
- [x] **`Deployable` trait is async** — per decision 05. `deploy`/`destroy`
  return a `BoxFuture<'static, Result<..>>`
  (`Pin<Box<dyn Future + Send>>`) the caller `.await`s; implementors write
  `Box::pin(async move { … })`. No `async_trait` macro, no RPITIT — the trait
  stays **object-safe** (`dyn Deployable`) and `Send` is explicit, not inferred
  (removing the RPITIT Send-inference fragility). The old TaskIterator/
  `Deploying`/`Spawner` machinery and the unused `update()`/`UpdateTask` helper
  were removed from `foundation_deployment::traits`.
- [x] **`Deployable` impl** (`src/deployable.rs`) — the crate's stated purpose
  (requirements §Summary, decisions 03/05). `ContainerDeployment` implements it:
  `deploy` creates+starts a container and persists its id via the namespaced
  state store; `destroy` reads it back and stops+removes it — each a plain
  `Box::pin(async move { … })`. Docker's Unix socket doesn't fit
  `ProviderClient`'s TCP+DNS HTTP client, so the futures build their own
  `DockerClient` and use `ProviderClient` only for state persistence (the
  canonical "unique underlying mechanics" case).
- [x] **`foundation_deployment` docs + README** — the boxed-future contract,
  why (object safety + explicit `Send` vs the one-alloc cost), state
  persistence, the unique-mechanics case, and how to drive a deploy.
- [x] **`build.rs` docker-only fix** — `main()` now always exists (was fully
  `#![cfg(buildkit)]`, so a `--features docker` build had no entry point).
- [x] **`deployable_tests`** — a `#[valtron_test]` that `.await`s
  `deploy`/`destroy`: deploy → inspect(running) → destroy → inspect(gone)
  against a real dockerd (force-removes leftover container first). Passes.
- NOTE: connectrpc codegen is unrelated to `Deployable` (the deployment
  generator is `foundation_openapi`, which already emits plain `async fn`s). No
  other `foundation_deployment` provider implements `Deployable` yet — this is
  the first working impl in the workspace.

### Phase 3: BuildKit (Feature 06) — ✅ complete
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
- [x] **Session-death ROOT CAUSE found + fixed (2026-07-14, commit 828ddae91)**: `H2Channel` double-wrapped its control frames — `PingFrame::encode` emits a *complete* frame and `queue_frame` prepended a second header, so the ACK to grpc-go's BDP-estimator PING went out with `length=17` → RFC 7540 §6.7 `FRAME_SIZE_ERROR` → buildkitd tore down the whole Level 1 connection ~10ms after our first tunneled DATA. Same bug fixed in SETTINGS ACK / GOAWAY / RST_STREAM paths. Second fix: connectrpc's Connect handler claimed bare `application/grpc` (codec `"grpc"`) before the gRPC handler → 415 on every grpc-go call. Full writeup: `backends/foundation_deployment_docker/specs/buildkit/README.md`
- [x] **Local Dockerfile build END-TO-END GREEN** — `build_dockerfile_end_to_end` passes vs buildkitd v0.31.1: session bidi stays open, health checks answered, `FileSync/DiffCopy` serves the context, `dockerfile.v0` solves (alpine layers pulled, overlayfs snapshots built). Suites: integration 8/8, filesync 2/2, types 5/5
- [x] **gRPC over the Unix socket** — `H2Transport::unix` (h2c-over-unix); `connect(socket_path)` reaches buildkitd's default `unix:///run/buildkit/buildkitd.sock`. `info_over_unix_socket` + `build_dockerfile_end_to_end_unix` pass (acceptance criterion 3)
- [x] **`Status` streaming during a real build** — `new_build_ref()` → `SolveRequest.Ref` + concurrent `Status(Ref)`; `build_with_status_stream` observes vertex progress live
- [x] **Exporters + `FileSend/Send`** — `SessionBuilder::export_to_file` + `FileExportSink`; `build_with_oci_export` receives a 3.86 MB OCI tar through the session (asserts ustar magic + size)
- [x] **Inline Dockerfile build** — `SessionServer::builder_inline` (session-owned temp Dockerfile, cleaned on drop); `build_inline_dockerfile_no_context_file` passes (acceptance criterion 4)
- [x] **`Auth/Credentials`** — `StaticRegistryAuth` (per-host creds + anonymous); `build_with_registry_auth_provider` passes (FetchToken/authority keep unimplemented defaults — buildkitd falls back to Credentials)
- [x] **Secrets / SSH session services** — `StaticSecrets` (`RUN --mount=type=secret`, content-asserted in `build_with_secret_mount`) + `SshAgentProxy` (CheckAgent + ForwardAgent byte pump, round-tripped in `ssh_agent_proxy_forwards_bytes`)
- [x] **Gateway client** — `gateway_for_build(ref)` LLBBridge client (routed by `buildkit-controlapi-buildid`); `gateway_build_ping_resolve_and_return` drives Ping + ResolveImageConfig (real alpine digest) + Return against a concurrent `Frontend=""` Solve
- [x] **Build-history wrappers** — `listen_build_history` (server-stream) + `update_build_history` (unary); `build_history_lists_completed_builds` lists 27 records
- [x] **gRPC error trailers decoded client-side** — `read_grpc_response` reader surfaces Trailers-Only + trailing `grpc-status` as `Err` (previously every server error decoded as `Ok(Default::default())` or `transport closed without response head`); streaming responses end in real `grpc-status` trailing HEADERS (`H2Frame::Trailers`)
- [x] **H2 flow control** — `WINDOW_UPDATE` credits (batched at 32 KiB) on both client + server, +4 MiB post-handshake connection window; without it any transfer >64 KiB stalled forever
- [x] **`H2PooledTransport` deleted** (per-call connections validated end-to-end); diagnostic `eprintln!`s → `tracing`
- [x] **connectrpc-codegen keyword fix** — RPC names colliding with Rust keywords (BuildKit's `LLBBridge.Return`) emit as raw identifiers
- [x] **Valtron scheduler fixes** surfaced by the tar-export flow (min-deadline sleep, readiness-poll quantum, Depends spin-guard streak reset) — see `foundation_core/src/valtron/docs/scheduler_latency_and_depends_guard.md`

**Full suite: buildkit integration 17/17, filesync/ssh 3/3, types 5/5;
regressions connectrpc 180, netio+http 1198, core valtron 333 — all green.**
Feature-06 throughput narrative:
`features/06-buildkit-connectrpc/export-throughput.md`.

**Known upstream bug found:** buildkitd v0.31.1 nil-derefs (`gateway.go:1040`)
on a Gateway `Return` with neither `Result` nor `Error` set — a real frontend
always sets one; our client/test set `Error` to abort cleanly.

### Phase 4: Remote transports (Feature 07, 08) — ✅ complete (2026-07-15)

- [x] **Feature 07: TLS transport** — `SSLConnector::from_client_mutual_pem`
  (uniform PEM mTLS constructor, rustls); `DockerTls { from_cert_dir }`;
  `DockerClient::connect_tls`; `base_url()` emits `https://` under TLS;
  `connect_with_defaults` parses `unix://` / `tcp://` (+
  `DOCKER_TLS_VERIFY` / `DOCKER_CERT_PATH`) / `ssh://`. Verified against
  docker-in-dind (`DOCKER_TLS_CERTDIR`): `tls_transport_tests.rs` 3/3
  (full-mTLS info, mTLS container round-trip, insecure-TLS info).
- [x] **Feature 08: SSH transport** — `connect_session()` extracted from pool
  (TCP + handshake + host-key verify + auth); `ChannelStream` (exec channel as
  `Read+Write` duplex owning its session); `Dialer` (session cache, fresh channel
  per request); `Host::resolve()` for `~/.ssh/config` (HostName/User/Port/
  IdentityFile); TOFU host-key verification with MITM rejection; OpenSSH auth
  order (password → agent → key files). Reused the existing `Connector` seam
  rather than a new transport mode; `should_pool()` opt-out for one-shot channels.
  `SshOverlay` + `SshConnector` in `client/ssh.rs`. Verified against dind + sshd:
  `ssh_transport_tests.rs` 2/2 (info + container round-trip).

**All 8 features complete. Spec-54 is done.**

## Related specs

- **[Spec 53](../53-docker-container-testbed/)** — Previous bollard attempt
- **[Spec 41](../41-connectrpc/)** — ConnectRPC foundation (used for BuildKit)
- **[foundation_openapi](../../backends/foundation_openapi/)** — OpenAPI analysis + UnifiedGenerator
- **[foundation_codegentools](../../backends/foundation_codegentools/)** — gen_api binary
- **[/formulas/src.rust/src.Containers/src.moby/](/home/darkvoid/Boxxed/@formulas/src.rust/src.Containers/src.moby/)** — Local moby checkout (specs + protos)
