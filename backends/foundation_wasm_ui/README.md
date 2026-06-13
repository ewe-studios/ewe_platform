# foundation_wasm_ui

The UI layer of the Ewe platform's owned WASM stack: DOM bindings, the `html!`
macro, reactive rendering over `foundation_signals`, a compile-time theme
system, and the wire protocols that ship DOM updates from Rust to the
browser — **without wasm-bindgen, a virtual DOM, or any JS framework**. The
Rust side never touches the DOM; it emits typed `DomOp` instructions over a
binary protocol, and the owned JS runtime (`runtimes/foundation-wasm-ui.js`)
applies them.

```rust
use foundation_wasm_ui::{html, App};

let app = App::new();                 // default: compact columnar wire v1 (zero-copy)
let (ctx, receiver) = app.context();  // the pair every reactive html!/component needs

let (count, set_count) = ctx.signal(0i64);
let _tree = html! { ctx, receiver,
    <button primal:onclick={set_count}>"Count: " {count.get()}</button>
};
app.stabilize();                      // → DomOps flush to JS: create nodes, set text, wire click
```

Every later `set_count.set(n)` (including clicks arriving from the browser)
re-runs exactly the affected slot effect and ships a minimal `SetText` — no
diffing, no re-render of the tree.

---

## Table of contents

