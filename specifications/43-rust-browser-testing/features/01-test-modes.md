# Feature 01 — Two test modes: native-App-streamed (Mode 1) + wasm-in-browser (Mode 2)

Status: DESIGN (2026-06-13). Both modes are supported; **Mode 1 is primary** for
component testing, **Mode 2** for full end-to-end. They share the one
`TestServer` transport built in phase 1.

## The fork

A `#[wasm_ui_server]` test drives a real browser. The question is *where the
`foundation_wasm_ui` App lives*:

| | **Mode 1 — native App, streamed** | **Mode 2 — wasm-in-browser** |
|--|--|--|
| App runs | the test process (native Rust) | the browser (wasm bundle) |
| Test drives state via | **signals directly** + CDP | CDP only (click/type) |
| Channel A (DOM-op stream) | server → browser (the App's output) | mostly unused (app self-drives) |
| `TestServer` serves | HTML shell + `foundation-wasm-ui.js` + the **stream route** | HTML shell + the **wasm bundle** (`StaticFileHandler`) + runtime |
| Who owns the App | **the test** (via the Harness) | the browser |
| Use case | the spec-42 catalog (white-box + black-box at once) | true end-to-end / hydration |

Mode 1 unifies our existing tests: today `App::mock()` buffers `DomOp`s into a
`Vec` and we assert "the op stream contains `SetAttribute data-open`". The SAME
App + component, run under `#[wasm_ui_server]` with `App::server()`, streams those
ops to a real browser and we assert "the browser actually shows the dialog and
traps focus." White-box control (set any signal) + black-box truth (real layout,
paint, focus, trusted input) in one test.

```rust
#[wasm_ui_server]
fn dialog_traps_focus(server: &TestServer, page: &Page) -> Result<()> {
    // The App lives in the test; its protocol sink streams frames to the page.
    let app = App::with_protocol(server.broadcast_sink());
    let (ctx, rcv) = app.context();
    let (open, set_open) = ctx.signal(false);
    app.mount(dialog(&ctx, &rcv, DialogConfig::default(), &open, set_open, slots));
    app.stabilize();                                // ship encoded frames → browser
    page.locator("dialog").expect().to_be_hidden()?;

    set_open.set(true);                             // drive a SIGNAL in Rust
    app.stabilize();                                // ship the update
    page.locator("dialog").expect().to_be_visible()?;   // assert the REAL browser
    Ok(())
}
```

## How Mode 1 works — the pipeline already exists

The `foundation_wasm_ui` building blocks are purpose-built for this:

- **`App::server() -> (App, CollectedFrames)`** — the doc calls it "the preset that
  makes server-driven UIs real": each `flush` lands an envelope-framed `Vec<u8>`
  in the returned queue, to "ship yourself (WebSocket binary frames, SSE events)
  toward a client `mount-stream`." (`App::server_with(encoder)` chooses the
  Layer-1 encoder — `JsonEncoder` for debuggable streams, Arrow, etc.)
- **`CollectedFrames = Rc<RefCell<VecDeque<Vec<u8>>>>`** — the encoded-frame queue;
  drain with `pop_front()`.
- **`App::with_protocol(server.broadcast_sink())`** — wires the App's protocol
  sink straight to the server's SSE broadcaster (the `BroadcastSink` adapter), so
  each `app.stabilize()` ships frames to the page; no `CollectedFrames` draining
  needed for streaming. `App::context()` then yields the `(Context,
  SharedInstructionReceiver)` the test mounts components + drives signals against.
- **`runtimes/foundation-wasm-ui.js`** — the BROWSER runtime that already connects
  to a stream (SSE-over-fetch *or* WebSocket, feature 21/11), decodes columnar/json
  frames, and applies `DomOp`s via morphdom. The HTML shell just loads it.

So the data path is:

```
test thread:  ctx/rcv ──signals/App::mount──▶ App ──stabilize()──▶ BroadcastSink (encodes frame)
                                                                          │  tx.send(frame)
                                                                          ▼
server thread:                                Broadcaster (Arc<Mutex<{streams, backlog}>>) ──sse.binary──▶
browser:                       foundation-wasm-ui.js  ◀──base64 SSE frames──  ensureApplicator → DomOps
```

## The `!Send` constraint (load-bearing)

`App`, `Context`, `SharedInstructionReceiver`, and `CollectedFrames` are **`Rc`-based
→ `!Send`** (the wasm-ui runtime is single-threaded by design). Therefore:

- The App is **created and driven on the test thread** (inside the test body, via
  `App::with_protocol(server.broadcast_sink())`). It NEVER moves to the
  `foundation_http` worker thread; only the `Send` `BroadcastTx` crosses over.
