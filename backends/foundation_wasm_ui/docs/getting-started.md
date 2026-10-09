# Getting started — zero to 100

This is the hands-on tour. Eight steps, each a small runnable example that builds
on the last. By the end you can write reactive, themed, composed UI **and** drive
a real browser from a native server. Each step links to the deep-dive doc for that
topic.

> Mental model in one line: **Rust never touches the DOM.** You build a tree, call
> `stabilize()`, and the runtime ships a minimal batch of typed `DomOp`s to the JS
> runtime, which applies them. No virtual DOM, no diffing, no wasm-bindgen.

---

## 1. Hello world

An `App`, one `html!` tree, one `stabilize()`. `App::new()` wires the whole stack
(signal runtime, root scope, protocol, receiver); `app.context()` hands back the
`(Context, receiver)` pair every reactive `html!` needs.

```rust
use foundation_wasm_ui::{html, App};

let app = App::new();
let (ctx, receiver) = app.context();

let _ui = html! { ctx, receiver,
    <div class="greeting"><h1>"Hello, world"</h1></div>
};
app.stabilize();   // builds the nodes and flushes them to JS as DomOps
```

That's the whole loop: build a tree, `stabilize()`, the runtime ships the batch.

## 2. Reactivity

State lives in a **signal** — a `(getter, setter)` pair from `ctx.signal(..)`. Read
it in a `{expr}` slot with `.get()`; that read subscribes the slot, so when the
setter writes, **only that slot** re-renders (one `SetText`, no diffing).

```rust
let (count, set_count) = ctx.signal(0i64);

let _ui = html! { ctx, receiver,
    <p class="count">"Count: " {count.get()}</p>
};
app.stabilize();          // first paint: "Count: 0"

set_count.set(1);
app.stabilize();          // exactly one SetText("1") — nothing else re-runs
```

Writes coalesce: several `set()`s before a `stabilize()` flush once, glitch-free.

## 3. Static vs reactive attributes

Three attribute kinds, distinguished by the brackets:

- **`attr="lit"`** — a literal, set once at build.
- **`attr=[expr]`** — **static-once**: evaluated a single time at mount, no effect,
  no clone — for owned config values.
- **`attr={expr}`** — **reactive**: wrapped in an effect, re-runs on signal change.

Plus the **presence** form for the data-attribute styling contract:
`{cond.then_some("")}` yields `Some` (sets the attribute) when on and `None`
(removes it) when off, so `[data-active]` in CSS matches *only* when active.

```rust
let (active, _set_active) = ctx.signal(false);

let _ui = html! { ctx, receiver,
    <section
        class="panel"                              // static literal
        data-count={count.get()}                    // reactive: re-runs on change
        data-active={active.get().then_some("")}    // presence: Some("") on, None off
    />
};
```

Full rules: **[html! macro](./html-macro.md)**.

## 4. Events

Two ways a browser event reaches Rust, both resolved by `MaybeCallback` at
compile time:

**(a) Two-way binding** — pass a `SignalSetter` to a value-carrying event; for
`String`/`bool`/numeric signals the setter pulls `value`/`checked` off the event:

```rust
let (name, set_name) = ctx.signal(String::new());
let _ui = html! { ctx, receiver,
    <input value={name.get()} primal:onchange={set_name} />
};
```

**(b) `ctx.callback`** — the escape hatch for events that carry *no* value (clicks,
load/error). It registers a closure and returns a `Callback`:

```rust
let (count, set_count) = ctx.signal(0i64);
let inc = ctx.callback(move |_event| set_count.set(count.get() + 1));
let _ui = html! { ctx, receiver, <button primal:onclick={inc}>"+1"</button> };
```

Deep dive: **[reactivity & events](./reactivity-and-events.md)**.

## 5. Lists & conditionals

Structural reactivity — mount/unmount on a condition, or reconcile a keyed list.
**Reactive form only.**

```rust
use foundation_signals::Context;
use foundation_wasm_ui::{Slot, SharedInstructionReceiver};

let (open, _set_open)   = ctx.signal(false);
let (items, _set_items) = ctx.signal(vec![1i64, 2, 3]);

let _ui = html! { ctx, receiver,
    <div>
        <Show when={open.get()}>
            { Slot::lazy(|ctx, rcv| html! { ctx, rcv, <p>"now visible"</p> }) }
        </Show>
        <ul>
            <For
                each={items.get()}
                key={|n: &i64| *n}
                render={|ctx: &Context, rcv: &SharedInstructionReceiver, n: &i64|
                    html! { ctx, rcv, <li>{*n}</li> }}
            />
        </ul>
    </div>
};
```

