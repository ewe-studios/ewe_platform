# F010: Workers Durable Objects & WebSocket Machinery

**Status:** ✅ Complete — all 3 stages (general DO machinery + WebSocket + SignalR hub DO)

**Depends on:** none (parallel; needs `worker` crate + `foundation_netio` for framing reuse)
**Unblocks:** keychain SignalR hub (008 Stage 3 remainder), any future Workers crate
**Decisions:** [09](../../decisions/09-workers-runtime-module.md)

## WHY

`foundation_deployment_cloudflare::workers/` has four empty stubs (`durable_object.rs`,
`websocket.rs`, `env.rs`, `context.rs`). The keychain (008 Stage 3) needs a
SignalR Durable Object, but:

- The DO machinery (`DurableObject` trait helpers, `State` + `Env` interop,
  hibernation WebSocket accept pattern) is **general** — any crate deploying to
  Workers would use it.
- WebSocket-Pair accept with SignalR framing reuse (`foundation_netio::websocket::shared`
  binary/text frame encode/decode, per decision 09) is also **general**.
- `Env` binding helpers (D1, KV, R2, Secrets, Queues) are the standard glue every
  Workers crate needs.

Putting this in `foundation_deployment_cloudflare` means `foundation_keychain`,
`foundation_auth`, and future crates all share one thin transport layer. The
keychain-specific SignalR hub (`SignalRHub: DurableObject`) is the **consumer** —
it lives in `foundation_keychain::server::signalr_do` and imports the general
machinery from `foundation_deployment_cloudflare::workers`.

## WHAT

`foundation_deployment_cloudflare::workers` becomes a real module (no more empty stubs):

### Stage 1 — General DO machinery

**`durable_object.rs`**: trait extension and helpers on top of workers-rs `DurableObject`.

- `WebSocketHub` trait: a higher-level DO pattern for WebSocket hubs (accept upgrade,
  track connected sockets, broadcast, idle GC via alarm). This is the generalised
  version of the keychain `SignalRHub`;
  `foundation_keychain::server::signalr_do` implements it.
- `WebSocketHibernation` helper: accept `WebSocketPair`, register with DO state for
  hibernation-aware keep-alive, auto-accept with optional tags (see
  `state.accept_websocket_with_tags`).
- `WebSocketRegistry`: `send_to_all(tag, bytes)` + `send_to_tagged(tag, bytes)` —
  thin wrapper around `state.get_websockets()` / `state.get_websockets_with_tag(tag)`.
  Keeps the DO trait implementation concise.
- `do_connect(ws: WebSocket, tags: &[&str])`: accept + tag in one call.
- `do_broadcast(state: &State, bytes: &[u8], tag: Option<&str>)`: send to all sockets
  (optionally filtered by tag).

**`env.rs`**: typed binding helpers.

- `get_d1(env: &Env, name: &str) -> Result<D1Database>` — already in keychain
  `cloudflare.rs`, generalise here so the keychain just calls it.
- `get_kv(env: &Env, name: &str) -> Result<KvStore>`
- `get_r2(env: &Env, name: &str) -> Result<R2Bucket>`
- `get_secret(env: &Env, name: &str) -> Result<String>`

**`context.rs`**: `RouteContext` helpers (for non-DO Workers routes).

- `with_db<T>(ctx: &RouteContext<D1Database>, f: impl FnOnce(&D1Database) -> T) -> T`

### Stage 2 — WebSocket accept + SignalR framing reuse

**`websocket.rs`**: `WebSocketPair` accept + framing bridges.

- `accept_websocket(state: &State, tags: &[&str]) -> Result<Response>` — creates a
  `WebSocketPair`, accepts the server side on the DO state (so events fire), returns
  the client side wrapped in a 101 `Response` with CORS headers.
- `send_frame(ws: &WebSocket, packet_type: WsPacketType, data: &[u8])`
- `close_socket(ws: &WebSocket, code: u16, reason: &str)`
- `SignalrHandshaker`: reusable SignalR handshake state machine (JSON `\x1E`-terminated
  handshake → accept → VarInt/MessagePack binary frames). Used by both the native
  WebSocket server (decision 04) and the Workers DO.
- The actual MessagePack framing codec (VarInt length prefix + `rmpv` encoding) is
  already portable in `foundation_keychain::core::notifications`. This module only
  does the **transport** layer (WS accept/send/recv).

