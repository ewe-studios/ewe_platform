# Forms & fields

The F7 family centers on `field` — the unified form-control lifecycle: **touched /
dirty / filled / focused / valid / errors**, reflected as data-attributes for CSS
and exposed as signals.

## How `field` works

The field **owns** the six signals and hands the control a `FieldBinding` of
ready-made `Callback`s to spread onto its own element — so *any* control works
(input, textarea, a custom widget), not just field-aware ones.

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
    FieldSlots::default(),       // label / description / errors slots
    |c, r, b| html! { c, r,      // ctx/rcv handed in (avoids a borrow conflict)
        <input id=[b.control_id.clone()] value={value.get()}
               primal:onchange={set_value}
               primal:onfocus={b.on_focus}
               primal:onblur={b.on_blur}
               primal:oninput={b.on_input} />
    },
);
// state.valid.get(), state.dirty.get(), … all live signals
```

## What the binding wires

- `on_focus` → `focused = true`
- `on_blur`  → `touched = true`, `focused = false` (+ validate on `OnBlur`)
- `on_input` → `dirty`/`filled` from the value (+ validate on `OnChange`)

The field root reflects `data-touched/dirty/filled/focused/valid/invalid` (+ static
`data-disabled`/`data-required`), and the label's `for` matches `b.control_id`.
Style validity with `.field[data-invalid] .field-error { … }`.

> Why "field hands a binding" rather than field-aware controls: it works with any
> markup the caller writes (cost: you wire the handlers). See spec-42 §M6.

## The rest of F7

- **`input`** — a thin styled-contract `<input>` wrapper.
- **`fieldset`** / **`form`** — grouping + submission scaffolding.
- **`number_field`** — numeric input with step/min/max + increment/decrement.
- **`otp_field`** — segmented one-time-code input.

All of these slot into a `field` for the validity/lifecycle contract, or stand
alone. See **[catalog](./catalog.md)**.
