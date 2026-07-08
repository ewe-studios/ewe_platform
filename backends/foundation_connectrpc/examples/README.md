# foundation_connectrpc examples

Each example is a self-contained program in its own directory (`<name>/main.rs`)
with a `README.md` that explains it in depth. Every one stands up a server and
calls it over a **real loopback TCP socket** — nothing is mocked.

Run any of them with:

```sh
cargo run -p foundation_connectrpc --example <name>
```

| Example | RPC kind | Codecs | Definition style | What it teaches |
|---|---|---|---|---|
| [`unary_echo`](unary_echo) | unary | proto + json | hand-wired router | The end-to-end spine: server, client, `Ctx`, connection ownership, authoring a `buffa::Message`. |
| [`server_streaming`](server_streaming) | server-stream | json only | hand-wired router | Returning a `Stream`, draining it, and a serde-only (no protobuf) service. |
| [`codecs`](codecs) | unary | **all combinations** | hand-wired router | proto+json, json, arrow, arrow+json, proto+json+arrow — the codec table as the single authority. |
| [`code_first_service`](code_first_service) | unary | json | `#[service]` | Generating a service + typed client from a Rust trait (no `.proto`). |
| [`code_first_streaming`](code_first_streaming) | server-stream | json | `#[service]` | Code-first streaming and the boxed-stream return shape. |

## Suggested reading order

1. **`unary_echo`** — the foundation everything else builds on.
2. **`server_streaming`** — add streaming and drop protobuf.
3. **`codecs`** — see every codec table side by side.
4. **`code_first_service`** → **`code_first_streaming`** — trade the hand-wired
   router for a trait + macro.
