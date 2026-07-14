# BuildKit Session Protocol — Deep Dive

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

When buildkitd receives the `Control/Session` bidi, its `grpcClientConn()`
([session/grpc.go](https://github.com/moby/buildkit/blob/master/session/grpc.go)):

7. Wraps the bidi `net.Conn` in a gRPC `*grpc.ClientConn` (HTTP/2, insecure,
   targeting dummy `"localhost"`).
8. Starts **`monitorHealth`** — calls `/grpc.health.v1.Health/Check` every 5s
   with a threshold of 2 failures. If it fails, the session context is cancelled
   → the Solve fails → the connection closes.
9. Routes RPCs through the H2 connection. buildkitd's `dockerfile.v0` frontend
   calls `FileSync/DiffCopy` to stream our build context.

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
    on stream 1.
14. Our `HealthService` returns `SERVING`. Without this, `monitorHealth` fails
    → session context cancelled → Solve killed at ~20ms with "context canceled".

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

---

## `grpc.health.v1.Health` — required, not optional

buildkitd's `grpcClientConn` runs `monitorHealth` in a goroutine:
```go
go monitorHealth(ctx, cc, cancel)
```

`monitorHealth` calls `/grpc.health.v1.Health/Check` every 5 seconds. After 2
consecutive failures, it cancels the session context. The session connection
closes ~20ms after the handshake if the health check fails — that's not a
timeout; it's an immediate cancellation on first failure.

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

---

## Running the tests

```sh
# Unit test: FileSync DiffCopy (no buildkitd needed)
cargo test -p foundation_deployment_docker --features buildkit \
    --profile uat --test buildkit_filesync_server_test -- --test-threads=1

# Integration test: full Solve against real buildkitd
docker run -d --name ewe-buildkitd --privileged -p 127.0.0.1:13434:1234 \
    moby/buildkit:latest --addr tcp://0.0.0.0:1234

cargo test -p foundation_deployment_docker --features "buildkit,integration-tests" \
    --profile uat --test buildkit_integration_tests build_dockerfile_end_to_end \
    -- --nocapture --test-threads=1

# docker logs ewe-buildkitd 2>&1 | grep -E "error|session|trace"
```

With `RUST_LOG=debug`, the H2 connection handler logs every frame type on
receive, route dispatch decisions, and handshake completion. The session pump
uses `eprintln!` for raw frame hex dumps (these are loud; in production they
move to `tracing::trace!`).
