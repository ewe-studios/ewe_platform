# foundation_wasm_ui

The UI layer of the Ewe platform's owned WASM stack: DOM bindings, the `html!`
macro, reactive rendering over `foundation_signals`, and the wire protocols
that ship DOM updates from Rust to the browser — **without wasm-bindgen, a
virtual DOM, or any JS framework**. The Rust side never touches the DOM; it
emits typed `DomOp` instructions over a binary protocol, and the owned JS
runtime (`runtimes/foundation-wasm-ui.js`) applies them.

```rust
use std::rc::Rc;
use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_wasm::MemoryAllocations;
use foundation_wasm_ui::{html, ColumnarV1, Runtime};

let signals = Rc::new(SignalsRuntime::new());
let ctx = Context::new(Rc::clone(&signals));
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())
    .build();
runtime.attach(&signals);
let receiver = runtime.receiver();

let (count, set_count) = ctx.signal(0i64);
let _tree = html! { ctx, receiver,
    <button primal:onclick={set_count}>Count: {count.get()}</button>
};
signals.stabilize(); // → DomOps flushed to JS: create nodes, set text, wire click
```

Every later `set_count.set(n)` (including clicks arriving from the browser)
re-runs exactly the affected slot effect and ships a minimal `SetText` — no
diffing, no re-render of the tree.

---

## 1. The mental model

One loop, five stages:

```
 Rust (wasm)                                        │  JS (browser)
                                                    │
 signal.set(v)                                      │
   └─ stabilize()            height-ordered, glitch-free
        └─ effects re-run    one per dynamic slot   │
             └─ DomOps queued on InstructionReceiver│
                  └─ flush() → protocol encode ─────┼─→ ColumnarParser (zero-copy views)
                                                    │     └─ DomOpApplicator (19 ops)
 invoke_signal_callback(id) ←───────────────────────┼── EventDispatcher (click, input, …)
   └─ setter runs → stabilize() … (loop repeats)    │
```

- **`foundation_ui_traits`** defines the shared vocabulary: the typed
  [`Html`] tree, [`Part`] (which slots are dynamic), the 19-variant
  [`DomOp`], compact wire ids for known tags/attributes, and the
  `ProtocolEncoder` seam.
- **`foundation_signals`** is the reactive engine (R3-style height-ordered
  graph). It knows nothing about DOM.
- **`foundation_wasm`** is the pure WASM↔JS ABI: arena memory, the uniform
  `host_apply(mem_id, ptr, len)` transport, function registry.
- **this crate** composes them: `html!`, the protocol implementations, the
  `InstructionReceiver`, the event bridge — plus the owned JS runtime file
  that is the *other half* of every contract here.

Nothing is hidden: a "component" is a function, an "update" is a signal
write, and the wire format is documented bytes you can parse yourself.

---

## 2. Wiring a program

The one-liner (spec-42 feature 03):

```rust
use foundation_wasm_ui::App;

let app = App::new();                 // THE DEFAULT: arrow-family wire v1 (zero-copy columnar)
let (ctx, receiver) = app.context();  // the pair every reactive html!/component call needs
// … mount templates …
app.stabilize();
```

Presets: `App::new()`/`columnar()` (arrow v1 — we always default to arrow),
`App::json()` (debugging), `App::mock()` (tests — returns the captured-ops
handle), `App::server()`/`server_with(encoder)` (see §server below),
`App::arrow_ipc()` (wire v2 real Arrow IPC, `arrow` cargo feature), and
`App::with_protocol(...)` as the escape hatch. `app.scope()` gives a child
`Context` for component-scoped teardown; dropping the `App` disposes the
root scope. `Context` clones are HANDLES to one scope (disposal at the last
handle).

### What `App` builds (the five objects)

Under the hood every program is the same five things:

