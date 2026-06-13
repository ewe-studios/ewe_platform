# foundation_ui_components

Headless, composable UI components over `foundation_wasm_ui` + `foundation_signals`.
"Headless" = we ship **behavior + a data-attribute contract**; you ship the CSS.
`no_std`-compatible. Components are plain functions + config/slot structs — no
component trait (composition is function composition).

> Catalog spec: `specifications/42-ui-component/features/05-headless-components/`.

## Contents
- [The component shape](#the-component-shape)
- [Config & the `[expr]` vs `{expr}` rule](#config--the-expr-vs-expr-rule)
- [The data-attribute styling contract](#the-data-attribute-styling-contract)
- [Events: how a click/focus reaches Rust](#events-how-a-clickfocus-reaches-rust)
- [F1 primitives](#f1-primitives) — button, toggle, toggle-group, separator, avatar
- [M6 field state](#m6-field-state) — `field` + `FieldBinding` callbacks

---

## The component shape

Every component is a function returning `Html`. Static config is a struct;
genuinely dynamic state flows in/out via signals; replaceable content is a slot.

```rust
use foundation_ui_components::{toggle, ToggleConfig, ToggleSlots};

let (pressed, set_pressed) = ctx.signal(false);
let html = toggle(&ctx, &rcv, ToggleConfig::default(), &pressed, set_pressed, ToggleSlots::default());
```

Pure components (no reactivity) take no `ctx`/receiver — e.g. `separator`.

## Config & the `[expr]` vs `{expr}` rule

Config text fields are `Cow<'static, str>` — string literals cost nothing,
owned `String`s are accepted (`class: Some("card".into())`). Inside a
component's `html!`:

- **`attr=[expr]`** — STATIC, evaluated once (config values: `class=[class]`).
- **`attr={expr}`** — REACTIVE, re-runs on signal change (`aria-pressed={p.get()}`).
- **`attr={cond.then_some("")}`** — PRESENCE (set when on, removed when off) —
  the data-attribute contract below.

### ⚠️ Embedding child `Html`: `<Fragment>{expr}</Fragment>`, not `{expr}`

In the **reactive** `html!` form, a `{expr}` *child* slot is **TEXT-ONLY** — the
generated effect does `into_html(expr).text` → `SetText`. An element/`Html`/
`Vec<Html>` value placed in a bare `{expr}` child is silently **text-ified to
empty** (tag + children dropped); attribute-only tests won't catch it. To mount
a rendered `Html` element (slot output, a built child, a machinery `<script>`),
wrap it:

```rust
html! { ctx, rcv,
    <button class=[class]>
        <Fragment>{children}</Fragment>   // ✅ mounts the element(s)
        // {children}                     // ❌ would render nothing
    </button>
}
```

`<Fragment>` expands the value once at mount (`mount_fragment`/`build_subtree`,
emitting CreateElement + attributes + children). Bare `{signal.get()}` is still
correct for genuine TEXT. Rule: **text → `{…}`, element content → `<Fragment>{…}</Fragment>`.**

## The data-attribute styling contract

Components NEVER toggle style classes. They toggle documented **data-attributes**;
your CSS targets them:

```css
.toggle[data-pressed] { background: var(--color-primary); }
.field-control[data-invalid] { border-color: red; }
.avatar[data-loading-status="loaded"] .avatar-fallback { display: none; }
```

Presence attributes are set when true and *removed* when false (so
`[data-pressed]` matches only when actually pressed). Each component's section
lists its attributes.

## Events: how a click/focus reaches Rust

Two paths (both detailed in the `foundation_wasm_ui` README):
- **`SignalSetter`** — two-way binding for value-carrying events: a text input's
  `primal:onchange={set_value}` delivers `value`; a checkbox delivers `checked`.
- **`ctx.callback(closure)`** — the escape hatch for events that carry *no*
  value (button click, image `load`/`error`, focus/blur). Returns a `Callback`
  you attach as `primal:onclick={cb}`. Components use this to, e.g., flip a
  toggle: `ctx.callback(move |_| set_pressed.set(!pressed.get()))`.

---

## F1 primitives

### `button` / `button_with_click`
`<button type="button">` with content + optional `loading` signal
(→ `data-loading` + `aria-busy`). `button_with_click` adds a `Callback`.
`focusable_when_disabled` keeps it in tab order via `aria-disabled`.
Data attrs: `data-disabled`, `data-loading`.

```rust
let go = ctx.callback(move |_| { /* … */ });
button_with_click(&ctx, &rcv, ButtonConfig::default(), ButtonSlots::single("Save"), None, go);
```

### `toggle`
A two-state `<button aria-pressed>`; clicking flips the `pressed` signal (no
`role` — native button + `aria-pressed` IS the contract). Data attr: `data-pressed`.

### `toggle_group`
`<div role="group">` over toggles sharing a `Vec<String>` selection (single or
`multiple`). Items render via `<For>` (correct ids + **live** `data-pressed`);
clicks update the Vec. Carries `data-composite` so the M5 roving-focus module
can attach. Data attrs: `data-orientation`, `data-disabled`, `data-multiple`.

### `separator`
Pure `<div role="separator">` with `aria-orientation`/`data-orientation`;
`aria-hidden` present only when decorative. No `ctx` needed.

### `avatar`
Image with fallback. `load`/`error` events (via `ctx.callback`) drive an
internal status signal; the fallback hides once `loaded` (`data-hidden`).
`fallback_delay_ms` → the `--avatar-fallback-delay` CSS var (CSS owns the
timing). Data attr: `data-loading-status` (idle/loading/loaded/error).

---

## M6 field state

The unified form-control lifecycle: **touched / dirty / filled / focused /
valid / errors**, reflected as data-attributes for CSS and exposed as signals.

The field **owns** the six signals and hands the control a `FieldBinding` of
ready-made `Callback`s to spread onto its own element — so any control works:

```rust
use foundation_ui_components::{field, FieldConfig, FieldSlots, ValidationMode};

let (value, set_value) = ctx.signal(String::new());
let (markup, state) = field(
    &ctx, &rcv,
    FieldConfig { validation_mode: ValidationMode::OnChange,
                  validator: Some(|v: &str| if v.is_empty() { vec!["required".into()] } else { vec![] }),
                  ..FieldConfig::default() },
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

The binding wires:
- `on_focus` → `focused = true`
- `on_blur`  → `touched = true`, `focused = false` (+ validate on `OnBlur`)
- `on_input` → `dirty`/`filled` from the value (+ validate on `OnChange`)

The field root reflects `data-touched/dirty/filled/focused/valid/invalid` (+
static `data-disabled`/`data-required`), and the label's `for` matches
`b.control_id`. Style validity with `.field[data-invalid] .field-error { … }`.

> Why "field hands a binding" rather than field-aware controls: it works with
> any markup the caller writes (cost: wire all the handlers). See spec-42 §M6.

---

## Machinery (`machinery::*`) — JS behaviors as scoped scripts

Some behavior is genuinely the browser's job. Rather than a monolithic runtime,
each behavior's JS is a `function(scope){…}` colocated beside its Rust contract
and delivered as a `<script scoped primal:script>` node the component embeds in
its root (`machinery::scoped_script`). The existing scoped-script hydrator runs
it once with `scope` (`scope.parent()` = the component root, `scope.addEvent`
auto-cleans on disconnect), then removes it — idempotent across morph/re-insert,
no `register_function`, no `cfg(wasm32)` gating. Options ride the component's
own `data-*` attributes; outputs WRITE the documented `data-*`/CSS-var contract
so CSS + morph see the same thing regardless of delivery. Embed with
`<Fragment>{behavior()}</Fragment>` (it's element content, §⚠️ above).

A recurring pattern: when JS must change Rust state, it `.click()`s a hidden
element whose `primal:onclick` the component already wired (e.g. `set_open(false)`)
— the same mechanism roving-select uses. No JS→signal bridge needed.

| Behavior | Builder | Container contract | Writes |
|----------|---------|--------------------|--------|
| **M5 composite** (roving focus) | `composite::composite_behavior()` | `data-composite` + `data-orientation`/`data-loop`; items `data-composite-item`; optional `data-composite-select` (move *and* select) / per-item `data-composite-active` | roving `tabindex`; focuses/clicks members on arrows/Home/End |
| **M3 dismiss** (light dismiss) | `dismiss::dismiss_behavior()` | `data-dismiss` + optional `primal:anchor` (ignore trigger) / `data-dismiss-escape\|outside="false"`; a hidden `[data-dismiss-action]` wired to `set_open(false)` | clicks the action on Escape / outside-pointer, stamping `data-dismiss-reason` |
| **M1 position** (anchored) | `position::position_behavior()` | `primal:anchor` (or prev sibling) + `primal:side`/`primal:align`/`data-offset`/`data-collision-side\|align` | `data-side`/`data-align`; `--anchor-*`/`--available-*`/`--positioner-*`/`--popup-*`/`--transform-origin`; re-runs on scroll/resize |
| **M7 transition** (enter/leave) | `transition::transition_behavior()` | open signal toggles `data-open`/`data-closed`; optional `[data-transition-complete]` + `data-instant` | `data-starting-style`/`data-ending-style`; fires complete on `transitionend` |

`toggle_group` (M5) and `radio_group` (M5 move-and-select) embed theirs today;
the M3/M1/M7 behaviors are ready for the F4 overlay family. Follow-ups noted in
the module docs: M5 typeahead, M3 hover-intent + safe polygon, M1
`fallbackAxisSide`/`sticky`/arrow centering.
