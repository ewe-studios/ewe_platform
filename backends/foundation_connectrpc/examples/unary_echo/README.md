# Example: `unary_echo`

The "hello world" of `foundation_connectrpc` — a single **unary** RPC (one request
message, one response message) served and called over a **real loopback TCP
socket**.

```sh
cargo run -p foundation_connectrpc --example unary_echo
```

Expected output:

```
unary echo OK: id=7 text="hello connectrpc"
round-trip verified ✓
```

## What it demonstrates

- Building a server: `Router` → `.unary(...)` → `ConnectRpcServe` → a
  `foundation_http` `HttpServer`.
- Building a client: `H1Transport` + a typed `Client<Req, Res>`.
- The full request/response round-trip over the wire (not a mock).
- Authoring a proto message type by hand (via the re-exported `buffa` surface).

## Walkthrough

### The message type

```rust
#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct EchoMessage { id: i32, text: String }
```

Because this example uses `ProcedureCodecs::defaults()` (which installs the
**proto** and JSON codecs), `EchoMessage` must implement `buffa::Message`. buffa
has no derive — those impls normally come from codegen — so the example hand-writes
a minimal `Message` (field 1 = `id`, field 2 = `text`) plus `DefaultInstance`.
Everything comes from the **re-exported** buffa surface, so the example needs no
direct `buffa` or `bytes` dependency:

```rust
use foundation_connectrpc::buffa::bytes::{Buf, BufMut};
use foundation_connectrpc::buffa::encoding::{decode_varint, encode_varint, ...};
use foundation_connectrpc::buffa::{DefaultInstance, Message, SizeCache, ...};
```

> `DefaultInstance::default_instance()` returns `&'static Self`, so it needs a
> value that lives forever — hence the `OnceLock`. (For a const-constructible type
> a plain `static` also works; codegen always emits the `OnceLock` form.)

### The server

```rust
let mut router = Router::new();
router.unary(
    "/demo.EchoService/Echo",                                 // procedure path (leading slash, R1)
    ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),  // proto + JSON
    |_ctx, req: Request<EchoMessage>| async move { Ok(Response::new(req.msg)) },
    HandlerOptions::new().with_idempotency(IdempotencyLevel::NoSideEffects),
);
Arc::new(ConnectRpcServe::new(router.into_handler()))
```

The handler is a plain async closure `Fn(Ctx, Request<Req>) -> ConnectResult<Response<Res>>`.
`Ctx` is passed **by value** (owned, cheap-`Clone`), and the signature returns
`ConnectResult<T>`.

The `HttpServer` runs its blocking accept loop on its own OS thread. That
per-connection task is the **server-side connection owner** (Decision 11): it
reads the request off the wire, dispatches through the router, and writes the
response back.

### The client

```rust
let transport: Arc<dyn Transport> = Arc::new(H1Transport::new(SimpleHttpClient::from_system()));
let client: Client<EchoMessage, EchoMessage> = Client::new(
    transport,
    "http://127.0.0.1:<port>/demo.EchoService/Echo",  // base URL + procedure path
    ProcedureCodecs::<EchoMessage, EchoMessage>::defaults(),
    ClientOptions::new(),                             // default protocol = Connect, default codec = proto
)?;

let ctx = Ctx::background().with_deadline(Duration::from_secs(10));
let resp = client.unary(ctx, Request::new(EchoMessage { id: 7, text: "hello connectrpc".into() })).await?;
```

`H1Transport::open` is the **client-side connection owner**: it spawns the byte
pump on the valtron pool and hands back the pipe halves, so a large request never
deadlocks a bounded pipe. The high-level `Client` sits on top and drives the
Connect protocol (content-type negotiation, envelope handling, error decoding).

### The entry point

```rust
#[valtron(seed = 1, threads = 4)]
async fn main() { ... }
```

`#[valtron]` is the engine equivalent of `#[tokio::main]` — it stands up the
valtron pool around the async body (driven to completion via `block_on_future`)
and tears it down after. The blocking `HttpServer` runs on a separate OS thread so
the client task can make progress concurrently.

## Things to notice

- **`proto` is the default wire codec.** `defaults()` registers both proto and
  JSON; the client uses `proto` unless you call `.with_codec("json")`. That's why
  `EchoMessage` needs a real (non-stub) `Message` impl — the proto encoding is
  what's actually on the wire here.
- This is the first end-to-end proof of the high-level `Client` + `H1Transport` +
  `ConnectRpcServe` path working together over a real socket.

## Related examples

- [`server_streaming`](../server_streaming) — a stream of responses, JSON-only.
- [`codecs`](../codecs) — every codec table (proto/json/arrow) in one program.
- [`code_first_service`](../code_first_service) — define the service from a Rust
  trait instead of a hand-written router.