Keys identify *instances*, not data. Deep dive: **[reactivity & events](./reactivity-and-events.md#show--for)**.

## 6. Theming

Hand a `theme!{}` to `App::new().theme(..)` and its CSS (custom properties, a dark
`@media` block, utility classes) is injected into `<head>` before the first batch.

```rust
use foundation_wasm_ui::{theme, App};

let app = App::new().theme(theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" }, accent: "#10b981" }
    spacing { sm: 8px, md: 16px }
    radius  { card: 8px }
});
// elements can now use --color-primary, .bg-primary, .p-md, .rounded-card, …
```

Full grammar + utility scales: **[theming](./theming.md)**.

## 7. Composing components

A "component" is just a function returning `Html` that takes the
`(Context, receiver)` pair — there is **no `Component` trait**.

```rust
use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

fn counter(ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
    let (count, set_count) = ctx.signal(0i64);
    let inc = ctx.callback(move |_| set_count.set(count.get() + 1));
    html! { ctx, rcv, <button primal:onclick={inc}>"Count: " {count.get()}</button> }
}

let _ui = html! { ctx, receiver, <div>{counter(&ctx, &receiver)}</div> };
```

For ready-made headless primitives (button, dialog, select, field, …), use
**[`foundation_ui_components`](../../foundation_ui_components/README.md)**. To accept
*content* into your own components, see **[composition](./composition.md)**.

---

## 8. How the page renders — two modes, no guesswork

There are **two ways** a `foundation_wasm_ui` app reaches a browser. The Rust code
above is the same for both; only the delivery path changes.

### Mode 1: WASM in the browser (client-side)

Your Rust compiles to `.wasm`, loaded by the browser. The `App` runs directly in
the WASM sandbox — `stabilize()` flushes `DomOp`s to the JS runtime, which
applies them to the live DOM. **No `<mount-stream>` needed.** The WASM binary
talks straight to `foundation-wasm-ui.js`.

**The HTML page:**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>My App</title>
</head>
<body>
  <script type="module">
    // The generated init() wrapper fetches the WASM, instantiates it, and wires
    // the DOM path (protocol 1 → DomOpApplicator → live document). One call, done.
    import { init } from './myapp.js';

    await init();   // ← fetches .wasm, boots runtime, wires DOM, runs Rust entrypoint
  </script>
</body>
</html>
```

**The Rust entrypoint** (`src/lib.rs`):

```rust
use foundation_wasm_ui::{html, App};
use foundation_macros::wasm_bin;

// The #[wasm_bin] attribute marks this as a main-thread WASM entrypoint.
// ewe-wasm-bundle generates the JS wrapper that calls `myapp()`.
#[wasm_bin]
pub fn myapp() {
    let app = App::new();
    let (ctx, receiver) = app.context();

    let (count, set_count) = ctx.signal(0i64);
    let inc = ctx.callback(move |_| set_count.set(count.get() + 1));

    // mount() appends the tree to <body> — no <mount-stream> needed,
    // the WASM runtime applies DomOps directly to the live document.
    app.mount(html! { ctx, receiver,
        <button primal:onclick={inc}>"Count: " {count.get()}</button>
    });
    app.stabilize();
}
```

**What files the browser needs:**

| File | Where it comes from | Purpose |
|------|---------------------|---------|
| `index.html` | you write it | Bootstrap page |
| `myapp.wasm` | `ewe-wasm-bundle build` output | Your compiled Rust |
| `myapp.js` | generated by `ewe-wasm-bundle` | WASM loader + `init()` |
| `foundation-wasm-ui.js` | `backends/foundation_wasm_ui/runtimes/` or `embedded` module | DOM runtime (applicator, events, `registerWasmApp`) |
| `foundation-wasm.js` | `backends/foundation_wasm/runtimes/` or `embedded` module | Core ABI (memory, transport, `FoundationWasm`) |

Serve them all from the same directory (or adjust paths). The bundler copies the
runtime assets automatically.

**How `init()` works:**

The generated wrapper handles the full boot sequence:
1. Fetches `myapp.wasm` and instantiates it with `FoundationWasm`'s `web_abi`
2. Binds the WASM instance to the runtime (`rt.init(instance)`)
3. Registers `columnarHandler` on protocol byte 1 so `App::stabilize()` DomOps reach the DOM
4. Calls your Rust entrypoint (`instance.exports.myapp()`)

The columnar wire (protocol byte 1) is what `App::new()` uses — without the handler
registered in step 3, every `stabilize()` dispatches to an unknown protocol and its
DomOps go nowhere. The wrapper wires this automatically so you just call `init()`.

### Mode 2: Native server streams to the browser (fullstack)

Your Rust runs as a **native server binary**. The `App` runs there, and each
`stabilize()` produces an envelope-framed byte batch. Your server ships those
batches to connected browsers over SSE, WebSocket, or any transport. The browser
side is a single `<mount-stream>` custom element that connects back and applies
the frames.

**The HTML page:**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>My App</title>
  <script type="module">
    import { registerWebComponents } from '/foundation-wasm-ui.js';
    registerWebComponents();
  </script>
</head>
<body>
  <!-- The mount-stream connects to your server and applies DomOp frames live.
       api = your server's streaming endpoint
       transport = "sse" (default), "ws", "chunked"
       protocol = "arrow" (binary envelope) or omit for plain HTML frames -->
  <mount-stream api="/__primal/stream" transport="sse" protocol="arrow"></mount-stream>
</body>
</html>
```

**The Rust server:**

```rust
use foundation_wasm_ui::{html, App};
use foundation_wasm_ui::server::{Broadcaster, BroadcastSink};

// A broadcaster fans frames to every connected client.
let broadcaster = Broadcaster::new();
let app = App::with_protocol(BroadcastSink::new(broadcaster.sender()));
let (ctx, rcv) = app.context();

let (status, set_status) = ctx.signal(String::from("ready"));

// mount() + stabilize() on the server encodes DomOps into frames
// that the broadcaster sends to every connected <mount-stream>.
app.mount(html! { ctx, rcv, <p id="status" class="status">{status.get()}</p> });
app.stabilize();                       // initial frame → all clients

set_status.set(String::from("live ✨"));
app.stabilize();                       // a SetText frame → the DOM updates live
```

Your server handler registers the `<mount-stream>`'s transport (SSE, WebSocket,
etc.) with the `Broadcaster` — see **[server & protocols](./server-and-protocols.md)**
for the `FrameTransport` trait and the full broadcast flow.

