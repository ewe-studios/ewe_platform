# foundation_wasm_ui

Build reactive web UIs — and fullstack, server-driven apps — in pure Rust. No
wasm-bindgen, no virtual DOM, no JS framework. You write Rust that looks like
HTML; the Rust side never touches the DOM — it emits typed `DomOp` instructions
over a compact binary wire, and a tiny dependency-free JS runtime applies them.

```rust
use foundation_wasm_ui::{html, App};

let app = App::new();                 // default: compact columnar wire (zero-copy)
let (ctx, receiver) = app.context();

let (count, set_count) = ctx.signal(0i64);
let _ui = html! { ctx, receiver,
    <button primal:onclick={set_count}>"Count: " {count.get()}</button>
};
app.stabilize();                      // → DomOps flush to JS: create, set text, wire click
```

Every later `set_count.set(n)` (including clicks from the browser) re-runs exactly
the one affected slot and ships a minimal `SetText` — no diffing, no re-render.

---

## Start here

**▶ [Getting started — zero to 100](./docs/getting-started.md)** — eight short
steps from hello-world to a native server driving a real browser.

## What you can build

- **Reactive client UIs** — signals, fine-grained updates, keyed lists, conditionals.
- **Reusable components** — plain functions returning `Html`, with typed slots.
- **Themed, scoped styling** — a `theme!{}` of design tokens; co-located scoped CSS.
- **Server-rendered pages** — first-paint HTML from the same templates.
- **Live server-driven UIs** — a native `App` streams DOM updates to many browsers
  over SSE/WebSocket, in columnar / JSON / Apache Arrow — driven by Rust signals.
- **Ready-made headless components** — see
  **[`foundation_ui_components`](../foundation_ui_components/README.md)** (dialog,
  select, field, menu, …): behavior + a data-attribute contract, you bring the CSS.

## Documentation

| Guide | What it covers |
|-------|----------------|
| [Getting started](./docs/getting-started.md) | The zero-to-100 tutorial |
| [The `html!` macro](./docs/html-macro.md) | Template syntax: forms, slots, attribute kinds |
| [Reactivity & events](./docs/reactivity-and-events.md) | Signals, two-way binding, `<Show>`/`<For>` |
| [Composition](./docs/composition.md) | Components, `Slot`/`Render`, `mount_*` |
| [Theming & scoped styles](./docs/theming.md) | `theme!{}`, utility scales, scoped CSS |
| [Server, protocols & fullstack](./docs/server-and-protocols.md) | Wire protocols, `App::server`, the `server` broadcaster, live-browser testing |
| [Internals](./docs/internals.md) | Runtime, JS runtime, web components, the wire |
| [Shipping](./docs/shipping.md) | Cargo features, wasm bundling, testing, limits |

## Install

```toml
[dependencies]
foundation_wasm_ui = { workspace = true }
```

`#![no_std]` on wasm32. The `arrow` feature (Apache Arrow IPC) is on by default;
`embedded-js` exposes the runtime JS as `&str` consts for bundler-less serving.

---

Design history, gap analyses, and per-feature verification live in
[`specifications/39-foundation-wasm-ui/`](../../specifications/39-foundation-wasm-ui/)
and [`specifications/42-ui-component/`](../../specifications/42-ui-component/).

## Browser actions

Install the frame reader before wiring a server or browser action. Serve its JavaScript modules from the same directory.

```js
import * as ui from "./foundation-wasm-ui.js";
import * as behaviours from "./foundation-ui-behaviours.js";
import {installFrameRuntime} from "./foundation-ui-frames.js";
import {FetchActionTransport, JsActionRegistry, installActionRuntime} from "./foundation-ui-actions.js";

const frames = installFrameRuntime(ui);
const registry = new JsActionRegistry();
registry.declare("server", "add_todo");
registry.registerHost("notify", ({node}) => node.focus());
const server = new FetchActionTransport({
  url: "/action",
  hello: () => manager.hello(),
  requestSignal: () => AbortSignal.timeout(5000),
});
const manager = installActionRuntime(ui, {frames, registry, server, behaviours});
```

Bind a control to `server:add_todo` or `host:notify`. On each event, the manager reads that control’s input and signed attachment.
An HTTP action sends one request. A retry requires an explicit policy. A socket action finishes its send when the open socket accepts the message.
Pass later binary replies to `manager.receive(bytes)`.

A compiled host action uses `WasmActionTransport`. Install its completion import before creating the WebAssembly instance.
Forward `env.ewe_ui_action_done(call, slot)` to `transport.complete(call, slot)`. Call `transport.close()` when the instance shuts down.

Run the shared route vectors and transport checks from the repository root.

```sh
node --test backends/foundation_wasm_ui/integration/test/actions-*.test.js
```

### Run a Rust action in a browser

The application supplies its request context, registered actions, connection and executor seed.
The context supplies the signer used to verify and issue state.
Call this function from the WebAssembly module's entry point.