```rust
use std::rc::Rc;
use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_wasm::MemoryAllocations;
use foundation_wasm_ui::{ColumnarV1, Runtime};

// 1. The signal runtime — owns the reactive graph.
let signals = Rc::new(SignalsRuntime::new());

// 2. A Context — an OWNERSHIP SCOPE for signals/effects. Dropping (or
//    disposing) a context tears down everything created through it.
let ctx = Context::new(Rc::clone(&signals));

// 3. The UI runtime — protocol + memory + the DomOp queue.
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())        // wire format (see §6)
    .memory(MemoryAllocations::new())   // the WASM-side arena
    .build();

// 4. Attach: after every stabilize(), queued DomOps flush automatically.
runtime.attach(&signals);

// 5. The receiver handle — what html! queues DomOps onto.
let receiver = runtime.receiver();
```

`Context` and `SharedInstructionReceiver` are both cheap `Rc`-backed handles.
They are passed **explicitly** — there is no global, no thread-local. A
"component" that needs reactivity takes them as parameters:

```rust
fn counter(ctx: &Context, receiver: &SharedInstructionReceiver) -> Html {
    let (count, set_count) = ctx.signal(0i64);
    html! { ctx, receiver,
        <button primal:onclick={set_count}>{count.get()}</button>
    }
}
```

For component-scoped teardown, create a child scope: `let scope = ctx.child();`
— signals/effects made through `scope` die when `scope` is disposed, without
touching the parent.

---

## 3. The `html!` macro

`html!` has **two forms**. Both parse real HTML syntax at compile time into
the typed `Html` tree (known tags/attributes resolve to compact wire ids;
custom elements like `<my-widget>` stay name-form).

### 3.1 Pure form — `html! { <markup> }` → `Html`

No context, no receiver, no reactivity — just a value. This is the form for
server rendering, static fragments, and composition:

```rust
let h = html! { <div class="card"><h2>"Title"</h2></div> };
```

Dynamic *values* are still allowed via `{expr}` slots; they are evaluated
once, through the `IntoHtml` trait:

| slot expression type        | becomes                                        |
|-----------------------------|------------------------------------------------|
| `Html` / `&Html`            | inlined as a child element                     |
| `Option<Html>`              | the element, or an empty fragment (`None`)     |
| `Vec<Html>`                 | a tagless wrapper whose children are the items |
| `&str` / `String`           | a text node                                    |
| integers / floats / `bool`  | a text node via `Display`                      |

That table is the whole composition story — plain functions returning `Html`
nest arbitrarily:

```rust
fn badge(label: &str) -> Html {
    html! { <span class="badge">{label}</span> }
}

fn user_card(name: &str, tags: &[&str], premium: bool) -> Html {
    let tag_badges: Vec<Html> = tags.iter().map(|t| badge(t)).collect();
    let crown = premium.then(|| html! { <b>"★"</b> });   // conditional render
    html! {
        <div class="card">
            <h2>{name}</h2>
            {crown}
            <ul>{tag_badges}</ul>
        </div>
    }
}
```

### 3.2 Reactive form — `html! { ctx, receiver, <markup> }`

The first two arguments are a `Context` and a `SharedInstructionReceiver`.
This form *mounts*: it allocates a contiguous block of `primal-id`s for the
template's nodes, queues the `CreateElement`/`SetAttribute`/`RegisterNode`
ops that build the subtree, and creates **one effect per dynamic slot**:

| part kind                   | what mounts                                              |
|-----------------------------|----------------------------------------------------------|
| `{signal.get()}` child slot | a dedicated text node + an effect emitting `SetText`     |
| `attr={signal.get()}`       | an effect emitting `SetAttribute` on change              |
| `primal:on*={handler}`      | one-time `AddEventListener` (no effect)                  |

Each invocation gets a **disjoint id block**, so instantiating the same
component in a loop is safe — instances can never collide. The initial render
arrives with the first `stabilize()` after mounting (effects run once on
creation).