- The `foundation_http` server thread only touches the **`ContextBag`** (the
  `Broadcaster` + the page) — never the App.
- `app.stabilize()` runs on the **test thread**: the `BroadcastSink` (the App's
  protocol sink) encodes each batch and hands the frame to the `Send`
  `BroadcastTx`, which writes it to every connection held by the `Broadcaster`
  (and the backlog). The connections (SSE streams) live behind `Arc<Mutex<…>>`;
  the worker thread only ADDS them (on connect), the test thread WRITES (on
  stabilize). No `CollectedFrames` draining — the sink pushes directly.
- Consequence: `TestServer`/`Harness` are `!Send` and stay on the test thread — fine,
  since the body and setup both run there, and only the broadcaster crosses.

## The transport: `StreamHandler` + `Broadcaster` (channel A)

Built on the phase-1 `Serve`-owns-the-connection design:

```rust
struct Broadcaster {
    sinks: Arc<Mutex<Vec<StreamSink>>>,   // SSE/WS connections; Send
    backlog: Arc<Mutex<Vec<Vec<u8>>>>,    // all frames so far — replayed to late joiners
}

impl Serve for StreamHandler {
    fn serve(&self, bag, req, conn) -> ConnectionResult {
        let bc = bag.get::<Broadcaster>().unwrap();
        let mut sink = if is_websocket(&req) { accept_websocket(&req, conn); StreamSink::ws(conn) }
                       else { StreamSink::sse(SseStream::new(conn)?) };
        for frame in bc.backlog.lock().unwrap().iter() { sink.send(frame); }  // replay history
        bc.sinks.lock().unwrap().push(sink);
        ConnectionResult::Take                                                 // own the socket
    }
}
```

The **backlog + replay** removes the connect/stabilize race: the browser opens
the stream after page load, possibly *after* the first `app.stabilize()`; a late
stream is caught up by replaying the backlog (each frame re-sent via
`sse.binary`). A push appends to the backlog AND writes to every live stream.

`TestServer` API surface (Mode 1):

```rust
impl TestServer {
    pub fn app(&self) -> (Context, SharedInstructionReceiver);  // App::server(); store App+frames
    pub fn flush(&self);                                        // app.stabilize() + drain frames → broadcaster
}
```

## Mode 2 — wasm-in-browser

Falls out of phase 1's `StaticFileHandler`: serve the wasm bundle dir, the HTML
shell boots it, the app self-drives. The test only uses CDP.

```rust
#[wasm_ui_server(static_dir = "target/wasm")]
fn end_to_end(_server: &TestServer, page: &Page) -> Result<()> {
    page.locator("#open-dialog").click()?;
    page.locator("dialog").expect().to_be_visible()?;
    Ok(())
}
```

The wasm bundle comes from `foundation_wasm_testbed`'s build step; the same
`foundation-wasm-ui.js` runtime hydrates it.

## Build order

1. **Done (phase 1):** `TestServer` on `foundation_http` (`PageHandler` String/File
   + `StaticFileHandler` dir) — the transport; the CDP driver; the macro + Harness.
2. **Broadcaster + `StreamHandler`** route (`/__primal/stream`), backlog/replay,
   SSE first (simplest; WS next). `ConnectionResult::Take`.
3. **Done:** `App::with_protocol(server.broadcast_sink())` + `App::mount` +
   `app.stabilize()` — the `BroadcastSink` pushes encoded frames to the
   broadcaster directly (no `CollectedFrames` drain). App stays on the test
   thread; only the `Send` `BroadcastTx` crosses.
4. **HTML shell** that loads `runtimes/foundation-wasm-ui.js` and opens the stream
   via a single targetless `<mount-stream>` (the default `TestConfig` page in
   Mode-1 tests).
5. **First real component end-to-end:** a spec-42 `dialog`/`popover` test driven by
   Rust signals, asserted in a real browser.
6. **Mode 2** validated with a built wasm bundle (`StaticFileHandler`).

## Use the owned SSE APIs (don't hand-roll the wire)

The stack already has a clean SSE writer surface — USE IT rather than formatting
`data:` lines by hand:

- **`foundation_http::native::upgrade::SseStream`** — the server-side SSE
  connection. `SseStream::new(&mut conn)` writes the streaming response head
  (`200`, `text/event-stream`, `Cache-Control: no-cache`, `Connection: keep-alive`,
  NO `Content-Length`) and wraps the connection. Then:
  - `sse.message(data: impl Into<String>)` — a `data:`-only event.
  - `sse.send(&SseEvent)` — a full event (id / event-type / retry).
  - `sse.comment(&str)` — a `: …` keep-alive comment.
  Each call flushes. The handler returns `ConnectionResult::Take` after creating it.
