# Component conventions

The handful of rules every component in the catalog follows. Learn these once and
the whole catalog reads the same.

## The component shape

Every component is a function returning `Html`. Static config is a struct;
genuinely dynamic state flows in/out via signals; replaceable content is a slot.

```rust
use foundation_ui_components::{toggle, ToggleConfig, ToggleSlots};

let (pressed, set_pressed) = ctx.signal(false);
let html = toggle(&ctx, &rcv, ToggleConfig::default(), &pressed, set_pressed, ToggleSlots::default());
```

Reactive components take `&Context` + `&SharedInstructionReceiver`. Pure components
(e.g. `separator`) take neither.

## Config & the `[expr]` vs `{expr}` rule

Config text fields are `Cow<'static, str>` — string literals cost nothing, owned
`String`s are accepted (`class: Some("card".into())`). Inside a component's `html!`:

- **`attr=[expr]`** — STATIC, evaluated once (config values: `class=[class]`).
- **`attr={expr}`** — REACTIVE, re-runs on signal change (`aria-pressed={p.get()}`).
- **`attr={cond.then_some("")}`** — PRESENCE (set when on, removed when off) — the
  data-attribute contract below.

## ⚠️ Embedding child `Html`: `<Fragment>{expr}</Fragment>`, not `{expr}`

In the **reactive** `html!` form, a `{expr}` *child* slot is **TEXT-ONLY** — the
generated effect does `into_html(expr).text` → `SetText`. An element/`Html`/
`Vec<Html>` placed in a bare `{expr}` child is silently **text-ified to empty**
(tag + children dropped). To mount rendered `Html` (slot output, a built child, a
machinery `<script>`), wrap it:

```rust
html! { ctx, rcv,
    <button class=[class]>
        <Fragment>{children}</Fragment>   // ✅ mounts the element(s)
        // {children}                     // ❌ would render nothing
    </button>
}
```

Rule: **text → `{…}`, element content → `<Fragment>{…}</Fragment>`.**

## The data-attribute styling contract

Components NEVER toggle style classes. They toggle documented **data-attributes**;
your CSS targets them:

```css
.toggle[data-pressed] { background: var(--color-primary); }
.field-control[data-invalid] { border-color: red; }
.avatar[data-loading-status="loaded"] .avatar-fallback { display: none; }
```

Presence attributes are set when true and *removed* when false (so `[data-pressed]`
matches only when actually pressed). Each component's [catalog](./catalog.md) entry
lists its attributes.

## Events: how a click/focus reaches Rust

Two paths (both detailed in the `foundation_wasm_ui`
[reactivity & events](../../foundation_wasm_ui/docs/reactivity-and-events.md) doc):

- **`SignalSetter`** — two-way binding for value-carrying events: a text input's
  `primal:onchange={set_value}` delivers `value`; a checkbox delivers `checked`.
- **`ctx.callback(closure)`** — the escape hatch for events that carry no value
  (button click, image `load`/`error`, focus/blur). Returns a `Callback` you
  attach as `primal:onclick={cb}`. Components use this internally, e.g. flip a
  toggle: `ctx.callback(move |_| set_pressed.set(!pressed.get()))`.

## Headless = behavior + contract, you bring CSS

"Headless" means the component ships structure, ARIA roles/attributes, keyboard
behavior, and the data-attribute contract — **not** visual styling. You write the
CSS that reads those attributes. This keeps components framework-agnostic about
design and lets one behavior serve any look.

See also: **[catalog](./catalog.md)** · **[machinery](./machinery.md)** ·
**[forms & fields](./forms-and-fields.md)**.