```rust
let (count, set_count) = ctx.signal(7i64);
let tree = html! { ctx, receiver, <div><span>{count.get()}</span></div> };
signals.stabilize();   // CreateElement(div), CreateElement(span), SetText("7"), …
set_count.set(8);
signals.stabilize();   // exactly one SetText("8") — nothing else re-runs
```

> The reactive form returns the same `Html` value (parts included), so you
> can still inspect or serialize what was mounted.

### 3.3 Events: closures vs. setters

`primal:on<event>={...}` accepts two shapes:

```rust
// 1. A closure — receives the full event context.
let log = |event: &EventData| { /* event.value, event.checked, key, mouse… */ };
html! { ctx, receiver, <button primal:onclick={log}>"go"</button> };

// 2. A SignalSetter — two-way binding sugar. The macro stamps a
//    `primal:setter` attribute with the setter's callback id; the JS
//    EventDispatcher routes the input's value straight into the signal.
let (name, set_name) = ctx.signal(String::new());
html! { ctx, receiver, <input primal:onchange={set_name} /> };
```

On the browser side the `EventDispatcher` serializes an `EventData` JSON into
the shared arena and calls the exported `invoke_signal_callback(callback_id,
allocation_id)`; the WASM side reads it, frees the slot, runs the callback,
and stabilizes synchronously — so the DOM answer to an input event goes out
in the same call.

Default conversions exist for setters of common types (`String`, numbers,
`bool` from checkboxes); closures get the raw `EventData`.

### 3.4 Scoped styles

A `<style primal:style>` child takes a **string literal** of CSS and scopes
it to the component at compile time (no runtime CSS work):

```rust
let h = html! {
    <div class="counter">
        <style primal:style>{r#"
            .display { font-size: 2rem; }
            :parent { border: 1px solid #ccc; }
        "#}</style>
        <span class="display">{count.get()}</span>
    </div>
};
```

Selectors are rewritten against the component's `primal-id` (`:parent`
targets the host element). In the pure form the style stays inline; in the
reactive form the rewritten CSS is queued as head-injection ops instead.

> CSS must be a string literal because raw CSS (hex colors, etc.) does not
> tokenize as Rust.

---

## 4. Signals in five lines

The full reactive API lives in `foundation_signals` (see its docs); the parts
you use daily:

```rust
let (get, set) = ctx.signal(0i64);          // state
let double = ctx.computed(move || get.get() * 2);   // derived, cached, lazy
ctx.effect(move || { let _ = double.get(); /* side effect */ });
set.set(5);
signals.stabilize();                        // run dirty effects, glitch-free
```

