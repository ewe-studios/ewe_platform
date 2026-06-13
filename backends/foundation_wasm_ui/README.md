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