### Mounting without streaming

If you don't need live updates, you don't need a stream at all:

```rust
// Pure HTML — no App, no signals, no runtime.
let body = html! {
    <main class="app">
        <h1>"Welcome"</h1>
        <p>"This page was rendered from Rust templates."</p>
    </main>
};
// Serve body.to_markup() as text/html — done.
```

`to_markup()` serializes the same tree the DomOp channel would build — one
renderer, slot spans and all. No `App`, no allocation arena. Use it for
first-paint pages, SEO-critical content, or static sites that never go reactive.

You can also mix: serve `to_markup()` for the initial paint, then drop a
`<mount-stream>` onto the page for later live updates:

```rust
let body = html! {
    <main>
        <h1>"Initial content"</h1>
        <!-- When the JS runtime loads, this element connects and starts streaming -->
        <mount-stream api="/live" transport="sse" protocol="arrow"></mount-stream>
    </main>
};
```

### `App::mount()` vs `<mount-stream>`

| | `App::mount()` (WASM mode) | `<mount-stream>` (server mode) |
|---|---|---|
| **Where App runs** | Browser (WASM) | Native server |
| **How DomOps arrive** | `host_apply` → `dispatcher` → `DomOpApplicator` | Envelope-framed bytes over SSE/WS |
| **HTML page needs** | `FoundationWasm` + `registerWasmApp(rt)` + `init()` | `<mount-stream>` + `registerWebComponents()` |
| **JS runtime** | `foundation-wasm.js` + `foundation-wasm-ui.js` | `foundation-wasm-ui.js` |
| **Live updates** | `stabilize()` → DomOps → DOM | `stabilize()` → encode → stream → browser applies |

### What `registerWebComponents()` does

It registers three custom elements the browser can use **without any app JS**:

| Element | Purpose |
|---|---|
| `<mount-stream>` | Continuous server-driven UI (SSE, WS, chunked) |
| `<mount-data>` | One-shot request → one response |
| `<primal-island>` | Self-contained scoped component (styles + scripts + events) |

It also creates `window.primal` — a programmatic API for mounting, wiring events,
and scoping styles from non-module scripts. Called once at page load; idempotent.

---

## Where to go next

| You want to… | Read |
|---|---|
| Master the template syntax | [html! macro](./html-macro.md) |
| Understand signals, events, `<Show>`/`<For>` | [reactivity & events](./reactivity-and-events.md) |
| Build reusable components with slots | [composition](./composition.md) |
| Theme and scope styles | [theming](./theming.md) |
| Serve fullstack / stream to browsers | [server & protocols](./server-and-protocols.md) |
| Understand the runtime + wire internals | [internals](./internals.md) |
| Ship a wasm bundle | [shipping](./shipping.md) |