Guarantees: height-ordered evaluation (no glitches — an effect never sees a
half-propagated diamond), `PartialEq` dedup (setting the same value is a
no-op), version-based staleness (a computed whose inputs changed but whose
*output* didn't does not wake its observers).

For **cross-thread** use, `foundation_signals` ships a `SignalHub` actor
(`valtron` feature): workers hold `RemoteSetter`/`RemoteGetter` handles,
commands apply FIFO with one stabilize per pump, and remote snapshots are
never glitchy. The UI thread stays single-threaded `Rc`/`RefCell` —
deliberately (WASM is single-threaded; no atomics tax on the hot path).

---

## 5. The receiver: queue → flush → ack

`InstructionReceiver` is the buffer between effects and the wire:

- effects **queue** `DomOp`s as they run;
- `Runtime::attach` registers a post-stabilize hook so every `stabilize()`
  ends with one **flush** — the whole batch encodes into a single arena
  allocation and one `host_apply` call;
- JS **acks** by disposing the arena slot after applying.

You rarely touch it beyond passing the handle into `html!`, but
`receiver.flush()` exists for manual control, and `MockProtocol` (see §9)
lets tests capture exactly what was sent.

For the highest-throughput path, `Runtime::builder().columnar()` swaps in the
`ColumnarReceiver`: effects then write string bytes *directly into column
buffers* as they queue — flush is a header write, with no intermediate
`Vec<DomOp>` at all.

---

## 6. Wire protocols

All protocols implement `ProtocolMethods` and are chosen at builder time.
The envelope's first byte is the protocol family; a VERSION byte demuxes
within it.

| protocol               | wire | what it is                                                                 |
|------------------------|------|----------------------------------------------------------------------------|
| `ColumnarV1`           | v1   | **default**: hand-written compact columnar layout — fixed-width columns (op kind, node id, …) + a shared text pool. 8-byte aligned by contract, so JS reads columns as `TypedArray` views **over live WASM memory** (true zero-copy; verified by tests that assert `view.buffer === wasm.memory.buffer`). |
| Arrow IPC (`foundation_arrow`) | v2 | real Apache Arrow RecordBatches for server-side/content-type consumers; read in JS with the bundled `apache-arrow.js`. |
| `BatchInstructionsV1`  |      | the V2 batch-operation codec (multiple op groups per message, `BATCH_OP_APPLY_DOM` for DOM payloads). |
| `JsonV1`               |      | human-readable DomOps; perfect for debugging the loop.                      |
| `MockProtocol`         |      | test double — records every flushed batch for assertions.                  |

**The alignment contract** (worth knowing if you implement a producer):
every encoder pads so the columnar header lands 8-byte aligned — the pure
encoder always pads, and the wasm framing computes the shim from the
absolute arena address. Misaligned input still *parses* (the JS parser falls
back to copying); only corrupt input fails, with typed decode errors.

---

## 7. The JS runtime (`runtimes/foundation-wasm-ui.js`)

A single ES module, no dependencies, importable piecemeal or used via the
auto-built `window.primal` namespace. The major exports:

- **`ColumnarParser`** — parses wire v1/v1.1; `zeroCopy: true` views when
  aligned, copy fallback otherwise.
- **`DomOpApplicator` + `NodeRegistry`** — applies all 19 `DomOp`s. Node ids
  0–3 are reserved ambient seeds (`0=head`, `1=body`, `2=html`, `3=theme
  style node`); Rust id allocation starts at 16.
- **`EventDispatcher`** — wires `primal:on*` attributes: `callback-N` ids →
  WASM dispatch, `primal:setter` → signal delivery, dot-paths (`"app.save"`)
  → JS scope functions; supports delegation and island-boundary
  `MutationObserver` cleanup.
- **`MorphDom`** — Datastar-style four-phase DOM morphing (id-aware reuse
  with a pantry for displaced nodes, form-state preservation) for
  server-rendered HTML patches.
- **`Transport`** factory — `fetch`, `sse`, `ws`, `chunked`, `worker`; plus
  request **batching** (`probeBatching`, id-multiplexed `/primal/messages`
  batches, WS/Worker queue coalescing).
- **`FetchEventSource` + `SseParser`** — SSE over `fetch()` with **any HTTP
  method/headers/body** (browser `EventSource` is GET-only). The parser is
  chunk-boundary-safe (split UTF-8 runes, CRLF split across chunks) and
  mirrors the Rust `foundation_netio` parser semantics exactly, so both ends
  of the wire agree.
- **`ProtocolHandler` + `Patcher` + `Hydrator`** — content-type → handler
  resolution and patch routing (`html`, `html-morph`, `json` signal patches,
  `arrow` DomOp columns).

### Web components

Three custom elements ship server-driven islands with zero app JS:

```html
<!-- Hydrate in place: scoped styles, scripts, event wiring. No network. -->
<primal-island>…server-rendered content…</primal-island>

<!-- One request, one response, rendered into a target. -->
<mount-data api="/api/profile" method="POST"
            data='{"id": 7}' target="#profile"></mount-data>

<!-- A continuous stream (default transport: SSE) until disconnect. -->
<mount-stream api="/feed" transport="sse" target="#timeline"></mount-stream>
```

Attributes: `api` (required), `data` (JSON), `transport`
(`fetch|sse|ws|chunked|worker`), `target` (CSS selector, `parent`, or omit
for self-replacement), `method`. Responses route by type — HTML morphs or
materializes, JSON patches signals, Arrow columns apply as DomOps.

---

## 8. Theming

`#[derive(ThemeTokens)]` turns a struct into CSS custom properties + utility
classes, with automatic dark-mode derivation (~80% luminance) unless you
specify `dark` explicitly:

```rust
use foundation_wasm_ui::{inject_theme_css, ThemeTokens};

#[derive(ThemeTokens)]
struct AppTheme {
    #[token(category = "color", light = "#3b82f6", dark = "#60a5fa")]
    primary: (),
    #[token(category = "spacing", value = "1rem")]
    gap: (),
}

// The derive generates `AppTheme::CSS` (custom properties + utilities).
// Queue it into <head> (uses the reserved theme style node):
inject_theme_css(&receiver, AppTheme::CSS);
```

---

## 9. Testing your UI

`MockProtocol` makes the whole loop assertable without a browser:

```rust
let mock = MockProtocol::new();
let sent = mock.sent_batches();              // Rc<RefCell<Vec<Vec<DomOp>>>>
let runtime = Runtime::builder()
    .protocol(mock)
    .memory(MemoryAllocations::new())
    .build();
runtime.attach(&signals);

// …mount, stabilize, then assert the exact DomOp stream:
assert!(matches!(sent.borrow()[0][0],
    DomOp::CreateElement { node_id: 16, .. }));
```

Running this crate's own suites:

```bash
# Rust (NOTE: the dev profile uses Cranelift — always test with uat)
cargo test --profile uat -p foundation_wasm_ui

# The JS runtime suite (node's built-in test runner)
node --test backends/foundation_wasm_ui/integration/test/
```

---

## 10. Rendering on the server

The macro expands to target-agnostic code (no `cfg(target_arch)` anywhere
in the expansion), so EVERYTHING here runs on a plain native server. Two
distinct server roles:

### 10.1 First-paint HTML — no runtime at all

The pure form + `to_markup()` serialize the same trees that drive the DomOp
channel (the morph contract — server markup and live DOM come from ONE
renderer, slot spans and all):

```rust
fn page(user: &str) -> String {
    let body = html! {
        <main class="app">
            <h1>{user}</h1>
            <mount-stream api="/live" target="#feed"></mount-stream>
            <div id="feed"></div>
        </main>
    };
    body.to_markup()   // → serve as text/html
}
```

Escaping, void elements, and the slot-`<span>` shape are handled; no
signals, no `App`, no allocation arena.

### 10.2 Live server-driven UI — `App::server()`

A server can run the FULL reactive loop and stream DOM updates to the
browser. On native, the FFI ship is a no-op stub, so the server presets
capture every flushed batch as a complete envelope-framed `Vec<u8>`
instead:

```rust
let (app, frames) = App::server();        // arrow-family wire v1 (default)
let (ctx, receiver) = app.context();
let (status, set_status) = ctx.signal(String::from("ready"));
let _ui = html! { ctx, receiver, <p class="status">{status.get()}</p> };
app.stabilize();

// Each frame is ready-to-ship wire bytes:
while let Some(frame) = frames.borrow_mut().pop_front() {
    // WebSocket: send as ONE BINARY frame (the envelope's protocol byte
    //   tells the client runtime what it is — feature 04 negotiation).
    // SSE: base64 the bytes under `event: arrow`.
    websocket.send_binary(&frame);
}
```

The client side is the existing `<mount-stream>`: it routes `arrow`-typed
results into `DomOpApplicator`, so the server's signal writes become DOM
mutations in the page — same loop, different side of the wire.

**Encoder choice** mirrors client negotiation: `App::server()` = arrow v1;
`App::server_with(JsonEncoder)` = readable `primal-json` streams;
`App::server_with(foundation_arrow::ArrowIpcEncoder)` = wire v2 real Arrow
IPC for standard tooling (server side needs no cargo feature — the encoder
comes from `foundation_arrow` directly).

**Lifecycle note:** one `App` per connection/session (its id space and
registry mirror one client DOM). Drop the `App` when the connection
closes — the root scope disposes every effect.

### 10.3 HTML patches

For morph-style updates without a wasm/runtime client, serve `text/html`
fragments (from `to_markup()`) to `mount-data`/`mount-stream` — the client
morphs them in (`html` / `html-morph` routing), preserving id-bearing
elements including slot spans.

---

## 11. Shipping: from `#[wasm_bin]` to a deployable directory

Annotate entrypoints, then let the build pipeline do discovery → compile →
wrappers → bundles:

```rust
#[wasm_bin]                      // main-thread app
pub fn app() { /* wiring from §2 */ }

#[wasm_worker]                   // web-worker entrypoint
pub fn heavy() { … }

#[wasm_service]                  // service-style entrypoint with routes
pub fn api() { … }
```

```bash
# This crate's own CLI (built on every native build of the crate):
ewe-wasm-bundle plan  --target path/to/your-crate     # dry run
ewe-wasm-bundle build --target path/to/your-crate --release --output dist
# (also available as `ewe_platform wasm-bundle …`; entrypoint scaffolding
#  lives in foundation_wasm's `ewe-wasm-bins` CLI.)
```

The output contains the `.wasm`, a JS wrapper per entrypoint (with a
`__EWE_WASM_LOAD__` seam for custom loading), optional single-file bundles
(wasm embedded as base64), and the runtime assets.

For bundler-less serving, the `embedded-js` feature embeds the assets in
your **server** binary:

```rust
use foundation_wasm_ui::embedded::{FOUNDATION_WASM_UI_JS, APACHE_ARROW_JS, FOUNDATION_WASM_JS};
// serve them from memory at /assets/… — no asset pipeline required.
```

---

## 12. Cargo features & targets

| feature       | default | effect                                                            |
|---------------|---------|-------------------------------------------------------------------|
| `embedded-js` | off     | `embedded` module: runtime JS (+ `apache-arrow.js`) as `&str` consts |

The crate is `#![no_std]` on wasm32. Build tooling (`build_tools`, `cli`, the
`ewe-wasm-bundle` binary) is **target-gated, not feature-gated**: every
native build carries it automatically; wasm32 builds never see it (the bin
degrades to a stub there).

---

## 13. Current limits (honest edition)

- **There is deliberately no `Component` lifecycle trait** — "it's all just
  functions" (spec-42 decision). Composition is `Render`/`Slot` +
  `<Fragment>`: a component takes a typed slot struct (`Slot` /
  `Option<Slot>` / `Vec<Slot>`), renders each slot where it wants it, and
  splices fragments with `<Fragment>{slots.menu.render(ctx, receiver)}
  </Fragment>` — including already-mounted reactive fragments (spliced by
  reference via `Html.runtime_id`). Text slots render as id-bearing
  `<span>`s in both forms (the morph contract), and `Html::to_markup()`
  serializes pure trees for server first-paint. Signal-driven placement
  (`<Show>`/`<For>`) is spec-42 feature 01.
- Pure-form `{slot}` values are evaluated once — they are values, not
  bindings; `<Fragment>` placement is once-at-mount (interiors stay
  reactive).
- The signal runtime is single-threaded by design; cross-thread access goes
  through `SignalHub`, not `Send` signals.

Design history, gap analyses, and per-feature verification live in
[`specifications/39-foundation-wasm-ui/`](../../specifications/39-foundation-wasm-ui/)
— `requirements.md` is the feature index; every feature directory has a
`status.md` with what shipped and why.
