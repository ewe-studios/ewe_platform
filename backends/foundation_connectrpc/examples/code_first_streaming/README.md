# Example: `code_first_streaming`

A **code-first server-streaming** service — defined from a Rust trait with
`#[service]` — round-tripped over a real socket. This is the streaming
counterpart to [`code_first_service`](../code_first_service), and it shows the one
place code-first streaming differs from what you write: the **boxed stream** return
shape.

```sh
cargo run -p foundation_connectrpc --example code_first_streaming
```

Expected output:

```
tick: seq=0
tick: seq=1
tick: seq=2
code-first server-streaming round-trip verified ✓ (3 ticks)
```

## The core issue: nested `impl Trait`

You *author* the streaming method the natural way:

```rust
use foundation_connectrpc::{service, Ctx, Request, ConnectResult};

#[service(package = "demo.tick.v1", codecs(json))]
pub trait TickService {
    async fn subscribe(&self, _ctx: Ctx, _req: Request<TickRequest>)
        -> ConnectResult<impl futures::Stream<Item = ConnectResult<Tick>>>;
}
```

But that exact shape is **not legal Rust to emit**: `impl Trait` nested inside
`ConnectResult<…>` (and, after the macro adds `-> impl Future<Output = …> + Send`,
nested inside *that*) is an error (`E0562: impl Trait not allowed in paths`). This
is why the macro cannot pass your signature through verbatim.

## What the macro generates instead

For server/bidi-streaming methods, the macro rewrites the return type to a
**boxed** stream — a concrete type, so nothing is `impl Trait` except the outer
future:

```rust
// generated trait method (conceptually):
fn subscribe(&self, ctx: Ctx, req: Request<TickRequest>)
    -> impl Future<
        Output = ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<Tick>> + Send>>>
    > + Send;
```

`Pin<Box<dyn Stream + Send>>` is `Stream + Send + 'static`, which is exactly what
`Router::server_stream` requires of the handler's stream.

## What you write in the impl

Because the generated trait returns a boxed stream, your implementation returns
`Ok(Box::pin(stream))`:

```rust
struct Ticker;
impl TickService for Ticker {
    async fn subscribe(&self, _ctx: Ctx, req: Request<TickRequest>)
        -> ConnectResult<Pin<Box<dyn Stream<Item = ConnectResult<Tick>> + Send>>> {
        let n = req.msg.count.max(0);
        let items: Vec<ConnectResult<Tick>> = (0..n).map(|i| Ok(Tick { seq: i })).collect();
        Ok(Box::pin(futures::stream::iter(items)))
    }
}
```

The one-line boxing (`Box::pin`) is the ergonomic cost of code-first streaming; in
exchange you get zero `.proto` and a fully typed client.

## Client side

The generated `TickServiceClient::subscribe` returns a `ServerStream<Tick>` you
drain exactly like the hand-wired [`server_streaming`](../server_streaming)
example:

```rust
let mut stream = client.subscribe(ctx, Request::new(TickRequest { count: 3 })).await?;
while let Some(tick) = stream.receive().await? { /* ... */ }
```

`codecs(json)` again means serde-only message types (no buffa), and the client
selects `json` (the default wire codec is `proto`).

## Things to notice

- This example is the acceptance proof for code-first **streaming**, which was a
  genuine Feature 27 gap: fixing it required three coordinated macro changes —
  reordering response-type extraction, fixing a latent stream-item extraction bug,
  and boxing the streaming return.
- Client-streaming and bidi follow the same boxed-return pattern for their response
  streams.

## Related examples

- [`code_first_service`](../code_first_service) — the unary code-first case and the
  full list of what the macro generates.
- [`server_streaming`](../server_streaming) — the same streaming RPC, hand-wired.
