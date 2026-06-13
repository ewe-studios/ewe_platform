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
    let (ctx, rcv) = server.app();                  // App::server() context; rcv → browser
    let (open, set_open) = ctx.signal(false);
    let _ui = dialog(&ctx, &rcv, DialogConfig::default(), &open, set_open, slots);
    server.flush();                                 // stabilize + ship encoded frames
    page.locator("dialog").expect().to_be_hidden()?;

    set_open.set(true);                             // drive a SIGNAL in Rust
    server.flush();                                 // ship the update
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
- **`App::context() -> (Context, SharedInstructionReceiver)`** — what `server.app()`
  returns; the test mounts components + drives signals against it.
- **`runtimes/foundation-wasm-ui.js`** — the BROWSER runtime that already connects
  to a stream (SSE-over-fetch *or* WebSocket, feature 21/11), decodes columnar/json
  frames, and applies `DomOp`s via morphdom. The HTML shell just loads it.

So the data path is:

```
test thread:  ctx/rcv  ──signals/mount──▶  App  ──flush()──▶  CollectedFrames (Vec<u8> frames)
                                                                     │  server.flush() drains
                                                                     ▼
server thread:                                   Broadcaster (Arc<Mutex<Vec<Sink>>>)  ──writes──▶
browser:                          foundation-wasm-ui.js  ◀──SSE/WS frames──  applies DomOps (morphdom)
```

## The `!Send` constraint (load-bearing)

`App`, `Context`, `SharedInstructionReceiver`, and `CollectedFrames` are **`Rc`-based
→ `!Send`** (the wasm-ui runtime is single-threaded by design). Therefore:

- The App is **created and driven on the test thread** (inside the test body, where
  `server.app()` is called). It NEVER moves to the `foundation_http` worker thread.
- The `foundation_http` server thread only touches the **`ContextBag`** (the
  `Broadcaster` + the page) — never the App.
- `server.flush()` runs on the **test thread**: it drains `CollectedFrames` and
  writes the bytes to each connection sink held by the `Broadcaster`. The sinks
  (SSE/WS connections) are `Send` and live behind `Arc<Mutex<…>>`; the worker
  thread only ADDS sinks (on connect), the test thread WRITES to them (on flush).
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

The **backlog + replay** removes the connect/flush race: the browser opens the
stream after page load, possibly *after* the first `flush()`; a late sink is
caught up by replaying the backlog. `server.flush()` appends to the backlog AND
writes to every live sink.

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
3. **`server.app()`/`flush()`** — `App::server()` held in the (`!Send`) `TestServer`;
   `flush` drains `CollectedFrames` → broadcaster.
4. **HTML shell** that loads `runtimes/foundation-wasm-ui.js` and opens the stream
   (the default `TestConfig` page in Mode-1 tests).
5. **First real component end-to-end:** a spec-42 `dialog`/`popover` test driven by
   Rust signals, asserted in a real browser.
6. **Mode 2** validated with a built wasm bundle (`StaticFileHandler`).

## Open decisions

- **SSE vs WS first:** SSE-over-fetch is simpler and the JS runtime supports it; WS
  gives lower latency + the coalescing path. Lean SSE for v1.
- **`flush()` explicit vs auto:** explicit `server.flush()` mirrors `app.stabilize()`
  (predictable in tests); an auto-flush-on-assert convenience can come later.
- **Frame encoder default:** `FrameSinkV1` (columnar) for fidelity, or `JsonEncoder`
  for debuggable streams in failing tests — possibly `TestConfig`-selectable.
