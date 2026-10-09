# Example: `codecs`

Every codec combination, over one server. One echo handler is registered five
times, each under a different `ProcedureCodecs` table, and the client calls each
selecting the matching wire codec — all verified over a real socket.

> Requires the `arrow` feature (on by default via `rpc`).

```sh
cargo run -p foundation_connectrpc --example codecs
```

Expected output:

```
codec combinations, each verified over a real socket:
  proto → .../ProtoJson  (value=1)
  json  → .../Json       (value=2)
  arrow → .../Arrow      (value=3)
  arrow → .../ArrowJson  (value=4)
  proto → .../All        (value=5)
  json  → .../All        (value=6)
  arrow → .../All        (value=7)
all codec combinations verified ✓
```

## Background: the codec table is the authority

The per-procedure `ProcedureCodecs` table is the **single codec authority**
(Decision 02). There is no request-time registry: the codecs a procedure accepts
are fixed at registration, and **codec names are the wire Content-Type tokens**
(`proto` → `application/proto` / `application/connect+proto`, `json` →
`application/json` / `…+json`, `arrow` → `application/connect+arrow`). The client
picks one registered name per call.

## The three codecs and their type bounds

| Codec | Wire name | `CodecFor<M>` bound |
|---|---|---|
| `ProtoCodec` | `proto` | `M: buffa::Message` |
| `JsonCodec` | `json` | `M: Serialize + DeserializeOwned` (serde only) |
| `ArrowCodec` | `arrow` | `M: ToArrow + FromArrow` |

To be servable by *all three*, a message must be **tri-family** — a buffa `Message`
(+ serde) that is also `ToArrow + FromArrow`. This example's `Sample { value: i32 }`
is exactly that: a hand-written `Message` (field 1 = `value`), the automatic serde
derives, and a one-column Arrow schema (`value: int32`).

## The five combinations

| Procedure | Table expression | Bound reached | Notes |
|---|---|---|---|
| **proto + json** | `ProcedureCodecs::defaults()` | `Message + serde` | The codegen default. `proto` is the default wire codec. |
| **json** | `ProcedureCodecs::of((JsonCodec,))` | serde only | No buffa, no protobuf — see [`server_streaming`](../server_streaming). |
| **arrow** | `ProcedureCodecs::only(ArrowCodec)` | `ToArrow + FromArrow` | Single-codec endpoint; columnar Arrow IPC on the wire. |
| **arrow + json** | `ProcedureCodecs::of((JsonCodec, ArrowCodec))` | serde + Arrow | Two codecs; the client chooses per call. |
| **proto + json + arrow** | `ProcedureCodecs::of((ProtoCodec, JsonCodec, ArrowCodec))` | all three | The full tri-family table. |

`of(..)` takes a tuple of concrete codecs (each independently `CodecFor<Req> +
CodecFor<Res>`), keeping every element's type. `only(c)` is sugar for `of((c,))`.

### Server: one handler, five tables

```rust
async fn echo(_ctx: Ctx, req: Request<Sample>) -> ConnectResult<Response<Sample>> {
    Ok(Response::new(req.msg))
}

router.unary(P_PROTO_JSON, ProcedureCodecs::<Sample, Sample>::defaults(), echo, opts());
router.unary(P_JSON,       ProcedureCodecs::<Sample, Sample>::of((JsonCodec,)), echo, opts());
router.unary(P_ARROW,      ProcedureCodecs::<Sample, Sample>::only(ArrowCodec), echo, opts());
router.unary(P_ARROW_JSON, ProcedureCodecs::<Sample, Sample>::of((JsonCodec, ArrowCodec)), echo, opts());
router.unary(P_ALL,        ProcedureCodecs::<Sample, Sample>::of((ProtoCodec, JsonCodec, ArrowCodec)), echo, opts());
```

`echo` is a plain `async fn` (a `Fn` item, hence `Copy`) reused across all five
registrations.

### Client: select the wire codec per call

The client's default wire codec is `proto`. For any procedure whose table lacks
`proto`, or to exercise a specific codec, select it with `.with_codec(..)`:

```rust
Client::new(transport, &url, codecs, ClientOptions::new().with_codec("arrow"))
```

The `proto + json + arrow` procedure is called **three times** — once per codec —
to show a single endpoint answering `proto`, `json`, and `arrow` requests
interchangeably. A request for an unregistered codec (e.g. `arrow` against the
JSON-only endpoint) would be rejected with HTTP 415 and the supported-names list,
never a silent fallback.

## The Arrow message impl

Arrow support is the only part that needs more than derives. `Sample` implements:

- `ArrowSchema` — the record-batch schema (`value: int32`, non-null);
- `ToArrow` — build a one-row (and a batched) `RecordBatch`;
- `FromArrow` — read the value back out of the `Int32Array` column.

These come from `foundation_arrow`. In a real project a derive/codegen would emit
them; the example writes them out so the Arrow IPC round-trip is fully visible.

## Related examples

- [`unary_echo`](../unary_echo) — proto + JSON, with the full walkthrough.
- [`server_streaming`](../server_streaming) — the JSON-only path in isolation.
