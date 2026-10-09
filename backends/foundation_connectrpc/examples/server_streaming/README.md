# Example: `server_streaming`

A **server-streaming** RPC — one request in, a *stream* of responses out — served
and called over a real loopback socket. It is also a **JSON-only** service, so it
uses **no protobuf at all**.

```sh
cargo run -p foundation_connectrpc --example server_streaming
```

Expected output:

```
streamed: index=0 label="item-0"
streamed: index=1 label="item-1"
streamed: index=2 label="item-2"
server-stream round-trip verified ✓ (3 items)
```

## What it demonstrates

- A streaming handler that returns any `futures::Stream<Item = ConnectResult<Res>>`.
- Draining the response stream on the client with `stream.receive().await`.
- A **JSON-only codec table** — the message types are plain `serde` structs with no
  `buffa::Message` impl.

## Walkthrough

### JSON-only means serde-only

```rust
#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct CountRequest { count: i32 }

#[derive(Clone, Default, Debug, serde::Serialize, serde::Deserialize)]
struct CountItem { index: i32, label: String }
```

That's the *entire* type definition — no `Message`, no `DefaultInstance`, no
hand-written protobuf encoding. This works because the route registers a
**JSON-only** table:

```rust
ProcedureCodecs::<CountRequest, CountItem>::of((JsonCodec,))
```

`JsonCodec: CodecFor<M>` is bounded on `Serialize + DeserializeOwned` alone — *not*
`buffa::Message`. Contrast [`unary_echo`](../unary_echo), which uses `defaults()`
(proto + JSON) and therefore needs the buffa `Message` impl.

### The streaming handler

```rust
router.server_stream(
    "/demo.CountService/Count",
    ProcedureCodecs::<CountRequest, CountItem>::of((JsonCodec,)),
    |_ctx, req: Request<CountRequest>| async move {
        let n = req.msg.count.max(0);                 // floor a negative count to 0
        let items: Vec<ConnectResult<CountItem>> = (0..n)
            .map(|i| Ok(CountItem { index: i, label: format!("item-{i}") }))
            .collect();
        Ok(futures::stream::iter(items))              // return ANY Stream<Item = ConnectResult<Res>>
    },
    HandlerOptions::new(),
);
```

The handler returns `ConnectResult<S>` where `S: Stream<Item = ConnectResult<Res>>`.
Here it's a `futures::stream::iter`, but any stream works — the library frames each
item into the Connect streaming envelope (a 5-byte header: 1 flag byte + 4-byte
big-endian length) and terminates with an `EndStream` frame carrying trailers.

> `.max(0)` returns the **larger** of `count` and `0` (a floor), guarding against a
> negative request — it does *not* cap the count at 0.

### The client drains the stream

```rust
let mut stream = client.server_stream(ctx, Request::new(CountRequest { count: 3 })).await?;
while let Some(item) = stream.receive().await? {
    // handle each item until receive() yields None (EndStream)
}
```

Because this is a JSON-only route, the client must select `json` explicitly — the
client's **default wire codec is `proto`**, which this table doesn't contain:

```rust
Client::new(transport, &url, ProcedureCodecs::of((JsonCodec,)), ClientOptions::new().with_codec("json"))
```

## Things to notice

- **Half-duplex is fine over HTTP/1.1.** Server-streaming sends the whole request,
  then streams the response — no full-duplex needed. Full bidi streaming requires
  HTTP/2 or WebSocket (pending).
- Swapping `of((JsonCodec,))` for `defaults()` would re-introduce the proto codec
  and thus require a `buffa::Message` impl on both message types (see the
  [`codecs`](../codecs) example for the full matrix).

## Related examples

- [`unary_echo`](../unary_echo) — the single-message case (proto + JSON).
- [`code_first_streaming`](../code_first_streaming) — the same streaming RPC, but
  defined from a Rust trait via `#[service]`.
- [`codecs`](../codecs) — proto / json / arrow tables side by side.
