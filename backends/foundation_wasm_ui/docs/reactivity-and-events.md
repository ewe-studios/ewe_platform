# Reactivity & events

State is signals; updates are fine-grained; events flow back through one syntax.

## Signals

A signal is a `(getter, setter)` pair from `ctx.signal(initial)`. Reading the
getter with `.get()` inside a reactive slot subscribes that slot; writing the
setter re-runs **only** the subscribed slots.

```rust
let (count, set_count) = ctx.signal(0i64);
let _ui = html! { ctx, receiver, <p>"Count: " {count.get()}</p> };
app.stabilize();      // "Count: 0"
set_count.set(1);
app.stabilize();      // one SetText("1")
```

Writes **coalesce**: several `set()`s before a `stabilize()` flush once,
glitch-free. The graph model lives in `foundation_signals` (re-exported via
`app.signals()`).

## Events: one syntax, two paths

A `primal:onX={handler}` attribute reaches Rust in one of two ways, resolved at
compile time by `MaybeCallback` — the macro never needs to know which it got.

**(a) `SignalSetter` — two-way binding.** For value-carrying events the setter's
default callback extracts `value`/`checked` and writes the signal:

```rust
let (name, set_name) = ctx.signal(String::new());
html! { ctx, receiver, <input value={name.get()} primal:onchange={set_name} /> };

let (on, set_on) = ctx.signal(false);
html! { ctx, receiver, <input type="checkbox" primal:onchange={set_on} /> }; // reads `checked`
```

**(b) `ctx.callback(closure)` — the escape hatch** for events with no convertible
value (button click, image load/error, Escape):

```rust
use foundation_signals::EventData;
let (pressed, set_pressed) = ctx.signal(false);
let toggle = ctx.callback(move |_e: &EventData| set_pressed.set(!pressed.get()));
html! { ctx, receiver, <button primal:onclick={toggle}>"toggle"</button> };
```

**(c) A plain `Fn(&EventData)` closure** gets the raw payload, no signal wiring:

```rust
let log = |e: &EventData| { let _ = (&e.value, &e.checked, e.key_code); };
html! { ctx, receiver, <input primal:oninput={log} /> };
```

### How the return leg works

On a value-carrying event the JS `EventDispatcher` builds an `EventData` JSON
(`type`, `primalId`, `value`, `checked`, `keyCode`, `modifiers`), writes it into a
global-arena slot, and calls the exported `invoke_signal_callback(callback_id,
allocation_id)`. The WASM side reads + frees the slot, dispatches through the
signals registry, and `stabilize()`s **synchronously** — so the DOM answer to an
input event goes out in the same call chain.

`ctx.signal` auto-registers a default conversion callback for event-friendly `T`
(`String`, `bool`, numerics); other types and valueless events use `ctx.callback`.

### Gotchas

- A bare closure handler gets `None` from `MaybeCallback` (no `primal:setter`
  stamped) — it can only write a signal if it captures setters.
- The default setter conversion only fires for value-carrying events; routing a
  button click through a `set_count` setter does nothing — use `ctx.callback`.
- `install_event_bridge` must run once at app init (the live loop does this), or
  `invoke_signal_callback` drops the event with "no event bridge installed".

## `<Show>` / `<For>` — structural reactivity

Signal-driven placement: mount/unmount on a condition, or reconcile a keyed list.
**Reactive form only.**

```rust
use foundation_wasm_ui::{html, Slot};
use foundation_signals::Context;
use foundation_wasm_ui::SharedInstructionReceiver;

let (open, _)  = ctx.signal(false);
let (items, _) = ctx.signal(vec![1i64, 2, 3]);

html! { ctx, receiver,
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

- `<Show>`'s child is exactly one `{ }` expression yielding an `impl Render`; it
  requires that child and cannot be self-closing.
- `<For>` is self-closing; `render` is `fn(&Context, &SharedInstructionReceiver,
  &T) -> Html`.

### How it works

Both own an **anchored region**: an empty id-bearing `<span>` marks the region's
END; content inserts before it. A single watcher effect reads the condition/items
(the only tracked read); mounting runs inside `untracked` so content signals never
become watcher dependencies. Instances mount under fresh child scopes — dropping a
scope disposes that instance's effects.

`mount_for` reconciles per run: removed keys unmount; if the kept keys' relative
order is unchanged (append/remove/update — the common case) it emits **zero move
ops**; otherwise an end→start `InsertBefore` pass restores order.

### Gotchas

- **Keys identify instances, not data.** A changed item with the same key does NOT
  re-render — read changing values from a signal in the item content.
- Duplicate keys are an error (logged; last wins) — keys must be unique per list.
- The order-restore pass is correct but may over-move on arbitrary reorders (an
  LIS minimal-move pass is future work).
- For conditionals/lists in a **pure** tree, use plain Rust (`Option`,
  `Vec<Html>`) around the macro.

See also: **[html! macro](./html-macro.md)** · **[composition](./composition.md)**.
