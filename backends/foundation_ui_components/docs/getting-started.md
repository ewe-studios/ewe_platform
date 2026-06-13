# Getting started — zero to 100

`foundation_ui_components` ships **headless** components over `foundation_wasm_ui`:
behavior + an accessibility + data-attribute contract. **You ship the CSS.** Each
component is a plain function returning `Html` — no component trait.

> New to `foundation_wasm_ui` (signals, the `html!` macro, events)? Read its
> **[getting started](../../foundation_wasm_ui/docs/getting-started.md)** first —
> everything here builds on that loop (`App` → `html!` → `stabilize`).

---

## 1. Drop in your first component

Every reactive component takes `&Context` + `&SharedInstructionReceiver`, a config
struct, the state signal(s) it reflects, and a slots struct.

```rust
use foundation_ui_components::{toggle, ToggleConfig, ToggleSlots};

let (pressed, set_pressed) = ctx.signal(false);
let html = toggle(&ctx, &rcv, ToggleConfig::default(), &pressed, set_pressed, ToggleSlots::default());
```

Pure components with no reactivity take no `ctx`/receiver — e.g. `separator`.

## 2. Style it via data-attributes (not classes)

Components NEVER toggle style classes. They toggle documented **data-attributes**;
your CSS targets them. Presence attributes are *set* when true and *removed* when
false, so a selector matches only when the state actually holds:

```css
.toggle[data-pressed] { background: var(--color-primary); }
.field-control[data-invalid] { border-color: red; }
.avatar[data-loading-status="loaded"] .avatar-fallback { display: none; }
```

This is the whole styling contract — see **[conventions](./conventions.md)**.

## 3. Wire events

Two paths (both from `foundation_wasm_ui`):

- **`SignalSetter`** — two-way binding for value-carrying events (text input
  `value`, checkbox `checked`): `primal:onchange={set_value}`.
- **`ctx.callback(closure)`** — the escape hatch for valueless events (click,
  focus, image load/error). Components use it internally to, e.g., flip a toggle.

## 4. Build a real form with `field`

`field` owns the form-control lifecycle (touched / dirty / filled / focused /
valid / errors) and hands your control a `FieldBinding` of ready-made callbacks to
spread onto any markup:

```rust
use foundation_ui_components::{field, FieldConfig, FieldSlots, ValidationMode};

let (value, set_value) = ctx.signal(String::new());
let (markup, state) = field(
    &ctx, &rcv,
    FieldConfig {
        validation_mode: ValidationMode::OnChange,
        validator: Some(|v: &str| if v.is_empty() { vec!["required".into()] } else { vec![] }),
        ..FieldConfig::default()
    },
    FieldSlots::default(),                 // label / description / errors slots
    |c, r, b| html! { c, r,
        <input id=[b.control_id.clone()] value={value.get()}
               primal:onchange={set_value}
               primal:onfocus={b.on_focus}
               primal:onblur={b.on_blur}
               primal:oninput={b.on_input} />
    },
);
// state.valid.get(), state.dirty.get(), … are live signals
```

The field root reflects `data-touched/dirty/filled/focused/valid/invalid`. Deep
dive: **[forms & fields](./forms-and-fields.md)**.

## 5. Reach for overlays, menus, pickers

The catalog has eight families — dialogs, drawers, popovers, tooltips, toasts,
menus, comboboxes, sliders, and more. Overlays compose with **machinery**
behaviors (positioning, dismiss, focus trap, transitions) delivered as scoped
scripts:

```rust
use foundation_ui_components::{dialog, DialogConfig, DialogSlots};
let (open, set_open) = ctx.signal(false);
let html = dialog(&ctx, &rcv, DialogConfig::default(), &open, set_open, DialogSlots::default());
```

Full list: **[catalog](./catalog.md)**. How the JS behaviors work:
**[machinery](./machinery.md)**.

---

## Where to go next

| You want to… | Read |
|---|---|
| Understand the component shape, config, events | [conventions](./conventions.md) |
| Browse every component | [catalog](./catalog.md) |
| Build forms with validation | [forms & fields](./forms-and-fields.md) |
| Understand positioning/dismiss/focus-trap | [machinery](./machinery.md) |
