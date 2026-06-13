# foundation_signals

**Inpesired by Ryan Carniato and His team**: See https://www.youtube.com/watch?v=drLX0yTKP04

The reactive graph for the Ewe UI stack: fine-grained signals, computeds, and
effects with **glitch-free, height-ordered** propagation through a single
`stabilize()` flush. `(getter, setter)` tuples, explicit scoped teardown, and a
JS-interop callback registry for two-way binding.

No DOM, no transport, no framework. It runs on any Rust target — native, WASM,
server — and knows nothing about HTML. The UI layers (`foundation_wasm_ui`)
build on top: effects queue `DomOp`s, the `html!` macro reads setter callback
ids. This crate just owns the graph.

> Design rationale: `specifications/39-foundation-wasm-ui/` (decisions 002/003/
> 008/029) and `specifications/42-ui-component/` (feature 03 — `Context` handles).

## Contents
- [Mental model](#mental-model)
- [`Context` — scopes & teardown](#context--scopes--teardown)
- [`signal` — `(getter, setter)`](#signal--getter-setter)
- [`computed`](#computed)
- [`effect`](#effect)
- [`Context::callback` & `Callback` — the valueless-event escape hatch](#contextcallback--callback--the-valueless-event-escape-hatch)
- [`EventData` / `Modifiers`](#eventdata--modifiers)
- [`Runtime` — the graph, callbacks, `stabilize`](#runtime--the-graph-callbacks-stabilize)
- [`SignalHub` — cross-thread signals (native)](#signalhub--cross-thread-signals-native)
- [What this crate is NOT for](#what-this-crate-is-not-for)

---

## Mental model

```rust
use std::rc::Rc;
use foundation_signals::{Context, Runtime};

let runtime = Rc::new(Runtime::new());
let ctx = Context::new(Rc::clone(&runtime));

let (count, set_count) = ctx.signal(0);
let doubled = ctx.computed(move || count.get() * 2);
ctx.effect(move || { let _ = doubled.get(); });   // runs NOW (decision 008)

set_count.set(5);
set_count.set(21);          // coalesced — nothing propagated yet
runtime.stabilize();        // ONE flush: doubled recomputes once, the effect re-runs once, sees 42
```

Three rules carry the whole crate:

1. **Writes never propagate inline.** `set()` marks observers dirty and bumps a
   version counter; nothing recomputes until `stabilize()`. Many writes between
   stabilizes coalesce into one flush.
2. **`stabilize()` is the only propagation entry point.** It drains dirty nodes
   in **height order** (a node never runs before the things it depends on), so
   diamonds evaluate each node exactly once — no glitches, no double-fires.
3. **Reads inside an evaluation subscribe automatically.** Calling `get()` while
   an effect/computed is running records the dependency; the dependency set is
   re-diffed on every run, so subscriptions are always current.

---

## `Context` — scopes & teardown

A `Context` is an ownership scope over the shared graph. Everything you create
through it (`signal` / `computed` / `effect`) is owned by it and disposed when
the scope's last handle drops.

```rust
let ctx = Context::new(Rc::clone(&runtime));   // root scope
let child = ctx.child();                        // sub-scope

let (n, set_n) = child.signal(0);
child.effect(move || { let _ = n.get(); });

drop(child);   // disposes the signal + effect created above — and any of ITS children, recursively
```

- **`Context::new(runtime)`** — a root scope on a shared `Rc<Runtime>`.
- **`child()`** — a sub-scope disposed when *it* drops, and also (recursively)
  when its parent drops first.
- **`Clone`** — a clone is **another handle to the same scope**, not a new
  scope. Signals created through any handle belong to the one scope; disposal
  happens when the **last handle** drops.
- **`on_cleanup(f)`** — register a teardown closure. Called *inside* an effect's
  evaluation it attaches to that effect (runs before each re-run and at
  disposal); called outside, it attaches to the scope and runs once at disposal.
- **`untracked(f)`** — run `f` with dependency tracking suspended (reads inside
  subscribe to nothing).
- **`runtime()`** — the shared `&Rc<Runtime>` (for `stabilize()`, etc.).
- **`allocate_id_block(count)`** — reserve a contiguous block of instance ids
  (used by the `html!` macro to give every mounted template a disjoint id range).

Disposal is recursive and double-dispose is a no-op at every level. A dispose
that lands mid-`stabilize()` is deferred until the flush drains, so it can never
corrupt the in-flight walk.

### How it works

A context records the `NodeId`s it created plus its children's inner records.
On drop it runs scope-level cleanups, disposes its owned nodes through the
runtime (running effect cleanups, deferring if a stabilize is in flight), then
recurses into children.

### Why it's there

The graph is unified — one runtime owns all nodes — but components need a scope
whose drop tears down exactly what it created, cascading through children,
without any global registry or thread-local magic (decision 003).

---

## `signal` — `(getter, setter)`

`ctx.signal(initial)` returns a `(SignalGetter<T>, SignalSetter<T>)` tuple
(decision 029). Both handles are cheap `Rc`-backed clones; the value lives once,
typed, behind a `RefCell`.

```rust
let (count, set_count) = ctx.signal(0i64);

count.get();             // current value; subscribes if read inside an effect/computed
count.get_untracked();   // current value WITHOUT subscribing — for event handlers

set_count.set(5);                       // marks observers dirty IF the value differs (PartialEq)
set_count.update(|n| *n += 1);          // in-place; dirties only if the result differs
let id: u64 = set_count.callback_id();  // the interop id the html! macro stamps (see below)
```

`T` must be `Clone + PartialEq + 'static`. `set()` and `update()` compare with
`PartialEq` and do nothing if the value is unchanged, so redundant writes never
dirty the graph.

### The default event-conversion callback

When you create a signal, the setter is assigned an interop **`callback_id`**
immediately (G17). For **event-friendly `T`** — `String`, `bool`, and the
numeric primitives (`i8`…`i64`, `u8`…`u64`, `isize`, `usize`, `f32`, `f64`) — a
default callback is **registered right away** that converts an incoming
[`EventData`] into `T` and calls `set()`:

- `String` takes the event's `value`;
- `bool` prefers `checked`, else parses `value`;
- numerics parse `value`.

This is what makes a text input's `primal:onchange={set_value}` "just work" as
two-way binding — the browser event lands on the setter with no extra wiring.
Other `T` get no default callback; they obtain one via
`Runtime::register_callback` (the `html!` two-way-binding codegen does this).

### How it works

The value lives in the typed `SignalStorage` (`RefCell<T>`); the graph node
carries only observers, a version, and the callback id. `get()` reports the read
to the runtime (dependency tracking) and clones the value; `set()` compares,
writes, and asks the runtime to mark observers — propagation always waits for
`stabilize()`.

---

## `computed`

A derived value that caches and recomputes only when a dependency **actually
changed**.

```rust
let (first, _set_first) = ctx.signal(String::from("Ada"));
let (last, _set_last)  = ctx.signal(String::from("Lovelace"));

let full = ctx.computed(move || format!("{} {}", first.get(), last.get()));

full.get();             // "Ada Lovelace" — seeded at creation, subscribes if read inside an effect
full.get_untracked();   // cached value, no subscription
```

`T: Clone + PartialEq + 'static`. The closure is evaluated **once at creation**
(tracked) to seed the cache and discover dependencies. During `stabilize()` it
re-evaluates only when a direct dependency's version moved, then compares the new
result against the cache with `PartialEq` — so a recompute that produces the same
value does **not** dirty downstream nodes. Computeds may depend on computeds (the
height order makes diamonds safe).

---

## `effect`

A side-effecting closure that re-runs when its dependencies change. This is the
extension point the DOM layer builds on.

```rust
let (count, set_count) = ctx.signal(0i64);

ctx.effect(move || {
    println!("count is now {}", count.get());   // the .get() subscribes this effect to `count`
});                                              // runs IMMEDIATELY (prints "count is now 0")

set_count.set(1);
runtime.stabilize();   // effect re-runs, prints "count is now 1"
```

Effects run **immediately on creation** (decision 008) — so the initial
side-effect happens up front — then re-run during `stabilize()` whenever a tracked
dependency changed. Re-runs are **glitch-free**: an effect downstream of several
signals that all changed in one batch runs exactly once, after its inputs settle.

To clean up between runs, call `Context::on_cleanup` from *inside* the effect; it
attaches to that effect and runs before the next re-run and at disposal. Because
effect closures are `'static` and cannot borrow the context, capture a runtime
clone and use `Runtime::on_cleanup_active`.

### The `Effect` trait

`Effect` is a **lifecycle contract** (`register` on creation, `unregister` on
drop) for concrete effect *types* in downstream crates — e.g.
`foundation_wasm_ui`'s DOM bindings, which *are* effects that bridge to the DOM
(decision 004). The graph itself only runs closures; this trait is the seam, not
a runtime requirement.

---

## `Context::callback` & `Callback` — the valueless-event escape hatch

The default setter callback (above) only fires for events that carry a
convertible `value`/`checked` — text inputs, checkboxes, radios. **Many DOM
events carry no such value**: button clicks, image `load`/`error`, focus/blur,
Escape-to-dismiss. For those, register an arbitrary closure with
`ctx.callback`:

```rust
use foundation_signals::EventData;

let (pressed, set_pressed) = ctx.signal(false);

// A handler that writes setters itself — returns a Callback carrying the interop id.
let toggle: foundation_signals::Callback =
    ctx.callback(move |_event: &EventData| set_pressed.set(!pressed.get()));

let id: u64 = toggle.callback_id();   // what the html! macro stamps as `primal:setter`
```

`ctx.callback(f)` takes an `FnMut(&EventData) + 'static`, registers it under a
fresh interop id, and returns a copyable `Callback` carrying that id.
`foundation_wasm_ui` implements `MaybeCallback` for `Callback`, so `html!`
stamps it exactly like a `SignalSetter` and the JS event runtime delivers the
event through the same signal bridge. The closure typically writes one or more
setters; the event runtime calls `stabilize()` after delivery.

---

## `EventData` / `Modifiers`

The wire shape of a delivered DOM event — plain serde structs matching the JS
`buildEventData` object field-for-field.

```rust
use foundation_signals::{EventData, Modifiers};

let data = EventData::with_value("change", "hello");   // common "value changed" shape

let _ = (
    &data.event_type,   // "click", "change", "input", …
    &data.primal_id,    // Option<String> — the element's primal-id
    &data.value,        // Option<String> — input/select/textarea value
    &data.checked,      // Option<bool>   — checkbox/radio state
    &data.key_code,     // Option<u32>    — keyboard events
    &data.modifiers,    // Modifiers { alt, ctrl, shift, meta }
);
```

Every field except `event_type` is optional — different event kinds carry
different payloads. `Modifiers` is four independent bools (the domain shape, not
a state enum).

---

## `Runtime` — the graph, callbacks, `stabilize`

`Runtime` is the unified reactive graph: it owns the nodes, the dirty bookkeeping,
the version counter, and the callback registry. Create it once, wrap it in `Rc`,
and pass it explicitly — it is **never** a thread-local singleton.

```rust
let runtime = Rc::new(Runtime::new());

// Callback registry (two-way binding / G17):
runtime.register_callback(id, |data: EventData| { /* … */ });  // register or replace
let fired = runtime.invoke_callback(id, EventData::default()); // dispatch; false for stale ids
let ok    = runtime.invoke_callback_json(id, "{\"type\":\"click\"}");  // JSON leg
let live  = runtime.has_callback(id);

runtime.stabilize();   // flush all dirty nodes in height order — the one propagation entry point
```

Key methods:

- **`stabilize()`** — drains dirty nodes height by height. `O(dirty nodes)`;
  diamonds resolve to one evaluation per node. Re-entrant `set()`s during effects
  land back in the queue (same pass if still ahead in height, next stabilize
  otherwise). `stabilize()` never recurses into itself.
- **`register_callback(id, f)`** — register (or replace) the handler dispatched
  by `invoke_callback`. The `html!` two-way-binding codegen calls this with the
  id it read from `SignalSetter::callback_id`.
- **`invoke_callback(id, data)` / `invoke_callback_json(id, json)`** — the JS
  return leg: deliver an event to a callback. Returns `false` for stale ids
  (disposed signals) — expected during teardown races, silently dropped. The
  callback usually writes a setter; call `stabilize()` afterwards to flush.
- **`untracked(f)`** — run with dependency tracking suspended (also legalises
  nested tracked evaluations — how `<Show>`/`<For>` mount reactive content from
  inside a watcher effect).
- **`add_notification_manager(m)`** — register a post-stabilize listener
  (`NotificationManager`); they fire after every stabilize, in registration order
  (this is how `foundation_wasm_ui` flushes `DomOp`s after each flush).

Interop callback ids and instance ids are **monotonic and never reused**; ids
0–15 are reserved for ambient nodes the JS registry seeds.

### How it works

All mutable graph state lives behind one `RefCell` (single-threaded, G15). The
borrow discipline is the heart of it: user closures (effects, computeds,
callbacks) always run with the graph borrow **released** — they re-enter through
getters/setters, which take their own short borrows. `stabilize()` pops a node,
clones its `Rc` eval closure, drops the borrow, runs it, re-borrows to commit the
dependency diff and mark observers.

---

## `SignalHub` — cross-thread signals (native)

On native targets (not wasm32), the optional `hub` module lets worker threads
write, read, and subscribe to signals **without locking the graph**. The graph
stays single-threaded; the hub is an actor seam that extends glitch-freedom
across threads — remote readers only ever observe post-`stabilize` snapshots.

- **`SignalHub`** — owner side; lives on the runtime thread. Holds a command
  queue, an exposure registry, and publisher effects. **Not `Send`.**
- **`HubHandle` / `RemoteSetter` / `RemoteGetter` / `SignalStream`** — the
  `Send + Sync + Clone` surface workers use. Setters never cross threads — only
  `RemoteId`s and `Send` values do.
- **`PumpReport`** — what one `pump()` applied/stabilized; **`HubGone`** — the
  hub's runtime shut down.
- **`HubDriver`** (with the `valtron` feature) — a `TaskIterator` that pumps the
  hub under valtron's multi pool.

Commands flow in through one MPSC queue; `pump()` applies the batch, runs one
`stabilize()`, and publisher effects copy each post-stabilize value into a watch
cell and fan out to subscriber channels.

---

## What this crate is NOT for

- **Not the DOM.** This is the reactive graph only — no HTML, no `DomOp`, no
  rendering. DOM wiring lives in `foundation_wasm_ui` (its bindings *are* effects
  from this crate). The `IntoHtml` impls for `SignalGetter`/`ComputedGetter` are
  the only nod to UI, and they just read `.get()`.
- **Not multi-threaded by default.** The core graph is single-threaded by design
  (`Rc`/`RefCell`, G15 — WASM is single-threaded). Cross-thread access goes
  through `SignalHub`, not `Send` signals.
- **Not an inline-propagation engine.** `set()` does not recompute — propagation
  is deferred to `stabilize()`. If you forget to stabilize, nothing updates.
- **Not a singleton.** There is no ambient runtime; you pass `Rc<Runtime>` /
  `Context` explicitly.