- [Getting started: newbie → expert](#getting-started-newbie--expert)
- [Mental model](#mental-model)
- [`App` and its protocol presets](#app-and-its-protocol-presets)
- [The `html!` macro](#the-html-macro)
- [`IntoAttrValue` — attribute value conversion](#intoattrvalue--attribute-value-conversion)
- [Events: `MaybeCallback`, setters, and `Context::callback`](#events-maybecallback-setters-and-contextcallback)
- [Slots and composition: `Render`, `Slot`, `mount_*`](#slots-and-composition-render-slot-mount_)
- [`<Show>` / `<For>` — structural reactivity](#show--for--structural-reactivity)
- [The theme system](#the-theme-system)
- [Scoped styles](#scoped-styles)
- [Wire protocols](#wire-protocols)
- [The runtime: `Runtime`, builder, receiver, event bridge](#the-runtime-runtime-builder-receiver-event-bridge)
- [Rendering on the server](#rendering-on-the-server)
- [The JS runtime](#the-js-runtime)
- [Shipping and cargo features](#shipping-and-cargo-features)
- [Current limits](#current-limits)

---

## Getting started: newbie → expert

Seven steps, each a small runnable example that builds on the last. By the end
you can write reactive, themed, composed UI. Each step links into the reference
section that covers it in depth.

### 1. Hello world

An `App`, one `html!` tree, one `stabilize()`. `App::new()` wires up the whole
stack (signal runtime, root scope, protocol, receiver); `app.context()` hands
back the `(Context, receiver)` pair every reactive `html!` needs.

```rust
use foundation_wasm_ui::{html, App};

let app = App::new();
let (ctx, receiver) = app.context();

let _ui = html! { ctx, receiver,
    <div class="greeting"><h1>"Hello, world"</h1></div>
};
app.stabilize();   // builds the nodes and flushes them to JS as DomOps
```

That's the whole loop: build a tree, `stabilize()`, the runtime ships the
`DomOp` batch. See [`App` and its protocol presets](#app-and-its-protocol-presets)
and [The `html!` macro](#the-html-macro).

### 2. Reactivity

State lives in a **signal** — a `(getter, setter)` pair from `ctx.signal(..)`.
Read it in a `{expr}` slot with `.get()`; that read subscribes the slot, so when
the setter writes, **only that slot** re-renders (one `SetText`, no diffing).

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
This is `foundation_signals` underneath — see its README for the graph model.

### 3. Static vs reactive attributes

Three attribute kinds, distinguished by the brackets:

- **`attr="lit"`** — a literal, set once at build.
- **`attr=[expr]`** — **static-once**: evaluated a single time at mount, *no
  effect, no clone* — for owned config values (a `Cow`, a precomputed class).
- **`attr={expr}`** — **reactive**: wrapped in an effect, re-runs on signal
  change. Read signals with `.get()`.

And the **presence** form for the data-attribute styling contract: `then_some`
yields `Some` (sets the attribute) when on and `None` (removes it) when off, so
`[data-active]` in CSS matches *only* when actually active.

```rust
use std::borrow::Cow;

let (active, _set_active) = ctx.signal(false);
let css: Cow<'static, str> = Cow::from("panel large");

let _ui = html! { ctx, receiver,
    <section
        class="panel"                              // static literal
        data-variant=[css]                          // static-once: moved in, no clone
        data-count={count.get()}                    // reactive: re-runs on change
        data-active={active.get().then_some("")}    // presence: Some("") on, None off (removed)
    />
};
```

Full rules: [The `html!` macro](#the-html-macro) and
[`IntoAttrValue`](#intoattrvalue--attribute-value-conversion).

### 4. Events

Two ways a browser event reaches Rust, both resolved by the `MaybeCallback`
trait at compile time:

**(a) Two-way binding** — pass a `SignalSetter` straight to a value-carrying
event. For `String`/`bool`/numeric signals the setter already knows how to pull
`value`/`checked` off the event and write itself:

```rust
let (name, set_name) = ctx.signal(String::new());
let _ui = html! { ctx, receiver,
    <input value={name.get()} primal:onchange={set_name} />
};
```

**(b) `ctx.callback`** — the escape hatch for events that carry *no* value
(button clicks, image load/error). It registers a closure and returns a
`Callback` you attach like a setter:

```rust
let (count, set_count) = ctx.signal(0i64);
let inc = ctx.callback(move |_event| set_count.set(count.get() + 1));

let _ui = html! { ctx, receiver,
    <button primal:onclick={inc}>"+1"</button>
};
```

Details and the JS return leg: [Events](#events-maybecallback-setters-and-contextcallback).

### 5. Lists & conditionals

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

Keys identify instances, not data — see [`<Show>` / `<For>`](#show--for--structural-reactivity).

### 6. Theming

Hand a `theme!{}` to `App::new().theme(..)` and its CSS (custom properties, a
dark `@media` block, utility classes) is injected into `<head>` before the first
content batch.

```rust
use foundation_wasm_ui::{theme, App};

let app = App::new().theme(theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" },   // explicit dark
              accent:  "#10b981" }                              // dark auto-derived
    spacing { sm: 8px, md: 16px }                               // bare suffixed literals
    radius  { card: 8px }
});
let (ctx, receiver) = app.context();
// elements can now use --color-primary, .bg-primary, .p-md, .rounded-card, …
```

Full grammar and the calculable utility scales: [The theme system](#the-theme-system).

### 7. Composing components

A "component" is just a function returning `Html` that takes the
`(Context, receiver)` pair — there is no `Component` trait.

```rust
use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

fn counter(ctx: &Context, rcv: &SharedInstructionReceiver) -> Html {
    let (count, set_count) = ctx.signal(0i64);
    let inc = ctx.callback(move |_| set_count.set(count.get() + 1));
    html! { ctx, rcv,
        <button primal:onclick={inc}>"Count: " {count.get()}</button>
    }
}

let _ui = html! { ctx, receiver, <div>{counter(&ctx, &receiver)}</div> };
```

For ready-made headless primitives (button, toggle, field, avatar, …) built this
way, reach for **`foundation_ui_components`** — its README covers the catalog and
the slot/config/data-attribute conventions. For accepting *content* into your own
components, see [Slots and composition](#slots-and-composition-render-slot-mount_).

---

## Mental model

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

- **`foundation_ui_traits`** defines the shared vocabulary: the typed `Html`
  tree, `Part` (which slots are dynamic), the 19-variant `DomOp`, compact
  wire ids for known tags/attributes, the `IntoHtml`/`IntoAttrValue`
  conversions, and the `ProtocolEncoder` seam.
- **`foundation_signals`** is the reactive engine (height-ordered graph). It
  knows nothing about the DOM.
- **`foundation_wasm`** is the pure WASM↔JS ABI: arena memory, the uniform
  `host_apply(mem_id, ptr, len)` transport, function registry.
- **this crate** composes them: `App`, `html!`, the protocol implementations,
  the `InstructionReceiver`, the event bridge, the theme injector — plus the
  owned JS runtime file that is the *other half* of every contract here.

Nothing is hidden: a "component" is a function, an "update" is a signal
write, and the wire format is documented bytes you can parse yourself. There
is **no `Component` trait** by design (see [Current limits](#current-limits)).

---

## `App` and its protocol presets

`App` is the one struct that owns the whole five-object wiring (signal
runtime, root `Context`, UI `Runtime`, attach hook, and the receiver handle),
so a program never repeats the ritual.

### 1. How to use it

```rust
use foundation_wasm_ui::App;

let app = App::new();                 // the default: compact columnar wire v1
let (ctx, receiver) = app.context();  // cheap Rc-backed clones

let (count, set_count) = ctx.signal(0i64);
let _ui = foundation_wasm_ui::html! { ctx, receiver,
    <p class="count">{count.get()}</p>
};
app.stabilize();                      // run dirty effects; the runtime flushes the batch
```

Constructors (each returns an `App`, except where noted):

| preset | wire | use it for |
|--------|------|------------|
| `App::new()` / `App::columnar()` | compact columnar v1 | THE default — the live browser loop |
| `App::arrow()` | Apache Arrow IPC v2 | bulk batches / Arrow tooling (`arrow` feature, on by default) |
| `App::json()` | readable JSON `DomOp`s | debugging the loop |
| `App::mock()` → `(App, SentBatches)` | none (records ops) | tests — `SentBatches = Rc<RefCell<Vec<Vec<DomOp>>>>` |
| `App::server()` → `(App, CollectedFrames)` | compact columnar v1 frames | server-driven UI (native, no JS host) |
| `App::server_with(encoder)` → `(App, CollectedFrames)` | any Layer-1 encoder | server-driven UI on a negotiated wire |
| `App::with_protocol(p)` | any `ProtocolMethods` impl | escape hatch |

Accessors and lifecycle:

```rust
let (ctx, receiver) = app.context();   // the pair components need
let ctx_ref         = app.ctx();        // &Context — the root scope
let receiver        = app.receiver();   // SharedInstructionReceiver clone
let signals         = app.signals();    // &Rc<SignalsRuntime>
app.stabilize();                        // run effects + flush
let scope: foundation_signals::Context = app.scope(); // a CHILD scope (ctx.child())

// theming (see the theme section):
let app = App::new().theme(my_generated_theme);
let _ = app.get_theme();                // Option<&GeneratedTheme>
```

A component takes the `(Context, SharedInstructionReceiver)` pair explicitly —
there is no global, no thread-local:

```rust
use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, SharedInstructionReceiver};

fn counter(ctx: &Context, receiver: &SharedInstructionReceiver) -> Html {
    let (count, set_count) = ctx.signal(0i64);
    html! { ctx, receiver,
        <button primal:onclick={set_count}>{count.get()}</button>
    }
}
```

### 2. How it works

`with_protocol` builds the five objects in one place:

```rust
let signals = Rc::new(SignalsRuntime::new());   // the reactive graph
let ctx = Context::new(Rc::clone(&signals));    // root ownership scope
let runtime = Runtime::builder()                // protocol + memory + DomOp queue
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())
    .build();
runtime.attach(&signals);                       // flush-after-stabilize hook
let receiver = runtime.receiver();              // what html! queues onto
```

The protocol is a **value** choice behind the `ProtocolMethods` seam, not a
type parameter — so `App` is one concrete type no matter which wire it uses,
and downstream signatures never go generic. `App::server()` swaps the FFI ship
for a `FrameSinkV1` whose flushes land in a `CollectedFrames` queue; `App::mock()`
swaps in a `MockProtocol` that records `Vec<DomOp>` batches. `Context` and
`SharedInstructionReceiver` are both cheap `Rc`-backed handles, so cloning the
pair into a component is free.

### 3. Why it's there

Every program (and every test) needs the same wiring; before `App` it was five
manual lines that were easy to get subtly wrong (forgetting `attach`, mismatched
memory arenas). `App` makes the default path one line and centralises drop
order: dropping the `App` disposes the root `Context` (tearing down every effect)
before the runtime goes away. Named constructors make the protocol choice
explicit and discoverable instead of buried in a builder chain.

### 4. What it's NOT for

- Not a multi-document manager. **One `App` per browser DOM / per server
  connection** — its id space and node registry mirror exactly one client DOM.
- `App::scope()` returns a **child** scope for component-scoped teardown; it is
  not a second root and does not get its own runtime.
- The `mock`/`json` presets are for tests and debugging — never ship them as
  the production wire.
- `App` does not render anything by itself; you still call `html!` and
  `stabilize()`. It is wiring, not a framework lifecycle.

---

## The `html!` macro

`html!` parses real HTML syntax at compile time into the typed `Html` tree
(known tags/attributes resolve to compact wire ids; custom elements like
`<my-widget>` stay name-form). It has **two forms** detected by the first
token: a leading `<` is the pure form; `ctx, receiver, <…>` is the reactive
form.

### 1. How to use it

**Pure form** — `html! { <markup> }` → an `Html` value. No context, no
receiver, no reactivity. For server rendering, static fragments, and
composition:

```rust
use foundation_wasm_ui::html;

let h = html! { <div class="card"><h2>"Title"</h2></div> };
let markup: String = h.to_markup();   // serialize for first-paint HTML
```

Dynamic *values* are allowed via `{expr}` slots; they evaluate **once**,
through `IntoHtml`:

| slot expression type        | becomes                                        |
|-----------------------------|------------------------------------------------|
| `Html` / `&Html`            | inlined as a child element                     |
| `Option<Html>`              | the element, or an empty fragment (`None`)     |
| `Vec<Html>`                 | a tagless wrapper whose children are the items |
| `&str` / `String`           | a text node                                    |
| integers / floats / `bool`  | a text node via `Display`                      |

Plain functions returning `Html` nest arbitrarily:

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

**Reactive form** — `html! { ctx, receiver, <markup> }`. The first two
arguments are a `Context` and a `SharedInstructionReceiver` (or any expressions
evaluating to them — they are split at the first two top-level commas). This
form *mounts*: it allocates a contiguous block of wire ids, queues the build
ops on the receiver, and creates **one effect per dynamic slot**.

```rust
let (count, set_count) = ctx.signal(7i64);
let tree = html! { ctx, receiver, <div><span>{count.get()}</span></div> };
app.stabilize();          // CreateElement(div), CreateElement(span), SetText("7"), …
set_count.set(8);
app.stabilize();          // exactly one SetText("8") — nothing else re-runs
```

The reactive form still returns the same `Html` value (with `parts` and a
`runtime_id`), so you can inspect or splice what was mounted.

#### Attribute kinds (precise)

| syntax | kind | behaviour |
|--------|------|-----------|
| `name="lit"` | **static** | a string literal, set once; `class` rides the `CreateElement` op |
| `name` (bare) | **static boolean** | value `"true"` |
| `name={expr}` | **reactive** | wrapped in an effect; re-runs on signal change. Read signals with `.get()`. `Some` sets, `None` removes (presence contract — see `IntoAttrValue`) |
| `name=[expr]` | **static-once** | evaluated a single time at build, **no effect, no clone needed** — for owned config values (a `Cow`, a computed class). `Some` sets, `None` omits |
| `primal:onX={handler}` | **event** | one-time `AddEventListener`; see the events section |

```rust
let (on, set_on) = ctx.signal(false);
let class: std::borrow::Cow<'static, str> = compute_class();
html! { ctx, receiver,
    <input
        class="field"                          // static
        value={text.get()}                     // reactive — effect, re-runs
        data-mode=[class]                       // static-once — moved, no clone
        data-checked={on.get().then_some("")}  // presence: Some("") sets, None removes
        primal:onchange={set_on}               // event
    />
}
```

### 2. How it works

The parser walks `proc_macro2::TokenTree`s (Rust already tokenised the input —
there is zero runtime HTML parsing). Codegen then numbers the tree twice:

- an **element id** (depth-first over elements only) — the spec-visible
  `primal-id` / `Part::node_id`;
- a **runtime index** over all nodes — in reactive mode node N's wire id is
  `__base + N`, where `__base = ctx.allocate_id_block(total)`. Every macro
  invocation gets a **disjoint id block**, so instantiating the same component
  in a loop can never collide.

For each dynamic slot/attr the reactive form emits one effect; because effects
run immediately on creation (decision 008), the **initial** render arrives with
the first `stabilize()` too. Each slot expression therefore lives in exactly one
closure — a handle used in several slots needs a per-slot `.clone()` (plain Rust
move rules). Text slots become a dedicated `<span primal-id>` in *both* forms
(the morph contract), so `SetText` targets only that span and server markup
lines up with the live DOM.

| part kind                   | what mounts (reactive)                                  |
|-----------------------------|----------------------------------------------------------|
| `{signal.get()}` child slot | a `<span>` text node + an effect emitting `SetText`      |
| `attr={signal.get()}`       | an effect emitting `SetAttribute`/`RemoveAttribute`      |
| `attr=[expr]`               | a one-shot `SetAttribute` at mount (no effect)           |
| `primal:on*={handler}`      | one-time `AddEventListener` (+ `primal:setter` if any)   |

### 3. Why it's there

Components want to express DOM structure as HTML, but the stack is WASM-first
with zero runtime parsing. Compiling HTML to typed `Html` + `DomOp` builds gives
stable ids, fine-grained updates (one effect per slot, not a re-render), and a
single renderer that serves both SSR markup and the live DOM. The two forms let
the *same* template be a pure value (server) or a mounted reactive tree (client)
with no `cfg(target_arch)` in the expansion.

### 4. What it's NOT for

- **No template directives.** `@if`/`@for` do not exist; ordinary control flow
  is plain Rust around the macro (pure form) or `<Show>`/`<For>` (reactive form).
- Pure-form `{slot}` values are **evaluated once** — they are values, not
  bindings. Reactivity requires the reactive form.
- `<Show>`/`<For>` are **reactive-only** — using them in a pure tree is a
  compile error.
- Exactly **one root element** per `html!` — multiple roots are a compile error
  (wrap them in a parent or a `<Fragment>`).
- Capitalised tags are reserved built-ins (`Fragment`, `Show`, `For`); a custom
  element must be lowercase with a dash (`<my-widget>`).
- The macro cannot resolve types — it never inspects whether an event handler is
  a setter or a closure; that is `MaybeCallback`'s job at Rust compile time.

---

## `IntoAttrValue` — attribute value conversion

`IntoAttrValue` (defined in `foundation_ui_traits`, used by `html!` for `{}`
and `[]` attributes) maps an expression to `Option<Cow<'static, str>>`.

### 1. How to use it

```rust
// Plain values always SET the attribute:
html! { ctx, receiver, <input value={name.get()} /> };          // String  → Some
html! { ctx, receiver, <div data-count={count.get()} /> };      // i64     → Some via Display
html! { ctx, receiver, <input disabled={is_locked.get()} /> };  // bool    → Some("true"/"false")

// Option / then_some opt INTO presence semantics (Some sets, None removes/omits):
html! { ctx, receiver, <li data-checked={on.get().then_some("")} /> };   // None removes the attr
html! { ctx, receiver, <a href={maybe_href.clone()} /> };                // Option<String>
```

### 2. How it works

```rust
pub trait IntoAttrValue {
    fn into_attr_value(self) -> Option<Cow<'static, str>>;
}
```

`Some(v)` → `SetAttribute(v)`; `None` → `RemoveAttribute` (reactive) or omit
(pure/SSR). There is one blanket impl over `Option<T: IntoAttrValue>` plus leaf
impls for `&str`, `String`, `Cow<'static, str>`, every integer/float, and
`bool`. The string/numeric/`bool` leaves always return `Some`, so existing
`attr={value}` usage is unchanged; only `Option<T>` (and `bool::then_some`)
opts into presence.

### 3. Why it's there

The presence pattern (`[data-checked]` matching in CSS only when *truly*
present) needs an attribute that can be *absent*, not stringified to
`"false"`. A `bool` rendered as the literal `"false"` would still match
`[data-checked]`. `IntoAttrValue` is the seam that lets a single `name={expr}`
syntax cover both "always set this value" and "set-or-remove" without the macro
knowing the expression's type.

### 4. What it's NOT for

- Not for child content — that is `IntoHtml`. `IntoAttrValue` only produces an
  attribute string.
- Bare `bool` is **not** presence: `disabled={flag}` always sets `"true"` or
  `"false"`. Use `flag.then_some("")` for true presence semantics.
- It does not escape or validate the value as safe HTML; it is a string
  conversion, and the wire/JS side owns DOM-attribute setting.

---

## Events: `MaybeCallback`, setters, and `Context::callback`

A `primal:onX={handler}` attribute reaches Rust in one of two ways, resolved at
Rust compile time by the `MaybeCallback` trait — the macro never needs to know
which it got.

### 1. How to use it

```rust
use foundation_signals::EventData;
use foundation_wasm_ui::html;

// (a) A SignalSetter — two-way binding sugar. For value-carrying events
//     (text input, checkbox), the setter's DEFAULT callback extracts
//     value/checked and writes the signal.
let (name, set_name) = ctx.signal(String::new());
html! { ctx, receiver, <input value={name.get()} primal:onchange={set_name} /> };

let (on, set_on) = ctx.signal(false);
html! { ctx, receiver, <input type="checkbox" primal:onchange={set_on} /> };  // reads `checked`

// (b) ctx.callback(closure) — the escape hatch for events with NO convertible
//     value (button click, image load/error, Escape dismiss).
let (pressed, set_pressed) = ctx.signal(false);
let toggle = ctx.callback(move |_event: &EventData| set_pressed.set(!pressed.get()));
html! { ctx, receiver, <button primal:onclick={toggle}>"toggle"</button> };

// (c) A plain closure over &EventData — gets the raw payload, no signal wiring.
let log = |event: &EventData| { let _ = (&event.value, &event.checked, event.key_code); };
html! { ctx, receiver, <input primal:oninput={log} /> };
```

### 2. How it works

```rust
pub trait MaybeCallback {
    fn maybe_callback_id(&self) -> Option<u64>;
}
```

The generated code calls `MaybeCallback::maybe_callback_id(&handler)`:

- `SignalSetter<T>` and `Callback` return `Some(callback_id)` — the macro stamps
  a `primal:setter="<id>"` attribute alongside the one-time `AddEventListener`.
- A plain `Fn(&EventData)` closure returns `None` — its wiring is the JS event
  runtime's concern (it dispatches the closure-side event without a setter id).

On a value-carrying event the JS `EventDispatcher` builds an `EventData` JSON
(`type`, `primalId`, `value`, `checked`, `keyCode`, `modifiers`), writes it into
a global-arena slot, and calls the exported
`invoke_signal_callback(callback_id, allocation_id)`. The WASM side reads and
frees the slot, dispatches through the signals registry, and `stabilize()`s
**synchronously** — so the DOM answer to an input event goes out in the same
call chain. `ctx.signal` registers a default conversion callback automatically
for event-friendly `T` (`String`, `bool`, numerics); other types and valueless
events use `ctx.callback`. `Callback` and `SignalSetter` both live in
`foundation_signals` (re-exported via `app.signals()`).

### 3. Why it's there

Two-way binding's return leg has to deliver a browser event to the exact
`SignalSetter` that owns the slot. Signal callback ids are a separate namespace
from `foundation_wasm`'s function-call registry, so they get their own export
(`invoke_signal_callback`) instead of piggybacking on `invoke_callback`. Trait
dispatch (`MaybeCallback`) lets one `primal:onX={…}` syntax accept both a setter
and a closure without the macro resolving types. `ctx.callback` exists because
many real events (clicks, load/error) carry no convertible value, so the default
setter conversion has nothing to extract.

### 4. What it's NOT for

- `ctx.callback`'s closure is `FnMut(&EventData)`; it does not return a value and
  does not itself stabilize from native code — the event runtime stabilizes after
  delivery. In the closure you typically call one or more setters.
- A bare closure handler (`Fn(&EventData)`) is **not** a two-way binding — it
  gets `None`, so no `primal:setter` is stamped; it cannot write a signal unless
  it captures setters.
- `install_event_bridge` must be called once at app init (the live loop does this)
  or `invoke_signal_callback` logs "no event bridge installed" and drops the event.
- The default setter conversion only fires for value-carrying events; routing a
  button click through a `set_count` setter does nothing — use `ctx.callback`.

---

## Slots and composition: `Render`, `Slot`, `mount_*`

There is no `Component` trait — composition is functions plus the `Render`/`Slot`
pair and the `mount_*` splicing helpers.

### 1. How to use it

```rust
use foundation_signals::Context;
use foundation_ui_traits::Html;
use foundation_wasm_ui::{html, mount_fragment, mount_into, Render, Slot, SharedInstructionReceiver};

// A component accepting content via typed slot fields:
struct Card {
    header: Slot,            // required
    footer: Option<Slot>,    // optional
    items:  Vec<Slot>,       // repeated
}

fn card(ctx: &Context, rcv: &SharedInstructionReceiver, slots: Card) -> Html {
    let header = slots.header.render(ctx, rcv);
    let items: Vec<Html> = slots.items.iter().map(|s| s.render(ctx, rcv)).collect();
    let footer = slots.footer.map(|f| f.render(ctx, rcv));
    html! { ctx, rcv,
        <section class="card">
            <header>{header}</header>
            <ul>{items}</ul>
            {footer}
        </section>
    }
}

// Building slots: from an Html value, or lazily from a closure.
let _slot_from_html: Slot = html! { <b>"hi"</b> }.into_slot();
let _slot_lazy: Slot = Slot::lazy(|ctx, rcv| html! { ctx, rcv, <span>"deferred"</span> });

// Splicing helpers (the runtime behind <Fragment>):
let parent_id: u32 = 16;
let _root = mount_fragment(ctx, rcv, html! { <p>"static"</p> }, parent_id);
let _root = mount_into(ctx, rcv, "plain text", parent_id);     // anything IntoHtml
```

In templates, `<Fragment>{slots.header.render(ctx, rcv)}</Fragment>` splices
content (including already-mounted reactive fragments, spliced by reference).

### 2. How it works

```rust
pub trait Render {
    fn render(&self, ctx: &Context, rcv: &SharedInstructionReceiver) -> Html;
    fn into_slot(self) -> Slot where Self: Sized + 'static;
}
```

`Render` takes `&self` (object-safe, re-invokable — each call mounts a FRESH
instance). `Html` implements `Render` directly (it hands out a clone, no
effects). `Slot` erases any `Render` through `Box<dyn Render>`; closures enter
via `Slot::lazy` because a blanket `Fn` impl would overlap `impl Render for Html`
(coherence). When a slot renders, the effects it creates register under the
**placing** component's `ctx` — content lifecycle follows placement, not the
recipe.

`mount_fragment` splices under `parent_id`:
- an already-mounted reactive fragment (it has a `runtime_id`) splices with a
  single `AppendChild` — never rebuilt;
- a pure fragment is built here from a fresh `allocate_id_block`, with the same
  create/register/append op sequence the macro emits (its compile-time
  `primal-id`s are replaced by runtime ids);
- a tagless grouping node (`Vec<Html>`/`<Fragment>`) contributes its children.

`mount_before` is the same, but inserts before a reference sibling and returns
**every** top-level spliced id (`<Show>`/`<For>` need this to unmount multi-root
content). `mount_into` is `mount_fragment` over anything `IntoHtml`.

### 3. Why it's there

A component must be able to accept content it places itself, with that content's
effects owned by the *placing* component's scope, while the recipe stays a cheap,
shareable, re-invokable value. Erasing through `Box<dyn Render>` gives one field
type (`Slot`, `Option<Slot>`, `Vec<Slot>`) for required/optional/repeated content
without generics on every component signature.

### 4. What it's NOT for

- `Slot::lazy` takes `Fn`, not `FnOnce` — slots are re-invokable templates; a
  closure that consumes its captures will not compile. Clone from captures into
  each output.
- An already-mounted fragment must not be spliced twice — `mount_fragment` takes
  the `Html` by value precisely to consume its identity.
- `mount_fragment` of a **pure** fragment produces inert nodes (no effects);
  reactivity needs the reactive `html!` form. A pure fragment's `parts` are inert.
- Rendering a `Slot` twice is defined behaviour (two independent instances) — it
  is not idempotent and not a singleton.

---

## `<Show>` / `<For>` — structural reactivity

Signal-driven placement: mount/unmount a fragment when a condition flips, or
reconcile a keyed list. **Reactive form only.**

### 1. How to use it

```rust
use foundation_wasm_ui::{html, Slot};

// <Show when={…}> mounts content while the condition holds.
let (open, _set_open) = ctx.signal(false);
html! { ctx, receiver,
    <div>
        <Show when={open.get()}>
            { Slot::lazy(|ctx, rcv| html! { ctx, rcv, <p>"now visible"</p> }) }
        </Show>
    </div>
};

// <For each={…} key={…} render={…} /> — a keyed list. Self-closing.
let (items, _set_items) = ctx.signal(vec![1i64, 2, 3]);
html! { ctx, receiver,
    <ul>
        <For
            each={items.get()}
            key={|n: &i64| *n}
            render={|ctx: &Context, rcv: &SharedInstructionReceiver, n: &i64|
                html! { ctx, rcv, <li>{*n}</li> }}
        />
    </ul>
};
```

The `<Show>` child is exactly one `{ }` expression yielding an `impl Render`.
The `<For>` `render` closure has the contract
`fn(&Context, &SharedInstructionReceiver, &T) -> Html`.

### 2. How it works

Both expand to `mount_show` / `mount_for`, each owning an **anchored region**:
an empty id-bearing `<span>` marking the region's END; content inserts *before*
it. A single watcher effect reads the condition/items (the only tracked read);
all mounting runs inside `untracked` so content signals never become watcher
dependencies. Instances mount under fresh child scopes — dropping a scope
disposes that instance's effects.

`mount_for` reconciles per run: removed keys unmount; when the kept keys' relative
order is unchanged (append/remove/update — the common case) it emits **zero move
ops**; otherwise an end→start `InsertBefore` pass restores order. **Keys identify
instances** — a changed item with the same key does NOT re-render; data changes
belong in signals the item content reads.

```rust
pub fn mount_show(ctx: &Context, rcv: &SharedInstructionReceiver, parent_id: u32,
                  when: impl Fn() -> bool + 'static, content: impl Render + 'static);

pub fn mount_for<T, K>(ctx: &Context, rcv: &SharedInstructionReceiver, parent_id: u32,
                       each: impl Fn() -> Vec<T> + 'static,
                       key: impl Fn(&T) -> K + 'static,
                       render: impl Fn(&Context, &SharedInstructionReceiver, &T) -> Html + 'static)
where T: 'static, K: PartialEq + Clone + core::fmt::Debug + 'static;
```

### 3. Why it's there

Fragment *placement* was fixed at mount in the earlier feature; real UIs need the
*set* of mounted fragments to follow signals — conditionals and keyed lists. The
anchored-region + untracked-watcher design keeps the region's reactivity separate
from its content's reactivity (toggling a counter inside `<Show>` re-renders the
counter's slot, not the whole region).

### 4. What it's NOT for

- Reactive-only. Conditionals/lists in a **pure** tree are plain Rust (`Option`,
  `Vec<Html>`) around the macro.
- Keys are identity, not data: do not expect `<For>` to re-render an item because
  its value changed — read that value from a signal in the item content.
- Duplicate keys are an error (logged; last instance wins) — keys must be unique
  within a list.
- `<For>` is self-closing; `<Show>` requires exactly one `{ }` child and cannot
  be self-closing.
- The order-restore pass is correct but may over-move on reorders (an LIS
  minimal-move pass is documented future work) — it is not a no-op for arbitrary
  reorders, only for append/remove/update.

---

## The theme system

A theme is a table of design tokens (colors, spacing, radii, …) and the CSS they
imply (`:root` custom properties, a dark `@media` override, utility classes). It
is generated **one way** regardless of how tokens are declared.

### 1. How to use it

```rust
use foundation_wasm_ui::{theme, App};

// (a) The theme!{} macro — the headline API. Enforced block set; unknown
//     blocks are a compile error.
let app = App::new().theme(theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" },   // explicit dark
              accent:  "#10b981" }                              // dark auto-derived
    spacing { sm: 8px, md: 16px }                               // bare suffixed literals
    radius  { card: 8px }
    shadow  { card: "0 1px 3px rgba(0,0,0,0.1)" }               // quoted multi-part value
    font_size { base: 1rem }
    animation { fast: 250ms }
});
```

```rust
// (b) The runtime Theme builder — config-driven, no macro.
use foundation_theme::Theme;
let theme = Theme::new()
    .color("primary", "#3b82f6", Some("#60a5fa"))
    .spacing("md", "16px")
    .radius("card", "8px")
    .build();                 // -> GeneratedTheme
let app = App::new().theme(theme);
```

```rust
// (c) #[derive(ThemeTokens)] — LEGACY compile-time form.
use foundation_wasm_ui::ThemeTokens;
#[derive(ThemeTokens)]
struct AppTheme {
    #[token(category = "color", light = "#3b82f6", dark = "#60a5fa")]
    primary: (),
    #[token(category = "spacing", value = "1rem")]
    gap: (),
}
// generates AppTheme::CSS, AppTheme::new(), AppTheme::css_string()
foundation_wasm_ui::inject_theme_css(&receiver, AppTheme::CSS);
```

The enforced `theme!` blocks are: `colors`, `spacing`, `padding`, `margin`,
`radius`, `shadow`, `font_size`, `animation`. Value grammar per entry:
quoted strings (verbatim CSS), bare suffixed literals (`16px`, `250ms`,
`1.5rem`), or the `{ light, dark }` brace form.

The generator also emits a fixed set of **calculable utility scales** —
unit-suffixed and computed from the index — that coexist with named tokens by
namespace:

```html
<div class="opacity-50 text-56-rem h-50-vh p-16 m-8 w-full font-bold">…</div>
```

Named token classes follow the category: colors → `.bg-*`/`.text-*`/`.border-*`,
spacing → `.p-*`/`.m-*`, radius → `.rounded-*`, shadow → `.shadow-*`,
font-size → `.text-*`, animation → `.duration-*`.

### 2. How it works

`theme!{}`, `#[derive(ThemeTokens)]`, and `Theme::build()` all funnel into one
generator, `foundation_theme::theme_css(&[ThemeToken])`. The two macros call it
at expansion time and embed the result as a `&'static str`; the builder calls it
at runtime. The output is `:root { --color-primary: …; }`, a dark `@media`
override block (colors only — explicit `dark` verbatim, otherwise auto-derived at
~80% luminance from `#rrggbb`/`#rgb`), per-category utility classes, the built-in
utilities, and the calculable scales (opacity, w, h, p, m, gap, text, border,
font). `theme!{}` and the builder both yield a `GeneratedTheme` (CSS + token
table; `const`-capable via `from_static`).

`App::theme(generated)` takes ownership and **immediately** queues the
stylesheet's head-injection ops via `inject_theme_css`, so the theme CSS reaches
`<head>` before the first content batch. `App::get_theme()` returns
`Option<&GeneratedTheme>`. `inject_theme_css(&receiver, css)` queues the four-op
sequence (create style, register, set id, set text, append to `<head>`) targeting
the reserved theme style node.

### 3. Why it's there

A function-like macro can hold values inline (a `#[derive]` cannot), so `theme!{}`
reads like a struct of design tokens with no attribute mechanics to learn. One
generator means the macro, the legacy derive, and the runtime builder can never
drift apart. Injecting CSS as the first `DomOp` batch guarantees tokens exist
before any element references them.

### 4. What it's NOT for

- **Not a full CSS engine / utility framework.** The scales are a fixed,
  calculable set; arbitrary one-off values use a named token or an inline
  `style=""` attribute — there is no `[arbitrary]` value syntax.
- `theme!{}` blocks are a closed set — an unknown block name is a compile error,
  not a custom category. (The runtime `Theme::token` is the escape hatch for
  custom categories.)
- Dark-mode auto-derivation only applies to **colors** given as hex
  (`#rrggbb`/`#rgb`); `rgb()`/`var()` or non-color tokens get no auto dark value.
- `#[derive(ThemeTokens)]` is legacy/back-compat — prefer `theme!{}` or the
  builder for new code.
- Injecting CSS is a one-shot at startup; the theme is not itself reactive
  (changing tokens means re-injecting, not signal updates).

---

## Scoped styles

A `<style primal:style>` child holds a CSS **string literal** scoped to its
parent component at compile time.

### 1. How to use it

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

### 2. How it works

At compile time the macro extracts every `<style primal:style>` child, combines
their CSS per parent, and rewrites selectors against the parent's identity
(`#id` wins, else first `.class`); `:parent` targets the host element. In the
**pure** form the rewritten CSS stays inline as a `<style data-primal-scoped>`
child; in the **reactive** form it is queued as head-injection ops (appended to
the reserved `<head>` ambient node, ids 0–3 reserved by the JS `NodeRegistry`:
`0=head`, `1=body`, `2=html`, `3=theme style node` — `THEME_STYLE_NODE_ID`,
`HEAD_NODE_ID`). Rust id allocation for content starts at 16.

### 3. Why it's there

Components want co-located styles that cannot leak. Doing the scoping at compile
time means zero runtime CSS work and selectors that are already namespaced to the
component's identity before they ship.

### 4. What it's NOT for

- The content **must be a single quoted string literal** — raw CSS (hex colors,
  etc.) does not tokenize as Rust.
- The parent must have an `id` or `class` to scope against; otherwise it is a
  compile error.
- It is not dynamic — the CSS is fixed at compile time, not signal-driven.

---

## Wire protocols

All protocols implement `ProtocolMethods` and are chosen at builder time (or via
an `App` preset). The envelope's first byte is the protocol family; a version
byte demuxes within it.

### 1. How to use it

```rust
use foundation_wasm_ui::{App, ColumnarV1, JsonV1, MockProtocol, Runtime};
use foundation_wasm::MemoryAllocations;

// Usually via App presets:
let app = App::new();          // ColumnarV1 (default)
let app = App::json();         // JsonV1
let (app, sent) = App::mock(); // MockProtocol; `sent: Rc<RefCell<Vec<Vec<DomOp>>>>`
# #[cfg(feature = "arrow")]
let app = App::arrow();        // ArrowIpcV2

// Or directly on the builder:
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())
    .build();
```

| protocol | byte/version | what it is |
|----------|--------------|------------|
| `ColumnarV1` | byte 1, v1 | **default** — hand-written compact columnar layout: fixed-width columns + a shared text pool, 8-byte aligned so JS reads columns as `TypedArray` views **over live WASM memory** (true zero-copy) |
| `ArrowIpcV2` (`arrow` feature) | byte 1, v2 | real Apache Arrow IPC `RecordBatch`es for server/Arrow-tooling consumers; read in JS via `apache-arrow.js` |
| `BatchInstructionsV1` | byte 0 | the V2 batch-operation codec (multiple op groups per message; `BATCH_OP_APPLY_DOM` for DOM payloads; `DomOpsBatch`) |
| `JsonV1` | byte 2 | human-readable `DomOp`s — debugging the loop |
| `MockProtocol` | byte 255 | test double — records every flushed batch and ACK |
| `FrameSinkV1` / `FrameSink<E>` | columnar v1 / any encoder | server-side: every flush lands as an envelope-framed `Vec<u8>` in a `CollectedFrames` queue |

### Wire performance (measured)

`wire_bench` (release; rerun with `cargo test --release -p foundation_wasm_ui
--test wire_bench -- --ignored --nocapture`):

| batch | compact columnar (v1) | Apache Arrow IPC (v2) | size v2/v1 |
|-------|------------------------|------------------------|------------|
| 10 DomOps | 674 ns enc · 458 ns dec · 344 B | 5.3 µs · 3.0 µs · 2378 B | 6.9× |
| 100 DomOps | 3.8 µs · 5.7 µs · 3.0 KB | 10 µs · 7.0 µs · 5.1 KB | 1.69× |
| 1,000 DomOps | 28 µs enc · 58 µs dec · 30 KB | 45 µs enc · **47 µs dec** · 33 KB | 1.08× |
| 10,000 DomOps | 259 µs · 580 µs · 311 KB | 430 µs · **439 µs** · 320 KB | 1.03× |

The compact columnar **dominates the loop's real traffic** (small UI deltas) by
~7× on latency and bytes — Arrow IPC pays a ~2 KB schema/flatbuffers framing
floor per message. At bulk sizes they converge and Arrow wins decode. Hence the
defaults: `App::new()` = compact columnar for the loop; `App::arrow()` /
`server_with(ArrowIpcEncoder)` for bulk batches or standard Arrow tooling.

### 2. How it works

Each protocol pairs a Layer-1 `ProtocolEncoder<Vec<DomOp>>` with the Layer-2
`ProtocolHandler` transport. `encode_and_write` encodes the ops, frames them in a
`WasmEnvelope` (protocol byte + version + `memory_id`) inside one arena slot, and
returns the slot + its live address; `encode_and_send` then ships
`(memory_id, ptr, len)` through the single uniform `host_apply` import. JS reads
the protocol byte from the buffer and `dispose_allocation`s the slot. The
write/send split exists for the global-arena path: `host_apply` synchronously
re-enters WASM (the JS ACK locks the arena), so the ship must happen after the
lock is released.

**Alignment contract:** every encoder pads so the columnar header lands 8-byte
aligned (the pure encoder always pads; the wasm framing computes the shim from
the absolute arena address). Misaligned input still parses (the JS parser falls
back to copying); only corrupt input fails, with typed decode errors.

### 3. Why it's there

Layer 1 (encode) and Layer 2 (transport) are deliberately separate;
`ProtocolMethods` is the Layer-3 seam that composes "encode these ops and ship
them" so the receiver never cares which wire is configured. Multiple formats
exist because the loop's traffic (tiny deltas) and bulk/server traffic have
opposite cost profiles, and debugging/testing want readable and recordable wires.

### 4. What it's NOT for

- `MockProtocol` (byte 255) and `JsonV1` are for tests/debugging — not the
  production wire.
- The `arrow` feature can be turned off for size-sensitive artifacts; `App::arrow()`
  then does not exist. Server-side Arrow needs no feature (the encoder comes from
  `foundation_arrow` directly via `server_with`).
- Do not call `encode_and_send` while holding the global arena lock — use
  `encode_and_write` + `send_to_js`.
- The wire is for `DomOp` batches; it is not a general RPC channel.

---

## The runtime: `Runtime`, builder, receiver, event bridge

`Runtime` owns the `InstructionReceiver` and wires it into the reactive loop.
`App` builds one for you; this is the lower layer if you need manual control.

### 1. How to use it

```rust
use foundation_wasm_ui::{ColumnarV1, Runtime, SharedInstructionReceiver, install_event_bridge};
use foundation_wasm::MemoryAllocations;
use foundation_signals::Runtime as SignalsRuntime;
use std::rc::Rc;

let signals = Rc::new(SignalsRuntime::new());
let runtime = Runtime::builder()
    .protocol(ColumnarV1::new())
    .memory(MemoryAllocations::new())   // OR .global_arena() for the live JS loop
    .build();
runtime.attach(&signals);               // flush after every stabilize
let receiver: SharedInstructionReceiver = runtime.receiver();

// For the two-way-binding return leg, install the bridge once:
install_event_bridge(Rc::clone(&signals));

// Highest-throughput path: columnar-native accumulation (no Vec<DomOp>):
let runtime = Runtime::builder().columnar().global_arena().build();
```

`SharedInstructionReceiver` is the cloneable handle effects capture:
`receiver.queue(op)`, `receiver.flush() -> Option<SendResult>`,
`receiver.ack(memory_id)`, `pending_count()`, `flush_count()`.

`invoke_signal_callback(callback_id, allocation_id)` and `install_event_bridge`/
`uninstall_event_bridge` are the event return-leg API (see the events section).

### 2. How it works

The receiver sits in `Rc<RefCell<…>>`; clones are cheap and single-threaded.
`Runtime::attach` registers a `NotificationManager` on the signals runtime that
calls `flush()` after every `stabilize()` completes — so the full loop is
`set() → stabilize() → effects queue DomOps → manager flushes → protocol encodes
→ one host_apply → JS applies + ACKs`. The builder requires a protocol and one of
`memory(..)` / `global_arena()` (it panics otherwise). `.columnar()` swaps in a
`ColumnarReceiver` that writes string bytes straight into column buffers as ops
queue — flush is just a header write, with no intermediate `Vec<DomOp>` (and it
implies `ColumnarV1`, so do not also set `.protocol(..)`).

### 3. Why it's there

Something must own the receiver and wire it into the loop exactly once,
automatically, with no global state. `Runtime` + `attach` is that glue;
`SharedInstructionReceiver` is the handle that lets effects queue ops without
knowing about protocols, arenas, or the FFI.

### 4. What it's NOT for

- Single-threaded by design (WASM is single-threaded; the `Rc`/`RefCell` handles
  are not `Send`). Cross-thread signal access goes through `foundation_signals`'
  `SignalHub`, not this runtime.
- `.global_arena()` and `.memory(..)` are mutually exclusive; `.columnar()` and
  `.protocol(..)` are mutually exclusive — mixing them panics at build.
- `install_event_bridge` is process-global (single-threaded contract); installing
  again replaces the previous runtime.
- You usually do not touch the receiver directly — `App` and `html!` handle it.

---

## Rendering on the server

The macro expands to target-agnostic code (no `cfg(target_arch)` in the
expansion), so everything here runs on a plain native server.

### First-paint HTML — no runtime at all

The pure form + `to_markup()` serialize the same trees that drive the DomOp
channel (the morph contract — server markup and live DOM come from one renderer,
slot spans and all):

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

### Live server-driven UI — `App::server()`

A native server runs the full reactive loop and ships each flushed batch as
ready-to-send wire frames (on native, `host_apply` is a no-op stub, so the server
preset captures frames instead of dropping them):

```rust
let (app, frames) = App::server();        // compact columnar wire v1
let (ctx, receiver) = app.context();
let (status, set_status) = ctx.signal(String::from("ready"));
let _ui = html! { ctx, receiver, <p class="status">{status.get()}</p> };
app.stabilize();

while let Some(frame) = frames.borrow_mut().pop_front() {
    // WebSocket: send as ONE BINARY frame (the envelope's protocol byte tells
    //   the client runtime what it is). SSE: base64 under `event: arrow`.
    // websocket.send_binary(&frame);
}
```

`App::server_with(JsonEncoder)` streams readable `primal-json`;
`App::server_with(foundation_arrow::ArrowIpcEncoder)` streams real Arrow IPC
(server side needs no cargo feature — the encoder comes from `foundation_arrow`
directly). **One `App` per connection/session**; drop it when the connection
closes (the root scope disposes every effect). The client side is the existing
`<mount-stream>`, which routes the frames into `DomOpApplicator`.

For morph-style updates without a wasm client, serve `text/html` fragments from
`to_markup()` to `mount-data`/`mount-stream` — the client morphs them in,
preserving id-bearing elements including slot spans.

---

## The JS runtime

`runtimes/foundation-wasm-ui.js` is a single dependency-free ES module — the
*other half* of every contract above. Major exports:

- **`ColumnarParser`** — parses wire v1; zero-copy `TypedArray` views when
  aligned, copy fallback otherwise.
- **`DomOpApplicator` + `NodeRegistry`** — applies all 19 `DomOp`s. Node ids 0–3
  are reserved ambient seeds (`0=head`, `1=body`, `2=html`, `3=theme style`);
  Rust id allocation starts at 16.
- **`EventDispatcher`** — wires `primal:on*`: `primal:setter` → signal delivery
  via `invoke_signal_callback`, `callback-N` ids → WASM dispatch, dot-paths →
  JS scope functions; supports delegation and island-boundary cleanup.
- **`MorphDom`** — Datastar-style DOM morphing for server-rendered HTML patches.
- **`Transport`** factory — `fetch`, `sse`, `ws`, `chunked`, `worker`; request
  batching and queue coalescing.
- **`FetchEventSource` + `SseParser`** — SSE over `fetch()` with any HTTP
  method/headers/body, chunk-boundary-safe, mirroring the Rust `foundation_netio`
  parser semantics.
- **`ProtocolHandler` + `Patcher` + `Hydrator`** — content-type → handler
  resolution (`html`, `html-morph`, `json` signal patches, `arrow` DomOp columns).

### Web components

Three custom elements ship server-driven islands with zero app JS:

```html
<primal-island>…server-rendered content…</primal-island>
<mount-data api="/api/profile" method="POST" data='{"id":7}' target="#profile"></mount-data>
<mount-stream api="/feed" transport="sse" target="#timeline"></mount-stream>
```

Attributes: `api` (required), `data` (JSON), `transport`
(`fetch|sse|ws|chunked|worker`), `protocol` (`html|json|arrow` override),
`target` (CSS selector / `parent` / omit for self-replacement), `method`.
Protocol negotiation precedence: attribute > content-type/event-name > channel
default. WebSocket binary frames are authoritative — the envelope header says
what they are. Malformed binary surfaces a typed error, never a silent JSON
attempt.

---

## Shipping and cargo features

Annotate entrypoints, then let the build pipeline do discovery → compile →
wrappers → bundles:

```rust
#[wasm_bin]      pub fn app()   { /* App wiring */ }   // main-thread app
#[wasm_worker]   pub fn heavy() { /* … */ }            // web-worker entrypoint
#[wasm_service]  pub fn api()   { /* … */ }            // service-style entrypoint
```

```bash
# This crate's own CLI (built on every native build):
ewe-wasm-bundle plan  --target path/to/your-crate
ewe-wasm-bundle build --target path/to/your-crate --release --output dist
```

The output has the `.wasm`, a JS wrapper per entrypoint (with a
`__EWE_WASM_LOAD__` seam), optional single-file bundles (wasm as base64), and
runtime assets. For bundler-less serving, the `embedded-js` feature embeds the
assets in your server binary:

```rust
# #[cfg(feature = "embedded-js")]
use foundation_wasm_ui::embedded::{FOUNDATION_WASM_UI_JS, APACHE_ARROW_JS, FOUNDATION_WASM_JS};
```

| feature | default | effect |
|---------|---------|--------|
| `arrow` | **on** | `ArrowIpcV2` / `App::arrow()` — real Apache Arrow IPC (wire v2) |
| `embedded-js` | off | `embedded` module: runtime JS (+ `apache-arrow.js`) as `&str` consts |

The crate is `#![no_std]` on wasm32. Build tooling (`build_tools`, `cli`, the
`ewe-wasm-bundle` binary) is **target-gated, not feature-gated**: every native
build carries it; wasm32 builds never see it (the bin degrades to a stub).

---

## Current limits

- **No `Component` lifecycle trait** — "it's all just functions". Composition is
  `Render`/`Slot` + `<Fragment>`; structural reactivity is `<Show>`/`<For>`.
- Pure-form `{slot}` values are evaluated once — they are values, not bindings.
- The signal runtime is single-threaded by design; cross-thread access goes
  through `foundation_signals`' `SignalHub`, not `Send` signals.
- `<For>`'s reorder path is correct but may over-move (LIS minimal-move pass is
  future work).

Design history, gap analyses, and per-feature verification live in
[`specifications/39-foundation-wasm-ui/`](../../specifications/39-foundation-wasm-ui/)
and [`specifications/42-ui-component/`](../../specifications/42-ui-component/).

### Testing the loop

`MockProtocol` makes the whole loop assertable without a browser:

```rust
let (app, sent) = App::mock();        // sent: Rc<RefCell<Vec<Vec<DomOp>>>>
let (ctx, receiver) = app.context();
let _ui = html! { ctx, receiver, <div><span>{0i64}</span></div> };
app.stabilize();
assert!(matches!(sent.borrow()[0][0], DomOp::CreateElement { .. }));
```

```bash
# Rust (the dev profile uses Cranelift — test with uat):
cargo test --profile uat -p foundation_wasm_ui
# The JS runtime suite:
node --test backends/foundation_wasm_ui/integration/test/
```
