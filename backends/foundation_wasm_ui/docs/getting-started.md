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

## 8. Going fullstack — a native server drives a real browser

The same `App` runs on a native server; its protocol sink streams encoded DOM
frames to the browser, where a single `<mount-stream>` applies them. Flip a signal
in Rust → the live page updates. This is the production server-driven-UI path
(and what the browser test harness uses).

```rust
use foundation_wasm_ui::{html, App};
use foundation_wasm_ui::server::{Broadcaster, BroadcastSink};

// On the server: a broadcaster fans frames to every connected client.
let broadcaster = Broadcaster::new();
let app = App::with_protocol(BroadcastSink::new(broadcaster.sender()));
let (ctx, rcv) = app.context();

let (status, set_status) = ctx.signal(String::from("ready"));
app.mount(html! { ctx, rcv, <p id="status" class="status">{status.get()}</p> });
app.stabilize();                       // initial mount → frame → browser

set_status.set(String::from("live ✨"));
app.stabilize();                       // a SetText frame → the DOM updates live
```

The browser side is just:

```html
<mount-stream api="/__primal/stream" transport="sse" protocol="arrow"></mount-stream>
<script type="module">
  import { registerWebComponents } from '/foundation-wasm-ui.js';
  registerWebComponents();
</script>
```

The wire is the envelope's business — columnar (default), JSON, or Apache Arrow
IPC all render identically. Full story (presets, transports, fullstack layout,
live-browser testing): **[server & protocols](./server-and-protocols.md)**.

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
