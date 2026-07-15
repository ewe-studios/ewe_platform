# BuildKit Session Protocol — Deep Dive

> **Status (2026-07-14): WORKING end-to-end.** `build_dockerfile_end_to_end`
> passes against a real buildkitd v0.31.1 — session bidi stays open, buildkitd
> pulls the build context through our `FileSync/DiffCopy`, and a `dockerfile.v0`
> Solve completes (alpine layers pulled, overlayfs snapshots built). The
> session-death root cause and its fix are documented at the bottom
> ([root cause](#root-cause-found-2026-07-14-malformed-ping-ack-on-the-level-1-connection)).

**Terminology used throughout:** the session tunnels one HTTP/2 connection
inside another, so frames exist at two levels:

- **Level 1** — the outer H2 connection to buildkitd's TCP port, carrying the
  Control gRPC (our `H2Transport`; we are the H2 client). The `Control/Session`
  bidi is one stream on it.
- **Level 2** — the H2 connection buildkitd's `grpcClientConn` runs *inside*
  the Session bidi's `BytesMessage` payloads (buildkitd is the H2 client; our
  loopback server is the H2 server).

A byte that buildkitd sends on Level 2 arrives to us as: Level 1 DATA frame →
gRPC envelope → `BytesMessage.data` → pump → loopback TCP → our H2 server.

## Architecture at a glance

A `docker build` with BuildKit involves **two distinct gRPC connections** between the
client and `buildkitd`, in opposite directions:

```
                        Control connection
       ┌─────────────────────────────────────────────→ buildkitd
       │ (we are the client)                            │
  our  │    Solve(), Status(), ListWorkers(), …         │
  code │                                               │
       │←──────────────────────────────────────────────│
                        Session connection
           (buildkitd is the client, we are the server)
              FileSync/DiffCopy, Health/Check, …
```

### Why two connections in opposite directions?

buildkitd can't read files from your machine. Your Dockerfile and build context
live on your disk. So buildkitd **calls back** to the client — the session
connection — to pull them. The `moby.buildkit.v1.Control/Session` bidi stream
carries a **multiplexed HTTP/2 connection**; on that H2 connection buildkitd is
the H2/gRPC client making RPC calls to us.

This is why we must run an **H2 + gRPC server** — we respond to buildkitd's
callbacks.

---

## The two connections in detail

### 1. Control connection (`H1Transport` over Unix socket)

| Property | Value |
|----------|-------|
| Transport | `foundation_connectrpc::H1Transport` over `DynNetClient` |
| Socket | `/run/buildkit/buildkitd.sock` (or `tcp://…`) |
| Our role | **gRPC client** |
| Protocol | `moby.buildkit.v1.Control` |
| RPCs we call | `Solve`, `Status`, `Info`, `ListWorkers`, `DiskUsage`, `Prune` |
| Codec | `ProcedureCodecs::defaults()` — proto + json via `buffa` |

```rust
// We open this. Standard gRPC client.
let client = BuildKitClient::connect_tcp("127.0.0.1:13434")?;
let info = client.info().await?;
```

### 2. Session connection (`H2Transport` over the `Control/Session` bidi)

| Property | Value |
|----------|-------|
| Transport | `foundation_connectrpc::H2Transport` (one conn per call) |
| Underlying stream | `Control/Session` bidi — buildkitd multiplexes H2 over it |
| Our role | **gRPC server** |
| Protocol | `grpc.health.v1.Health`, `moby.filesync.v1.FileSync`, `moby.filesync.v1.Auth`, `moby.buildkit.secrets.v1.Secrets`, `moby.sshforward.v1.SSH` |
| RPCs buildkitd calls | `Health/Check`, `FileSync/DiffCopy`, `FileSync/TarStream`, `Auth/Credentials`, `Auth/FetchToken`, `Secrets/GetSecret`, `SSH/CheckAgent`, `SSH/ForwardAgent` |
| Codec | `ProcedureCodecs::defaults()` — proto + json via `buffa` |

```rust
// We spin up a loopback H2 server, then bridge its TCP socket to the Session bidi.
let session = SessionServer::start(&buildkitd_addr, &context_dir).await?;
let mut req = SolveRequest::default();
req.Session = session.id;
client.solve(req).await?;
```

---

## Session lifecycle — step by step

### Phase 0: Bring up the loopback gRPC server

1. `SessionServer::start()` creates a `Router`, registers `FileSync` + `Health`
   handlers, wraps it in `ConnectRpcServeH2`, and adds routes to `HttpApp`.
2. Binds a `TcpListener` on `127.0.0.1:0`. Starts `HttpServer` in a background
   thread — this is an **H2C (cleartext HTTP/2) server** on the random port.

### Phase 1: Open `Control/Session`

3. Opens a bidi gRPC call to `http://{buildkitd}/moby.buildkit.v1.Control/Session`
   with these **required headers** on the call:

   | Header | Value | Purpose |
   |--------|-------|---------|
   | `x-docker-expose-session-uuid` | random 32-char hex | Ties the session to this Solve |
   | `x-docker-expose-session-name` | `"ewe"` | Debug label |
   | `x-docker-expose-session-grpc-method` | `/moby.filesync.v1.FileSync/DiffCopy` | **Repeated** — one per registered service method. Tells buildkitd which RPCs our loopback server handles |

   The header is **repeated** (not comma-separated). buildkitd reads each
   instance as a separate entry in `meta["x-docker-expose-session-grpc-method"]`.
   Missing a method means buildkitd won't route that RPC through the session.

   **Bollard** does the same via a tonic `Interceptor` that appends one
   `x-docker-expose-session-grpc-method` per registered gRPC service method
   (see `bollard/src/grpc/driver/mod.rs:74-94`).

4. After opening the bidi, calls `bidi.split()` to get separate sender and receiver.

### Phase 2: Bridge the Session bidi ↔ loopback TCP

5. Dials `127.0.0.1:{port}` — connects to our own loopback H2 server.
6. Spawns two **byte pumps** (bidi → server, server → bidi):

```
buildkitd  ──Session bidi──→ [receiver] ──mpsc──→ [write thread] ──→ loopback server
buildkitd ←──Session bidi── [sender]   ←─unbounded─ [read thread] ←── loopback server
```

   Each pump is split across two threads:
   - **Blocking thread**: does TCP `read`/`write` (never on a valtron pool worker)
   - **Async task** (pool-detached): does `sender.send()`/`receiver.receive()`

   This split is **necessary**: blocking I/O on a valtron worker starves the pool.
   `std::sync::mpsc` + `futures::channel::mpsc` bridge the two worlds.

### Phase 3: buildkitd's `grpcClientConn` dials us

When buildkitd receives the `Control/Session` bidi (verified against v0.31.1
sources — `control/control.go`, `session/manager.go`, `session/grpc.go`,
`session/grpchijack/dial.go`):

7. `Controller.Session()` hijacks the gRPC stream:
   `conn, closeCh, opts := grpchijack.Hijack(stream)` — the bidi becomes a raw
   `net.Conn` whose `Read`/`Write` are `stream.RecvMsg`/`SendMsg` over
   `BytesMessage`. It then **blocks** on `SessionManager.HandleConn(ctx, conn,
   opts)` for the session's whole lifetime, with
   `ctx = WithCancelCause(stream.Context())` and a goroutine that cancels on
   `<-closeCh`.
8. `handleConn` → `grpcClientConn(ctx, conn, opts)` wraps the hijacked conn in
   a `*grpc.ClientConn` via `grpc.DialContext(ctx, "localhost", …)` with a
   **one-shot custom dialer** (a second dial attempt returns
   `"only one connection allowed"`). `DialContext` (the old, eager API)
   connects immediately — that's why the Level 2 client preface arrives right
   after `session started`.
9. It starts **`monitorHealth(ctx, conn, cc, cancelConn, cfg)`** with defaults
   `interval=5s`, `defaultTimeout=15s` (grows to 1.5× the last check's
   duration), `failureThreshold=2`, `successResetThreshold=1` (overridable via
   the `X-Buildkit-Session-Health-Custom-Timeout` header, floored at 1s).
   Health checks are the only traffic buildkitd initiates unprompted.
10. `handleConn` then parks on `<-c.ctx.Done()` and returns `nil` when it
    fires. RPCs (`FileSync/DiffCopy`, …) are routed through the Level 2
    connection on demand by the solver.

**The teardown chain (memorize this — it decodes the logs):**

```
monitorHealth exits
  → closeConn(): cancelConn() + cc.Close() + conn.Close()
      → cc.Close() = Level 2 transport writes GOAWAY "client transport shutdown"
  → handleConn's <-c.ctx.Done() unblocks → returns nil
  → Session() logs "session finished: <nil>" → defer conn.Close()
  → Level 1 stream ends → our pump reads 0
```

`monitorHealth` itself exits only two ways: `<-ctx.Done()` (parent context
canceled) or `failureThreshold` consecutive failed health checks (logged as
`healthcheck failed` warnings, then `healthcheck failed fatally`). Every other
`conn.Close`/cancel path in the chain is *downstream* of one of those two.
Since the ticker can't fire before 5s, **any `session finished: <nil>` earlier
than ~5s means the Level 1 stream context was canceled** — i.e. buildkitd's
gRPC server killed our Session stream (client RST_STREAM, a protocol error we
committed, or TCP close). That deduction is what cracked the session-death bug.

### Phase 4: The handshake (H2 server side)

When buildkitd's gRPC `ClientConn` connects to our loopback H2 server:

10. buildkitd sends **H2 client preface + SETTINGS**.
11. Our server (`H2ConnectionHandler → H2Conn → H2Connection::server_recv_settings`):
    - **Sends our own SETTINGS first** (the server connection preface) — MUST be
      the first frame the server sends per RFC 7540 §3.5.
    - **Then** sends ACK for client's SETTINGS.
    - Sending ACK first is a protocol violation; grpc-go (buildkitd's client)
      rejects it. Our own H2 client tolerated it, hiding the bug.
12. buildkitd ACKs our SETTINGS. Handshake complete.

### Phase 5: Health check

13. buildkitd's `monitorHealth` goroutine calls `/grpc.health.v1.Health/Check`
    over the Level 2 connection — first check at the 5s ticker, then every 5s.
14. Our `HealthService` returns `SERVING`. Without it (or with the Level 2
    handshake incomplete), each check runs into its 15s timeout; after 2
    consecutive failures `monitorHealth` calls `closeConn` → session torn down
    at ~40s (observed empirically: `session started` → first
    `healthcheck failed` warn at +20s → `healthcheck failed fatally` at +43s).

### Phase 6: FileSync

15. buildkitd's `dockerfile.v0` frontend opens `FileSync/DiffCopy` (bidi stream).
16. Our `DirFileSync` walks the context directory, streams `PACKET_STAT` entries,
    an empty-stat terminator, then serves `PACKET_DATA` responses for each
    `PACKET_REQ`.

### Phase 7: Teardown

17. `Solve` completes (or fails). buildkitd closes all streams.
18. Our `SessionServer::Drop` fires the shutdown signal → `HttpServer` stops
    accepting → pump threads stop when `read()` returns 0 → `SessionServer` is
    dropped.

---

## HTTP/2 SETTINGS — protocol pitfalls

Several subtle bugs surfaced during implementation:

### 1. Server SETTINGS MUST precede ACK (RFC 7540 §3.5)

The server connection preface is "a SETTINGS frame that MUST be the first frame
the server sends." Sending SETTINGS ACK **before** our own SETTINGS is a protocol
violation. grpc-go (buildkitd's session client) rejects it; our own H2 client
tolerated it.

**Fix** (`connection.rs:server_recv_settings`): send `to_frame_server()` first,
then the ACK. Same in `channel.rs:server_handshake_step`.

### 2. Server MUST NOT advertise `SETTINGS_ENABLE_PUSH`

`ENABLE_PUSH` (id 2) is a **client→server** control: it tells the server whether
the client accepts server push. A server advertising it (especially value 1) is
rejected by strict stacks like grpc-go. Real servers (nginx, grpc-go) never send it.

**Fix** (`settings.rs`): `to_frame_server()` omits `ENABLE_PUSH` from the
SETTINGS frame. `to_frame()` (used by the client) still includes it.

### 3. `MAX_CONCURRENT_STREAMS = u32::MAX`

This is `0xFFFF_FFFF` on the wire. Go's `http2` package stores the remote value
in an `int32`; `u32::MAX` overflows to `i32(-1)`, which gets clamped to 0,
effectively forbidding any streams. Our H2 client tolerated it because it didn't
check; grpc-go did.

**Fix** (`settings.rs`): default capped at `256`. RFC 7540 leaves this unbounded,
but in practice every real implementation caps it.

### 4. Control frames MUST have exact lengths — no double-wrapping (§6.4/6.5/6.7)

`PingFrame::encode()` (and every other frame `encode` in
`foundation_netio/src/http2/frame/mod.rs`) emits the **complete** frame —
9-byte header + payload. `H2Channel` passed that output to `queue_frame`,
which prepended a *second* header, producing PING ACKs with `length=17`
(must be 8), SETTINGS ACKs with `length=9` (must be 0), and RST_STREAMs with
`length=13` (must be 4). Each is a **connection error** (`FRAME_SIZE_ERROR`)
— grpc-go tears the whole connection down, canceling every stream on it.
This was THE session killer (see root-cause section).

**Fix** (`channel.rs`): all control frames encode directly into `write_buf`;
`queue_frame` deleted.

### 5. Never answer a PING that has the ACK flag (§6.7)

An ACK is the peer's *answer* to one of our PINGs; responding to it is
forbidden. `H2Conn::read_stream_frame()` (server side) used to ACK every
`Kind::Ping` — including buildkitd's ACK of our keepalive PING.

**Fix** (`conn.rs`): skip frames with `ping_flags::ACK` set.

### 6. The server's first frame MUST be SETTINGS — no early PING (§3.5)

A "pre-handshake PING" injected into the bidi before the loopback server's
SETTINGS (an old keepalive workaround) is itself connection-fatal: grpc-go's
`http2Client.reader()` requires the first frame from the server to be
SETTINGS and closes the transport on anything else. Removed.

### 7. Bare `application/grpc` belongs to the gRPC handler, not Connect

grpc-go sends `content-type: application/grpc` (no `+proto` suffix). Our
router dispatch offers protocol handlers in order (Connect, gRPC, gRPC-Web);
`parse_connect_content_type` parsed bare `application/grpc` as a Connect
request with a codec named `"grpc"`, claimed it, failed codec membership, and
answered **415** — so buildkitd's `DiffCopy` died with
`unexpected HTTP status code received from server: 415 (Unsupported Media
Type); malformed header: missing HTTP content-type`.

**Fix** (`foundation_connectrpc/src/shared/protocol/mod.rs`): the bare
`grpc` / `grpc-web` subtypes are rejected by the Connect parser, letting
dispatch fall through to `GrpcHandler` (which maps bare `application/grpc` to
the default `proto` codec, per gRPC spec).

---

## `grpc.health.v1.Health` — required, not optional

buildkitd's `grpcClientConn` runs `monitorHealth` in a goroutine
(v0.31.1 `session/grpc.go`):

```go
ctx, cancel := context.WithCancelCause(ctx)
go monitorHealth(ctx, conn, cc, cancel, healthCheckConfigFromHeaders(opts))
```

Defaults: `interval=5s`, `defaultTimeout=15s` (adaptive: grows to 1.5× the
previous check's duration), `failureThreshold=2`, `successResetThreshold=1`.
The `X-Buildkit-Session-Health-Custom-Timeout` header (milliseconds, floored
at 1s) overrides interval+timeout and drops the threshold to 1 — buildkit uses
it in tests.

A session with no working Health service therefore survives **~40 seconds**
(2 × 15s timeouts on the 5s ticker), not milliseconds. If your session dies
faster than the first ticker (5s), the health check is *not* your problem —
look for a Level 1 stream cancellation instead (see the teardown chain in
Phase 3 and the root-cause section).

Our `HealthService` returns `ServingStatus::SERVING` unconditionally. The
`Watch` server-stream RPC is left as `unimplemented` — buildkitd only calls
`Check`.

---

## FileSync protocol (`moby.filesync.v1.FileSync`)

### DiffCopy (bidi stream)

The server (us) streams `fsutil.types.Packet` to buildkitd:

```
[server → buildkitd]  PACKET_STAT { path: "Dockerfile", mode: 0o644, size: N }
[server → buildkitd]  PACKET_STAT { path: "src/main.rs", … }
[server → buildkitd]  PACKET_STAT { … }  → empty stats object = "end of walk"
                       ^^ buildkitd now knows every file

[buildkitd → server]  PACKET_REQ { ID: 0 }  → "give me file 0"
[server → buildkitd]  PACKET_DATA { ID: 0, data: <chunk> }
[server → buildkitd]  PACKET_DATA { ID: 0, data: <> }  → empty = EOF
[buildkitd → server]  PACKET_FIN  → "done"
[server → buildkitd]  PACKET_FIN  → "ack"
```

### Key details

- **fsutil paths** are slash-separated and relative to the context root. No
  leading slash. Leading `/` in the Dockerfile path ("filename" frontend attr)
  is stripped by buildkitd before looking up the stat.
- **Dir entries** use `mode = 0x8000_0000 | 0o755` (`GO_MODE_DIR` bit set).
- **Chunk size** should stay well below buildkitd's default 16 MiB gRPC receive limit.
- Each `PACKET_REQ` references a file by index (0-based position in the stat
  walk), matching what Go's fsutil does.

### Known proto/wire mismatch (buildkit issue #2109)

The `.proto` files declare `FileSync/DiffCopy` as taking `BytesMessage`, but the
**actual wire format** is `fsutil.types.Packet`. This works in Go because the
gRPC library takes `interface{}` when encoding/decoding. Non-Go clients must
send/receive `Packet`, not `BytesMessage`.

---

## Session headers reference

These are set on the `Control/Session` bidi call, not on `Solve`:

| Header | Cardinality | Example | Notes |
|--------|-------------|---------|-------|
| `x-docker-expose-session-uuid` | 1 | `a1b2c3d4…` | Must match `SolveRequest.Session` |
| `x-docker-expose-session-name` | 1 | `ewe` | Debug label in buildkitd logs |
| `x-docker-expose-session-grpc-method` | N | `/moby.filesync.v1.FileSync/DiffCopy` | One per registered service RPC |
| `x-docker-expose-session-grpc-method` | N | `/moby.filesync.v1.FileSync/TarStream` | Repeated for each additional method |
| `x-docker-expose-session-grpc-method` | N | `/grpc.health.v1.Health/Check` | Health is implicit but explicit is safer |
| `x-docker-expose-session-grpc-method` | N | `/grpc.health.v1.Health/Watch` | ditto |

The `with_header` method on `ClientOptions` appends values (repeated calls with
the same key = repeated header values), which is exactly what buildkitd needs.

---

## Comparison with bollard

| Aspect | bollard | us |
|--------|---------|-----|
| gRPC runtime | tonic + tokio | `foundation_connectrpc` + valtron |
| Session server transport | `tokio::io::duplex()` (in-memory pipe) | TCP loopback (`127.0.0.1:0`) |
| Session pump | tokio tasks over duplex | raw thread + channel + pool-detached async |
| Service generation | `tonic-build` (prost) | `buffa-build` + `foundation_connectrpc_codegen` |
| Health service | Tonic `HealthServer` | Our `ConnectRpcServeH2` |
| H2 implementation | Go stdlib `net/http` (buildkitd's grpc.ClientConn) | Our `H2Connection` + `H2Channel` |

bollard uses `tokio::io::duplex()` to create an in-memory pipe: one end is read
by tonic's gRPC server, the other end sends/receives on the Session bidi. We
use a TCP loopback instead — functionally equivalent, but TCP is easier to debug
(you can `nc` it, hexdump it, Wireshark it).

---

## Our wire format — control `Solve` request

```rust
let mut req = SolveRequest::default();
req.Frontend = "dockerfile.v0".into();
req.FrontendAttrs.insert("filename".into(), "Dockerfile".into());
req.Session = session.id;  // must match x-docker-expose-session-uuid
```

buildkitd's `dockerfile.v0` frontend:
1. Reads `FrontendAttrs["filename"]` → Dockerfile path relative to context root.
2. Calls `FileSync/DiffCopy` on the session connection.
3. Receives stat packets, matches the filename, requests the file.
4. Parses the Dockerfile, resolves instructions into LLB ops.
5. Solves the LLB DAG, streams `StatusResponse` on the control connection.

---

## Key files in our codebase

```
backends/foundation_deployment_docker/
├── build.rs                          # buffa-build proto → Rust + service codegen
├── src/buildkit/
│   ├── mod.rs                        # BuildKitClient (control connection)
│   ├── session.rs                    # SessionServer + DirFileSync + HealthService
│   ├── generated.rs                  # Generated proto message tree glue
│   ├── services.rs                   # Generated service traits + register fns
│   └── types.rs                      # SolveRequest, StatusResponse, etc. re-exports
├── specs/buildkit/
│   ├── github.com/moby/buildkit/…    # Vendored buildkit protos
│   ├── grpc/health/v1/health.proto   # Vendored grpc health proto
│   └── README.md                     # ← this file
└── tests/
    ├── buildkit_integration_tests.rs # E2E against real buildkitd
    └── buildkit_filesync_server_test.rs # FileSync DiffCopy unit test (no buildkitd)

backends/foundation_netio/src/http2/
├── settings.rs   # SettingsStore + to_frame() / to_frame_server()
├── connection.rs # H2Connection<S> — shared state machine
├── channel.rs    # H2Channel — non-blocking I/O variant
└── conn.rs       # H2Conn — concrete H2Connection over SharedByteBufferStream

backends/foundation_http/src/native/server/
└── h2_connection.rs  # H2ConnectionHandler — valtron TaskIterator driving server H2
```

---

## Debugging checklist

When the session dies unexpectedly:

1. **Handshake complete?** Look for SETTINGS + ACK both ways in the hex dumps.
   The server MUST send its SETTINGS before ACKing the client's.

2. **ENABLE_PUSH absent?** The server's SETTINGS frame should be 39 bytes
   (5 settings × 6 bytes + header), not 45 (6 settings). If 45, the server is
   advertising ENABLE_PUSH.

3. **MAX_CONCURRENT_STREAMS sane?** Verify it's not `0xFF_FF_FF_FF` in the
   SETTINGS frame (bytes at offset 4-7 of the `0003` setting).

4. **Health check responding?** buildkitd sends HEADERS on stream 1 with
   `:path /grpc.health.v1.Health/Check`. If we 404, the session dies.

5. **`x-docker-expose-session-grpc-method` repeated?** Missing a method = that
   RPC never reaches us.

6. **Session id matches?** `x-docker-expose-session-uuid` must equal
   `SolveRequest.Session`.

7. **TCP loopback stuck?** Both pumps are on raw threads — if one blocks
   (e.g., `tcp_read.read()` waiting), check that the socket is non-blocking or
   that the peer is actually writing. The blocking thread approach is correct
   but fragile — consider valtron-driven non-blocking I/O for production.

8. **Decode the death time.** `session finished: <nil>` earlier than the 5s
   health ticker ⇒ the **Level 1** Session stream context was canceled — the
   fault is on the outer connection (something *we* sent), not in the tunnel
   payload. Death at ~40s with `healthcheck failed` warnings ⇒ the Level 2
   handshake or Health service is broken.

9. **`GODEBUG=http2debug=2` logs only successfully-parsed frames.** Both the
   grpc-go server (Level 1) and the tunneled client (Level 2) framers log
   `read`/`wrote` lines — but a malformed frame dies inside `ReadFrame`
   *before* the log line. A connection that dies right after your side sent
   something, with a clean-looking peer log, means **your frame didn't parse**.
   (DATA frames are also not logged — only control frames and HEADERS.)

10. **Know grpc-go's reflexes.** After receiving its first DATA, grpc-go sends
    `WINDOW_UPDATE (conn) incr=N` plus a **BDP-estimator PING** with opaque
    data `\x02\x04\x10\x10\t\x0e\a\a` (`{2,4,16,16,9,14,7,7}`) and expects an
    exact-echo ACK of length 8. If your session dies one poll-tick after your
    first DATA frames, inspect your PING ACK encoding first.

11. **The held-handshake experiment isolates the trigger.** Open the Session
    bidi, read buildkitd's Level 2 preface + SETTINGS, respond with
    *nothing*, and time the death. Alive until ~40s (health timeouts) ⇒ your
    Level 1 client is clean and the killer is something you *send*. Dead in
    milliseconds ⇒ the Level 1 connection itself is broken. Bisect what you
    send from there (SETTINGS → ACK → PING ACK → DATA).

12. **`docker kill -s QUIT ewe-buildkitd`** dumps all goroutine stacks to the
    container log — useful to confirm `Session()` is parked in `HandleConn`
    and whether `monitorHealth` is still alive at a given moment.

---

## Running the tests

```sh
# Unit test: FileSync DiffCopy (no buildkitd needed)
cargo test -p foundation_deployment_docker --features buildkit \
    --profile uat --test buildkit_filesync_server_test -- --test-threads=1

# Integration test: full Solve against real buildkitd.
# Add -e GODEBUG=http2debug=2 and --debug when diagnosing — the framer log
# shows every parsed control frame on BOTH the Level 1 and Level 2 connections
# (distinguish them by the Framer pointer address in each line).
docker run -d --name ewe-buildkitd --privileged \
    -e GODEBUG=http2debug=2 -p 127.0.0.1:13434:1234 \
    moby/buildkit:latest --addr tcp://0.0.0.0:1234 --debug

cargo test -p foundation_deployment_docker --features "buildkit,integration-tests" \
    --profile uat --test buildkit_integration_tests build_dockerfile_end_to_end \
    -- --nocapture --test-threads=1

# docker logs ewe-buildkitd 2>&1 | grep -E "error|session|healthcheck|Framer"
```

Tests skip (pass) when no buildkitd is reachable, so CI stays green; point
`EWE_BUILDKITD_ADDR` elsewhere to override the default `127.0.0.1:13434`.
Besides the end-to-end build, `buildkit_integration_tests` carries the
diagnostic tests used in the investigation (`session_stays_alive_without_solve`,
`session_bidi_manual_response`, `session_bidi_raw_no_pump`) — they're cheap
and worth keeping: each pins down a different layer of the tunnel.

With `RUST_LOG=debug`, the H2 connection handler logs every frame type on
receive, route dispatch decisions, and handshake completion. The session pump
uses `eprintln!` for raw frame hex dumps (these are loud; in production they
move to `tracing::trace!`).

## Session bidi closure — root cause investigation (2026-07-14, RESOLVED)

Kept as a worked example of debugging a tunneled-H2 failure: the symptom
tables below record what each experiment proved *at the time*; the actual
root cause (which explains every row) follows them.

### Symptoms

1. Level 2 H2 handshake completes cleanly (both SETTINGS + ACKs exchanged)
2. ~10ms later: buildkitd sends GOAWAY NO_ERROR "client transport shutdown"
3. `session started` + `session finished: <nil>` logged in the same second
4. Solve returns `Canceled: context canceled`

### Diagnostic commands

```sh
# http2 debug logging on buildkitd
docker run -d --name ewe-buildkitd --privileged \
  -e GODEBUG=http2debug=2 \
  -p 127.0.0.1:13434:1234 \
  moby/buildkit:latest --addr tcp://0.0.0.0:1234 --debug

# Run the manual response test (opens Session bidi, completes the Level 2
# handshake by hand — server SETTINGS then SETTINGS ACK — and times how long
# the bidi survives afterwards)
cargo test -p foundation_deployment_docker --features "buildkit,integration-tests" \
  --profile uat --test buildkit_integration_tests \
  session_bidi_manual_response -- --nocapture
```

### What we've proven

| Assertion | Evidence |
|-----------|----------|
| H2 handshake works (both levels) | `http2debug=2` shows SETTINGS + ACK both ways |
| Our PING arrives at Level 2 transport | `read PING len=8` logged on the session framer |
| `to_frame_server()` emits empty SETTINGS | Hex dump: `000000040000000000` (0 settings) |
| GOAWAY source is `cc.Close()` | `Debug="client transport shutdown"` = grpc-go's `transport.Close()` |
| `monitorHealth` goroutine exits immediately | `session started` → `session finished` same second |
| Not a version-specific regression | Same behavior on v0.18.1, v0.31.1 |
| Not a timing race | Our PING + SETTINGS arrive within 15ms; GOAWAY fires after |
| Bollard uses in-memory pipe, not TCP | `tokio::io::duplex()` — zero network latency |

### Proven findings

| # | Finding | Source |
|---|---------|--------|
| 1 | `session started` → `session finished: <nil>` in same second | buildkitd `--debug` log |
| 2 | `handleConn` returns `nil` because `<-c.ctx.Done()` unblocks immediately | source: `session/manager.go` |
| 3 | `grpcClientConn` spawns `go monitorHealth(...)` and returns | source: `session/grpc.go` |
| 4 | `monitorHealth` goroutine exits, firing `defer cc.Close()` → GOAWAY | `http2debug=2`: "client transport shutdown" |
| 5 | `defer cancelConn(context.Canceled)` fires → session context cancelled | `handleConn`'s `c.ctx.Done()` unblocks |
| 6 | `Session()` gRPC handler blocks on `HandleConn` for session lifetime | source: `control/control.go` |
| 7 | `grpchijack.Hijack(stream)` takes over the gRPC bidi — the Level 1 H2 stream becomes a raw `net.Conn` passed to `grpcClientConn` | source: `control/control.go` |
| 8 | Our Level 2 SETTINGS are empty (correct — `to_frame_server()` working) | hex: `000000040000000000` |
| 9 | Our PING arrives at Level 2 transport BEFORE GOAWAY fires | `http2debug=2`: `read PING len=8` |
| 10 | GOAWAY fires AFTER PING arrives (not a timing race) | `http2debug=2`: GOAWAY after PING read |
| 11 | **Same behavior on v0.18.1 and v0.31.1** — not a version regression | tested both images |
| 12 | Bollard uses `tokio::io::duplex()` (in-memory pipe), NOT TCP loopback | source: `bollard/src/grpc/driver/channel.rs:57` |

### Decisive experiments (the ones that cracked it)

| Experiment | Result | What it proved |
|-----------|--------|----------------|
| Read v0.31.1 `monitorHealth` — enumerate every exit path | only `<-ctx.Done()` can fire before the 5s ticker | the 10ms death = **parent ctx canceled**, not a failed health check |
| Trace every `conn.Close`/cancel in `Session()`/`handleConn`/`grpchijack` | `closeCh`, `serve()`, `closeConn` are all *downstream* of the cancel | the only non-circular canceler is `stream.Context()` — buildkitd's gRPC server killed **our Level 1 stream** |
| Hold the handshake (respond with nothing) | session alive 43s, died of legit health-check timeouts | Level 1 client, pump, valtron, TCP all clean at rest; the killer is something we **send** |
| Complete the handshake manually (SETTINGS + ACK only) | dead at +10ms — exactly one client `POLL_DELAY` after buildkitd's `WINDOW_UPDATE` + BDP PING arrived | narrowed the poison to our reaction to those frames |
| `http2debug=2` full framer log on the dying run | Level 1 server wrote `WINDOW_UPDATE (conn) incr=16` + `PING \x02\x04\x10\x10\t\x0e\a\a`, then `session finished` — **nothing read from us in between** | our fatal bytes never parsed (framer logs only parsed frames) → malformed frame |
| Read `H2Channel::handle_ping` | `PingFrame::encode` (full frame) fed to `queue_frame` (adds another header) | PING ACK length=17 → `FRAME_SIZE_ERROR` → connection teardown. QED |

An earlier "nuclear" variant of the hold experiment died at 10ms and nearly
sent the investigation astray — it was still sending the (since-removed)
pre-handshake PING, itself a §3.5 violation. Lesson: strip *every* workaround
before trusting an isolation experiment.

### ROOT CAUSE (found 2026-07-14): malformed PING ACK on the Level 1 connection

`H2Channel::handle_ping` (`foundation_netio/src/http2/channel.rs`) built its
PING ACK by calling `PingFrame::encode` — which emits a **complete** frame
(9-byte header + 8-byte opaque data) — and then passed those 17 bytes to
`queue_frame`, which prepended a **second** 9-byte header. The wire result: a
PING frame with `length=17`.

RFC 7540 §6.7: a PING frame with a length other than 8 is a **connection error
of type FRAME_SIZE_ERROR**. The kill chain:

1. Our first DATA frames on the Session stream (the tunneled Level 2 server
   SETTINGS) prompt grpc-go's **BDP estimator** to send a PING
   (`opaque = {2,4,16,16,9,14,7,7}`) on the **Level 1** connection.
2. Our Level 1 client H2 pump ACKs it — malformed (length 17).
3. buildkitd's grpc-go server hits the parse error in `ReadFrame` **before**
   the `http2debug=2` log line (frames are only logged after a successful
   parse — this is why the wire looked clean) and tears down the whole
   Level 1 connection.
4. Every stream context on that connection is canceled →
   `monitorHealth` exits via `<-ctx.Done()` → its `closeConn` defers fire →
   `cc.Close()` writes the Level 2 GOAWAY `"client transport shutdown"` →
   `handleConn` returns `nil` → `session finished: <nil>` → our pump reads 0.

Every earlier observation is explained: the death always landed one
`POLL_DELAY` (10ms) after our first DATA frames because that's when the
poisoned ACK went out; holding the handshake (sending nothing) kept the
session alive for 43s (no DATA → no BDP ping → no ACK) until health checks
legitimately timed out; the behavior was version-independent and immune to
every latency/keepalive/loopback change.

The same double-wrap bug existed in `handle_settings` (SETTINGS ACK with a
9-byte payload — §6.5 requires ACK length 0), `send_goaway`, and
`send_rst_stream` (length 13 instead of 4, §6.4). All four now encode the
complete frame directly into `write_buf`; `queue_frame` was deleted. The
server-side `H2Conn` additionally no longer answers PING **ACK**s (§6.7
forbids ACKing an ACK).

### Follow-up fix: Connect handler claimed `application/grpc` (HTTP 415)

With the session alive, buildkitd's `FileSync/DiffCopy` call reached the
loopback server and was rejected 415. Dispatch offers protocol handlers in
order (Connect, gRPC, gRPC-Web) and `parse_connect_content_type` parsed bare
`application/grpc` as a Connect request with a codec named `"grpc"` — claiming
the request, then failing codec membership. It now rejects the bare `grpc` /
`grpc-web` subtypes so the gRPC handler gets the request.

With both fixes, `build_dockerfile_end_to_end` passes: session bidi stays
open, DiffCopy serves the build context, and `dockerfile.v0` solves.

### Red herrings (kept for archaeology)

These were all workarounds aimed at the misdiagnosed "grpc-go kills idle
transports" theory and are **not needed** (the keepalive thread and
pre-handshake PING were removed — the latter was itself a protocol violation:
the first frame the server preface allows is SETTINGS, so an early PING kills
the transport on its own):

| Approach | Result |
|----------|--------|
| Pre-handshake PING via bidi | Protocol violation — made things worse |
| `POLL_DELAY=1ms` during H2 handshake | Kept (harmless latency win) |
| Rapid 5ms keepalive PINGs | Removed — root cause was elsewhere |
| Dedicated send thread (`futures_lite::block_on`) | Kept (harmless latency win) |
| Minimal SETTINGS frame (0 values) | Kept (correct per RFC) |
| Health methods in session headers | Required regardless |
