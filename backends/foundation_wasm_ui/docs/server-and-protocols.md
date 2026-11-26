# Server rendering and frame delivery

EWF1 identifies a binary user interface frame. A live channel retains a sequence of frames for connected clients.
Server-sent events (SSE) deliver text events over a Hypertext Transfer Protocol (HTTP) connection.

## Render the first page

A pure `html!` template returns an `Html` value. Call `to_markup()` to obtain its first-page markup.
A mounted WebAssembly application can emit its first frame directly to the browser.
A native application can collect its frames or publish them to a live channel.

## Collect application frames

```rust
use foundation_wasm_ui::{html, App};

let (app, frames) = App::server();
let (ctx, receiver) = app.context();
app.mount(html! { ctx, receiver, <span>"Ready"</span> });
app.stabilize()?;
assert_eq!(&frames.borrow()[0][4..8], b"EWF1");
```

## Publish application frames

```rust
use foundation_wasm_ui::{html, App};
use foundation_wasm_ui::server::{FrameBroadcastSink, LiveChannel};

let channel = LiveChannel::new(1, Default::default(), Default::default());
let app = App::with_protocol(FrameBroadcastSink::new(channel.sender()));
let (ctx, receiver) = app.context();
app.mount(html! { ctx, receiver, <span>"Ready"</span> });
app.stabilize()?;
```

Keep the channel in the server context. A connection subscribes with its last applied sequence.
The server sends retained frames in order before sending new changes.
A frame mount supplies the cursor automatically when it reconnects.

## Receive frames

```html
<mount-stream api="/live" transport="sse" protocol="frame" data-channel-id="1"></mount-stream>
<script type="module">
  import * as ui from "./foundation-wasm-ui.js";
  import {installFrameRuntime} from "./foundation-ui-frames.js";
  installFrameRuntime(ui);
  ui.registerWebComponents();
</script>
```

Each SSE event carries one base64 frame. Each WebSocket binary message carries one bare frame.
An HTTP response with type `application/primal-frame` prefixes each frame with its four-byte little-endian length.
The application can also send markup to an HTML mount or send JSON signal patches.

## Live browser testing

The test server publishes application frames at `/__primal/live`.
Call `App::with_protocol(server.broadcast_sink())` to connect an application to that endpoint.
The separate `/__primal/stream` endpoint serves raw markup or bytes from `server.broadcaster()`.

See the [browser smoke tests](../../foundation_browser/tests/smoke.rs) and the [todo example](../../../examples/ui_todo/README.md).