### Stage 3 — Keychain SignalR hub (consumer)

In `foundation_keychain::server/signalr_do.rs`:

- `SignalRHub` struct implements `#[durable_object]` + `DurableObject`.
- Uses `foundation_deployment_cloudflare::workers::durable_object::WebSocketHub` +
  `websocket::SignalrHandshaker`.
- DO keyed by user UUID; all of a user's devices connect to the same DO instance.
- Alarm every 15s → ping all sockets via `do_broadcast(state, &[6], None)`.
- `POST /notify` → broadcast a SignalR invocation frame to all sockets.
- `GET /ws` → WebSocket upgrade via `accept_websocket`.

## HOW

### Cargo changes

`foundation_deployment_cloudflare/Cargo.toml`:
```toml
# New: Workers runtime support (wasm-only, gated).
[target.'cfg(target_family = "wasm")'.dependencies]
worker = { version = "0.8.3", features = ["d1"], optional = true }
wasm-bindgen = "0.2"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["WebSocket", "Response"] }
serde = { workspace = true }
serde_json = { workspace = true }

[features]
workers = ["dep:worker"]   # NEW feature
```

`lib.rs` already gates `pub mod workers` on `wasm + workers` — no change needed
except adding the `workers` feature to the `[features]` table.

### Layering

```
foundation_deployment_cloudflare::workers
├── durable_object   (WebSocketHub trait, WebSocketRegistry, do_connect, do_broadcast)
├── websocket        (WebSocketPair accept, SignalrHandshaker, send_frame, close_socket)
├── env              (get_d1, get_kv, get_r2, get_secret)
└── context          (RouteContext helpers)

foundation_keychain::server::signalr_do
├── SignalRHub: DurableObject + WebSocketHub
├── Uses workers::durable_object (WebSocketRegistry, do_broadcast)
├── Uses workers::websocket (accept_websocket, SignalrHandshaker)
└── Uses core::notifications (MessagePack framing — portable)
```

### Why not in foundation_keychain?

The DO machinery is Cloudflare Workers infrastructure. Any crate that deploys to
Workers — `foundation_auth` (IdP), `foundation_keychain` (vault), future
`foundation_dashboard` — needs the same WebSocket hub pattern. Centralising it
prevents duplication and keeps the keychain module focused on Bitwarden business
logic, not Workers runtime glue.

## Task list

1. Add `workers` feature to `foundation_deployment_cloudflare/Cargo.toml` with
   `worker`, `wasm-bindgen`, `js-sys`, `web-sys` deps (wasm-target-only).
2. Implement `durable_object.rs`: `WebSocketHub` trait, `WebSocketRegistry`,
   `do_connect`, `do_broadcast`.
3. Implement `websocket.rs`: `accept_websocket`, `SignalrHandshaker`, `send_frame`,
   `close_socket`.
4. Implement `env.rs`: `get_d1`, `get_kv`, `get_r2`, `get_secret`.
5. Implement `context.rs`: `with_db` and any other `RouteContext` helpers.
6. Compile-check `foundation_deployment_cloudflare` on wasm32 with `workers` feature.
7. Implement `foundation_keychain::server::signalr_do.rs` (SignalRHub) on top of
   stages 1-2.
8. Add `[[durable_objects]]` binding to keychain `wrangler.toml`. Compile-check
   `worker-build --release`. Validate DO boots with `wrangler dev`.
9. Tests: `durable_object.rs` unit tests (WebSocketRegistry, SignalrHandshaker state
   machine); wasm32 integration test with `wasm-bindgen-test` if a test harness is
   available (otherwise deferred).

## Test plan / success

- `cargo check -p foundation_deployment_cloudflare --target wasm32-unknown-unknown --features workers` green.
- `worker-build --release` in `foundation_keychain` produces `< 5 MB`.
- `wrangler dev` boots the DO; WebSocket upgrade returns 101; SignalR handshake
  completes; notification broadcast reaches all connected sockets.
- Unit tests for `WebSocketRegistry`, `SignalrHandshaker`, VarInt codec pass.

## Related

- [[project_spec41_f51_websocket_unified]] — foundation_netio WebSocket (framing reuse)
- Decision 09 — Workers WebSocket framing reuse
- Decision 04 — native notifications via foundation_netio
- [[project_spec57_foundation_keychain]] — first consumer (keychain SignalR hub)
