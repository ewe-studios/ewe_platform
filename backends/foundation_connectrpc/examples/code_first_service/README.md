# Example: `code_first_service`

Define a ConnectRPC service **from a Rust trait** — no `.proto` file — using the
`#[service]` attribute macro (Decision 10 Mode 3, Feature 27), then
round-trip a **unary** call over a real socket.

```sh
cargo run -p foundation_connectrpc --example code_first_service
```

Expected output:

```
greet response: "Hello, code-first!"
code-first #[service] round-trip verified ✓  (service = demo.greet.v1.GreetService)
```

## What it demonstrates

- The `#[service]` macro turning a trait into a full service surface.
- A `codecs(json)` service that needs **only serde** — no protobuf, no buffa.
- Wiring the *generated* server registration + typed client together.

## Walkthrough

### The macro input

```rust
use foundation_connectrpc::{service, Ctx, Request, Response, ConnectResult};

#[service(package = "demo.greet.v1", codecs(json))]
pub trait GreetService {
    async fn greet(&self, _ctx: Ctx, _req: Request<GreetRequest>)
        -> ConnectResult<Response<GreetResponse>>;
}
```

The generated code refers to the runtime crate by its **real name**
(`foundation_connectrpc::…`), so there is **no crate alias** — just import the
`service` macro and the types you use. (The macro classifies each method by the
`Request` / `Response` / `ConnectResult` / `Stream` idents, so bare imported type
names work fine.)

### What the macro generates

From that one trait, `#[service]` emits:

| Item | Purpose |
|---|---|
| `pub const GREETSERVICE_NAME` | Fully-qualified service name (`demo.greet.v1.GreetService`). |
| `pub mod procedure { pub const GREET: &str = ...; }` | Procedure path constants (leading slash). |
| the `GreetService` trait | With default `unimplemented` bodies (implement incrementally, tonic-style). |
| `register_greet_service(&mut Router, Arc<S>)` | Registers every method onto a router. |
| `struct UnimplementedGreetServiceHandler` | A no-op impl for stubbing. |
| `struct GreetServiceClient` + `GreetServiceClientExt` | A typed client + a trait for mocking. |

### The `+ Send` desugar (why the macro rewrites your signature)

`Router::unary` bounds the handler future as `Future + Send + 'static`. A native
`async fn` in a trait produces a **non-`Send`** `impl Future`, so a naive
generated trait would make `register_greet_service` fail to compile with *"future
cannot be sent between threads safely."* The macro therefore desugars each method
to `-> impl Future<Output = ...> + Send`, making every implementation's future
`Send` by contract. You still write a normal `async fn` in your impl:

```rust
struct Greeter;
impl GreetService for Greeter {
    async fn greet(&self, _ctx: Ctx, req: Request<GreetRequest>)
        -> ConnectResult<Response<GreetResponse>> {
        Ok(Response::new(GreetResponse { message: format!("Hello, {}!", req.msg.name) }))
    }
}
```

### Serde-only messages

```rust
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
pub struct GreetRequest { name: String }
```

A `codecs(json)` service generates `ProcedureCodecs::of((JsonCodec,))`, which is
bounded on serde alone — so **no `buffa::Message`, no protobuf**. (A `codecs(proto,
…)` service would additionally need buffa-generated `Message` impls, since proto
requires a schema.)

> The message structs are `pub` because the generated `pub trait`/client mention
> them in their signatures — they must be at least as visible.

### Wiring it up

```rust
// server
let mut router = Router::new();
register_greet_service(&mut router, Arc::new(Greeter));
Arc::new(ConnectRpcServe::new(router.into_handler()))

// client (JSON-only table → select "json")
let client = GreetServiceClient::new(transport, &base_url, ClientOptions::new().with_codec("json"))?;
let resp = client.greet(ctx, Request::new(GreetRequest { name: "code-first".into() })).await?;
```

The generated `GreetServiceClient::new` takes a **base URL** and appends each
procedure path internally, so one client covers the whole service.

## Things to notice

- This example is the acceptance proof for Feature 27's unary path — the macro was
  previously only token-tested and did **not** compile against a real registration
  until the `+ Send` desugar landed.
- Cross-crate generation (`generate!`) works the same way via an exported
  descriptor macro — see Decision 10 §Cross-crate generation.

## Related examples

- [`code_first_streaming`](../code_first_streaming) — the streaming counterpart,
  and the boxed-stream return shape the macro generates for it.
- [`unary_echo`](../unary_echo) — the same unary RPC, hand-wired without the macro.