```rust
#[cfg(target_family = "wasm")]
fn install_actions(
    ctx: foundation_core::valtron::Ctx,
    seed: u64,
    registry: std::sync::Arc<foundation_ui_runtime::action::ActionRegistry>,
    connection: foundation_ui_runtime::dispatch::Connection,
) {
    use foundation_core::valtron::single;
    use foundation_ui_runtime::action::Origin;
    use foundation_ui_runtime::dispatch::{DispatcherBuilder, LocalDispatcher};
    use foundation_wasm_ui::actions::install_local_dispatcher;

    single::initialize_pool_with_ctx(seed, ctx.clone());
    let dispatcher = DispatcherBuilder::new(Origin::Host, registry).build();
    install_local_dispatcher(LocalDispatcher::new(dispatcher, ctx, connection));
}
```

The JavaScript bridge transfers its request allocation to Rust when it calls `ewe_ui_action`.
Rust releases that allocation before it returns the start status. A zero status means that Valtron accepted the task.
When the action waits, Valtron returns control to the browser. A wake schedules a microtask to poll the action again.
Rust passes the call number and reply allocation to `env.ewe_ui_action_done`. JavaScript copies the reply and releases the allocation.
The missing-reply value is the unsigned 64-bit maximum. The first arena allocation can have number zero.
Call the transport's `close()` when the module stops. It rejects pending promises and drops the corresponding Rust futures.

## Apply a frame in a browser

A frame is one FlatBuffers message with the four-byte identifier `EWF1`.
The identifier starts at byte four. The browser shares one node registry with the existing document renderer.
A registry maps each numeric node identifier to its document node.

Install the reader before registering the mount elements. This page receives binary frames through server-sent events (SSE).
The server sends one base64-encoded frame in each event.

```html
<mount-stream api="/updates" transport="sse" protocol="frame"></mount-stream>
<script type="module">
  import * as ui from "/__primal/foundation-wasm-ui.js";
  import { installFrameRuntime } from "/__primal/foundation-ui-frames.js";

  const frames = installFrameRuntime(ui, {
    document,
    onReport: report => console.error(report.code, report),
    onGap: gap => console.warn("Request a replay after", gap.resumeFrom),
  });
  ui.registerWebComponents();
</script>
```

An HTTP response with content type `application/primal-frame` starts each frame with its four-byte, little-endian byte count.
A WebSocket binary message carries one frame directly. The default frame limit is four mebibytes.
Set `maxFrameBytes` in the installation options to change that limit.

The reader applies shared node, property, text, binding, placement, behaviour and signed-state operations.
It sends data tables to application code through the `ewe:data` event.
A placement accepts a numeric node identifier or one unique browser selector.
A child selector for before or after must match one direct child.
Use `frames.registerUnmount(element, dispose)` to release a local reactive scope when its component leaves the document.

### Write a message by hand

The following module builds a complete message with the generated types. The browser body has registry identifier 1.
Use this example with the installed `frames` reader above.

```js
import * as w from "/__primal/foundation-ui-wire.js";

const create = new w.OperationT(w.UiOperation.CreateNode, new w.CreateNodeT(42, w.NodeKind.TEXT));
const text = new w.OperationT(w.UiOperation.SetText, new w.SetTextT(42, "Buy milk"));
const append = new w.OperationT(w.UiOperation.AppendChild, new w.AppendChildT(1, 42));
const messages = [create, text, append].map(op => new w.MessageT(op));
const value = new w.FrameT(1, 1n, 0n, w.FrameKind.FrameMessage, new w.FrameMessageT(0n, messages));
const builder = new w.Builder();
w.Frame.finishFrameBuffer(builder, value.pack(builder));
frames.receive(builder.asUint8Array());
```

Only `AttachState` writes the reserved `primal::state` attribute. It copies the signer's bytes to the specified element.
The browser uses unpadded base64url so the attribute can carry any byte sequence.

The Rust fixture test encodes all 19 document operations with both writers.
It also writes the shared operations as individual rows and as one column batch.
The browser tests read the saved bytes and compare the resulting nodes, properties and registry entries.

```sh
UPDATE_FRAME_FIXTURES=1 cargo test -p foundation_wasm_ui --test frame_parity_fixtures_tests
mise run ui-web:test
```

[Wire format and Rust writer](../foundation_ui_wire/README.md) · [Browser fixtures](./integration/fixtures/frames)

### Dispose a local Rust scope when a frame removes its node

A reactive scope owns the effects that watch local signals. The WebAssembly app installs its event bridge at startup.
`<Show>` and `<For>` register their local scopes automatically. An app can register its own component in the same receiver.

```rust
use foundation_signals::Context;
use foundation_wasm_ui::{html, install_app_bridge, App};

fn mount_clock(app: &App) -> Context {
    install_app_bridge(app);
    let scope = app.ctx().child();
    let receiver = app.receiver();
    let view = html! { scope, receiver, <span>Clock</span> };
    let root = app.mount(view);
    receiver.register_scope(&[root], &scope);
    scope
}
```

Keep the returned context while the component is mounted.
After loading the WebAssembly instance, call `ui.registerWasmApp(runtime, { document })` in the browser.
The local renderer registers the instance's disposer on each node it creates.
When a remote frame replaces that node, the reader calls `ewe_ui_dispose_mount` on the same instance.
The receiver stops its effects and runs scope cleanup once. A fragment with several roots stays active until its last root leaves.
A keyed node that survives a morph keeps its scope. A host-owned JavaScript component can supply its own cleanup.

```js
const observer = new ResizeObserver(updateLayout);
observer.observe(componentRoot);
frames.registerUnmount(componentRoot, () => observer.disconnect());
```