- **`foundation_netio::event_source::EventWriter<W>`** — what `SseStream` wraps;
  use it directly to write SSE to any `W: Write`. Same `send`/`message`/`comment`,
  all flushing, correct multi-line `data:` framing.
- **`foundation_netio::event_source::SseEvent`** — the event builder:
  `SseEvent::message(data)`, plus `.id(..)`, `.event(type)`, `.data(..)` via its
  builder for structured events the browser routes on.
- **`foundation_netio::event_source::shared::response::SseResponse`** — a header
  builder for an SSE response (`with_header`/`with_status`) if you need the head
  separately. NOTE: its `build()` uses `SendSafeBody::None`, so it relies on the
  `Content-Length` fix below.
- **Client side** (for Rust SSE consumers): `foundation_netio::event_source`'s
  `SseStream::connect` / `SseParser` / the reconnecting task.

`StreamHandler` therefore just `SseStream::new(conn)?`, replays the backlog +
streams via **`sse.binary(frame)`**, and detaches. `SseStream::binary` is
foundation_http machinery (the server half of the binary-over-SSE contract): it
base64-encodes the frame so columnar/arrow bytes survive the text-only `data:`
line, pairing with the browser runtime's `streamEventResult("arrow", …)` which
`atob`s it back. The test holds NO bespoke encoding — the frame is a
self-describing `[protocol][version][length][..]` envelope; the transport just
moves bytes.

## Mounting: head AND bottom-of-body are first-class startup delivery

Two gaps surfaced wiring Mode 1 end-to-end; both are now machinery, not test code:

- **JS — the arrow stream installed no applicator.** `Patcher.route` for a
  `{type:"arrow"}` result called `applyDomOps`, but `Patcher.runtime.applicator`
  was never set, so the apply was a SILENT no-op (no error, empty DOM). Fixed
  with `Patcher.ensureApplicator(doc)` (called on the arrow path): it lazily
  builds a `DomOpApplicator` whose `NodeRegistry` is `seedDocument(doc)`-seeded,
  so the reserved ambient ids resolve — `RESERVED = {HEAD:0, BODY:1, HTML:2}`.
  (Note: this is the Patcher `NodeRegistry`, distinct from the megatron
  `DomHeap` whose reserved slots are self/heap/window/document/body.)
- **Rust — a built `html!` tree is DETACHED.** `html! { ctx, rcv, … }` creates
  and registers a subtree but never appends it to a parent, so nothing reaches
  the page. `App::mount(root)` queues the single `AppendChild` onto the reserved
  `BODY_NODE_ID` (1) — the body counterpart of `App::theme`, which injects into
  `HEAD_NODE_ID` (0). `AppendChild` is last-child semantics, so successive mounts
  land at the BOTTOM of `<body>` in order; that is how you place an element that
  must follow the body content, e.g. a `<script>` that runs once the body is
  parsed. The startup DOM-op stream thus covers head AND bottom-of-body.

With both in place a Mode-1 page is exactly what the user asked for: a single
`<mount-stream>` in the body with **no `target`** (the arrow protocol carries
absolute primal-ids and the App mounts its own root, so the mount self-replaces
and the App drives the document from there). No `#app` container, no manual
`AppendChild`, no bespoke base64 — every capability lives in `foundation_wasm_ui`
/ `foundation_http`.

## Protocol parity: a mount renders identically across wires

The App streams a `DomOp` batch; the wire encoding is an envelope detail, and a
mount must look identical whichever protocol it streamed over. The mount-stream's
`protocol="arrow"` means "base64 envelope frames" — `decodeEnvelopeFrame`
dispatches on the envelope's protocol byte, NOT the attribute:

- **Protocol 1 (custom binary columnar)** → `{type:"arrow", columns}` →
  `ensureApplicator` + `applyDomOps`. The default.
- **Protocol 1 v2 (Apache Arrow IPC)** → `{type:"arrow-ipc", table}`. Two gaps:
  `route` had no `arrow-ipc` case (fell through to `textContent`), and the reader
  needed hand-wired registration per page. Fixed: a `route("arrow-ipc")` case
  reads the table into a batch (`arrowTableToBatch`) through the SAME applicator,
  and `registerArrowIpc(arrow = globalThis.Arrow)` is the ONE framework call that
  wires `tableFromIPC` (the bundled `apache-arrow.js` sets `globalThis.Arrow`) —
  no per-page reader glue.
