# Server, protocols & fullstack

The same `App` runs on a native server. This doc covers the wire protocols, the
`App` presets, first-paint HTML, live server-driven UI, the `server` fan-out
module, and live-browser testing.

## Wire protocols

Every flush encodes a `DomOp` batch into an **envelope**: `[protocol:1]
[version:1][length:4 LE][payload]`. The envelope is self-describing — the receiver
dispatches on the protocol byte, so the same mount renders identically whichever
wire it streamed over.

| protocol | byte/version | what it is |
|----------|--------------|------------|
| `ColumnarV1` | byte 1, v1 | **default** — compact columnar layout: fixed-width columns + a shared text pool, 8-byte aligned so JS reads columns as `TypedArray` views over live WASM memory (zero-copy) |
| `ArrowIpcV2` (`arrow` feature) | byte 1, v2 | real Apache Arrow IPC `RecordBatch`es; read in JS via `apache-arrow.js` |
| `JsonV1` | byte 2 | human-readable `DomOp`s — debugging |
| `MockProtocol` | byte 255 | test double — records every flushed batch |

Pick via an `App` preset or the builder:

```rust
use foundation_wasm_ui::App;
let app = App::new();          // ColumnarV1 (default)
let app = App::json();         // JsonV1
let (app, sent) = App::mock(); // sent: Rc<RefCell<Vec<Vec<DomOp>>>>
# #[cfg(feature = "arrow")]
let app = App::arrow();        // ArrowIpcV2
```

### Performance (measured, `wire_bench`)

| batch | compact columnar (v1) | Apache Arrow IPC (v2) |
|-------|------------------------|------------------------|
| 10 DomOps | 674 ns enc · 344 B | 5.3 µs · 2378 B |
| 1,000 DomOps | 28 µs enc · 30 KB | 45 µs enc · 33 KB |
| 10,000 DomOps | 259 µs · 311 KB | 430 µs · 320 KB |

Compact columnar dominates the loop's real traffic (small UI deltas) by ~7× on
latency and bytes; Arrow IPC pays a ~2 KB framing floor per message but converges
at bulk sizes. Hence the defaults: columnar for the loop, Arrow for bulk/interop.

## First-paint HTML — no runtime at all

The pure form + `to_markup()` serialize the same trees that drive the DomOp
channel (one renderer, slot spans and all):

```rust
fn page(user: &str) -> String {
    let body = html! {
        <main class="app">
            <h1>{user}</h1>
            <mount-stream api="/live" target="#feed"></mount-stream>
            <div id="feed"></div>
        </main>
    };
    body.to_markup()   // serve as text/html
}
```

Escaping, void elements, and the slot-`<span>` shape are handled; no signals, no
`App`, no allocation arena.

## Live server-driven UI

A native server runs the full reactive loop and ships each flushed batch as wire
frames (on native, `host_apply` is a no-op stub, so the server presets capture
frames instead of dropping them).

### Option A — collect frames yourself (`App::server()`)

```rust
let (app, frames) = App::server();        // compact columnar wire v1
let (ctx, receiver) = app.context();
let (status, set_status) = ctx.signal(String::from("ready"));
app.mount(html! { ctx, receiver, <p class="status">{status.get()}</p> });
app.stabilize();

while let Some(frame) = frames.borrow_mut().pop_front() {
    // WebSocket: send as ONE BINARY frame (the envelope byte tells the client what
    // it is). SSE: base64 it. Same frame either way.
    // websocket.send_binary(&frame);
}
```

`App::server_with(JsonEncoder)` streams readable `primal-json`;
`App::server_with(foundation_arrow::ArrowIpcEncoder)` streams real Arrow IPC.
**One `App` per connection/session**; drop it when the connection closes (the root
scope disposes every effect).

### Option B — fan out to many clients (`server` module)

`foundation_wasm_ui::server` (native) is the broadcaster: a protocol-agnostic
fan-out with a replayed backlog so late joiners catch up. It's transport-agnostic —
it holds `Box<dyn FrameTransport + Send>`, so it depends on no socket/HTTP crate;
your server provides the `FrameTransport` impl (SSE, WebSocket, …).

```rust
use foundation_wasm_ui::server::{Broadcaster, BroadcastSink, FrameTransport};

let broadcaster = Broadcaster::new();
let app = App::with_protocol(BroadcastSink::new(broadcaster.sender())); // columnar
// BroadcastSink::with_encoder(JsonEncoder, tx) / (ArrowIpcEncoder, tx) for others
let (ctx, rcv) = app.context();

app.mount(html! { ctx, rcv, <p id="status">{status.get()}</p> });
app.stabilize();   // BroadcastSink encodes the batch → broadcaster → every client
```

Your transport implements `FrameTransport`:

```rust
struct SseTransport(MySseStream);
impl FrameTransport for SseTransport {
    fn send_binary(&mut self, frame: &[u8]) -> bool { self.0.send_base64(frame).is_ok() }
    fn send_text(&mut self, text: &str) -> bool     { self.0.send_line(text).is_ok() }
}
// on connect: broadcaster.register(SseTransport(stream));
```

- `BroadcastTx::send(&[u8])` — a binary (envelope) frame: columnar/arrow/json.
- `BroadcastTx::send_text(&str)` — raw markup: the HTML protocol (`routeHtml`).

## The browser side

A single `<mount-stream>` + the runtime. No app JS:

```html
<mount-stream api="/stream" transport="sse" protocol="arrow"></mount-stream>
<script type="module">
  import { registerWebComponents, registerArrowIpc } from '/foundation-wasm-ui.js';
  registerWebComponents();
  // registerArrowIpc();   // only for the Apache Arrow IPC wire (after apache-arrow.js)
</script>
```

`protocol="arrow"` means "base64 envelope frames" — `decodeEnvelopeFrame`
dispatches on the actual protocol byte (columnar, json, and arrow-ipc all decode).
Omit `protocol` for plain-text HTML frames. See **[internals](./internals.md)** for
the web-component attributes.

## Live-browser testing

`foundation_browser` drives a real Chromium over CDP (pure Rust, no Playwright).
The `#[wasm_ui_server]` macro boots a server + browser around a test body:

```rust
#[foundation_browser::wasm_ui_server(window_size = (800, 800))]
fn dialog_opens(server: &TestServer, page: &Page) -> Result<()> {
    let app = App::with_protocol(server.broadcast_sink());
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    app.mount(dialog(&ctx, &rcv, /* … */));
    app.stabilize();
    page.locator("dialog").expect().to_be_hidden()?;

    set_open.set(true);          // drive a signal in Rust
    app.stabilize();
    page.locator("dialog").expect().to_be_visible()?;   // assert the REAL browser
    Ok(())
}
```

Set `PRIMAL_TEST_HEADFUL=1` to watch it run. See spec-43 for the full harness.

See also: **[getting started §8](./getting-started.md)** · **[internals](./internals.md)**.