- **Protocol 2 (JSON)** — the `JsonEncoder` flat-row form
  (`[{op_id, node_id, operation, attribute, value, text_val}, …]`). A gap
  surfaced here: `routeJson` sent EVERY protocol-2 payload to the signal-patch
  bridge, so a JSON-encoded DomOp batch was dropped (no applicator, empty DOM).
  Fixed: `routeJson` now detects a DomOp-batch array (rows carry `operation`) and
  routes it through the SAME applicator (`jsonRowsToBatch` → `{type:"arrow"}`).
  Signal patches (`signalId`) and `{morph}` wrappers are unaffected. This is a
  decode-layer fix, so it holds for WS binary frames AND base64 SSE — any
  transport, test-server or real HTTP server.
- **HTML** is the non-DomOp delivery: a plain SSE text frame
  ([`BroadcastTx::send_text`]) routes through `routeHtml` — full markup
  (`materialize`) or, for an `<island>` wrapper, `html-morph` into a stable
  target. A different mount mode, not an App DomOp stream.

Coverage: the Rust browser e2e drives ALL FOUR protocols through real Chromium
over the real `foundation_http` server (`mode1_columnar` / `mode1_arrow_ipc` /
`mode1_json` / `mode1_html`), each with its own page `<title>`/`<h1>`, cycling a
shared `MESSAGES` set; JS unit tests assert the route+apply contract for protocol
1, protocol 1 v2 (Arrow IPC), protocol 2, the signal-patch regression, and HTML
materialize.

## Where the machinery lives (no test-only capabilities)

A standing rule: anything that makes a protocol work is a framework capability,
not test glue, so it works in a real server too.

- **`foundation_wasm_ui::server`** (native, target-gated like the CLI) owns the
  transport-AGNOSTIC server-driven-UI machinery: [`FrameTransport`] (a client
  connection trait), [`Broadcaster`]/[`BroadcastTx`] (fan-out + replayed
  backlog), and [`BroadcastSink`] (the App protocol sink). It depends on no
  socket/HTTP crate — connections are held as `Box<dyn FrameTransport + Send>`.
- **foundation_http** owns the SSE wire (`SseStream::binary` base64 envelope /
  `SseStream::message` text) AND a reusable [`StaticAssetHandler`] — supply bytes
  + content type + headers, register on a path. The embedded JS assets (the
  runtime + `apache-arrow.js`) are served through it; no per-asset `Serve` impl.
- **foundation_browser** keeps ONLY the glue: an `SseTransport(SseStream)` newtype
  (`impl FrameTransport`) + the `Serve` route, the page server, and the CDP
  driver.
- **`window_size = (800, 800)`** on `TestConfig` / the `#[wasm_ui_server]` macro
  sets the (headful) browser window via `--window-size`.

## The `Content-Length` fix (root cause, fixed at source)

The first cut hand-wrote the head because `SseStream` produced a broken response:
`SimpleOutgoingResponseBuilder::build()` AUTO-ADDED `Content-Length: 0` for an
absent body (`SendSafeBody::None`). A browser then reads zero bytes, treats the
SSE body as COMPLETE, and ignores every streamed `data:` line (`EventSource`
errors; `fetch().body` ends immediately).

Fixed at the source (`foundation_netio` `SimpleOutgoingResponseBuilder::build`):
an **absent body no longer auto-adds `Content-Length`** — a streaming response is
delimited by its framing / connection close, not a declared length. An
*explicitly*-set `Content-Length` (e.g. `SimpleOutgoingResponse::empty()`'s `0`)
is preserved. Bodies that ARE present (`Text`/`Bytes`) still get the correct
`Content-Length`. With that, `SseStream` works directly and we use it.

## `Take` already IS "take and owned"

The worry that `ConnectionResult::Take` would let the worker reap/close the socket
(motivating a `TakeAndOwned`) is unfounded: `SharedByteBufferStream` is `Arc`-shared
(`OwnedReader` over `Arc<RwLock<…>>`), so the worker dropping ITS handle on `Take`
leaves the broadcaster's `SseStream` clone holding the socket open. A handler can
detach (clone into the broadcaster, return `Take`) WITHOUT blocking a worker per
stream — no new variant required.

Verified end-to-end: a frame pushed from Rust (`server.push_frame`) reaches a real
browser's `EventSource` over SSE (base64-decoded in the page), the stream stays
open, and teardown drops the connections cleanly.

## Open decisions

- **SSE vs WS first:** SSE-over-fetch is simpler and the JS runtime supports it; WS
  gives lower latency + the coalescing path. Lean SSE for v1.
- **`flush()` explicit vs auto:** explicit `server.flush()` mirrors `app.stabilize()`
  (predictable in tests); an auto-flush-on-assert convenience can come later.
- **Frame encoder default:** `FrameSinkV1` (columnar) for fidelity, or `JsonEncoder`
  for debuggable streams in failing tests — possibly `TestConfig`-selectable.
