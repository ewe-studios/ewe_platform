# F2 — Selection controls: switch, checkbox, checkbox-group, radio, radio-group

> **Implementation status (2026-06-13):** `switch`, `checkbox` (tri-state
> `CheckState`), `checkbox_group` (+ `parent_check_state` computed showcase),
> and `radio_group` are implemented and tested in
> `backends/foundation_ui_components`. The hidden-native-input pattern uses the
> existing two-way binding: a checkbox/switch `change` delivers `checked`
> straight to a `SignalSetter<bool>`; tri-state checkbox and radio selection
> use `ctx.callback`. Data-attribute + ARIA contracts (presence pairs,
> `aria-checked="mixed"`) verified on the op stream.
>
> **Deferred (documented):** (1) M5 arrow-key roving — groups emit the
> `data-composite` container contract; the keyboard "move (+select for radio)"
> behavior is the M5 JS module's job (feature 05 machinery, not yet built).
> (2) the hidden input's `.indeterminate` DOM property (needs a `SetProperty`
> path; `aria-checked="mixed"` + `data-indeterminate` already carry the
> contract). (3) full F7 `field` integration of F2 controls (emitting the six
> field data-attrs from a passed `FieldState`). (4) standalone `radio()` for
> custom group layouts (today radios are built by `radio_group` items).

Source: base-ui `types.md` references (complete prop tables harvested
2026-06-13). Shared spine for the whole family:

- **Hidden native input**: the styled part is a `<span>` (or `<button>`),
  with a visually-hidden real `<input>` beside it carrying
  `name`/`value`/validity — forms and AT see a native control. We adopt
  this 1:1 (the input is part of the component's template; `M6` wires it).
- **Field integration**: when inside `field` (F7), all controls emit
  `data-valid/invalid/touched/dirty/filled/focused` and inherit
  `disabled`/`name` from the field. In ours that's the `FieldState` signal
  bundle passed as an optional config member.
- **Controlled triple collapses**: every `x`/`defaultX`/`onXChange` becomes
  one signal pair.
- **`readOnly` vs `disabled`** (captured behavior): readOnly stays
  focusable + announced, only mutation is blocked; disabled leaves tab
  order (unless `focusableWhenDisabled` style override in F1 spirit).

---

## switch

Parts: Root (span + hidden input), Thumb (span). `role="switch"`,
`aria-checked`.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `checked`/`defaultChecked`/`onCheckedChange` | `(checked, set_checked)` | signal |
| `name` | `config.name` | static |
| `value: string` (submitted when on; default "on") | `config.value` | static |
| `uncheckedValue: string` (submitted when off; default: nothing — native parity) | `config.unchecked_value: Option` | static |
| `form: string` (owning form when rendered outside it) | `config.form: Option` | static |
| `required`/`readOnly`/`disabled` (all false) | config | static |
| `nativeButton` (render as `<button>` for sibling-label pattern) | `config.as_button` | static — span default supports ENCLOSING labels (`<label><switch/>text</label>`); button form pairs with `for`/`id` labels and must hoist the hidden input OUTSIDE the label (captured base-ui subtlety) |
| `id` | `config.id: Option` | static — goes on the hidden input (or root when button) for label wiring |

Data attributes: `data-checked` / `data-unchecked` (presence pair — CSS
selects both directions without `:not()`), `data-disabled`,
`data-readonly`, `data-required` + the field six.

Keyboard: Space toggles (Enter too when button form). Click anywhere on
root toggles. Toggling writes the signal; everything else (aria-checked,
data-pair, hidden-input checked, field dirty/filled) is effects.

**Platform notes:** `<input type=checkbox switch>` exists only in Safari —
not adoptable. `accent-color` styles native checkboxes but can't do thumb
geometry; headless span+thumb stays justified.

---

## checkbox

Parts: Root (span + hidden input), Indicator (span, visible when checked).

Everything switch has, plus:

| prop | ours | notes |
|------|------|-------|
| `indeterminate: bool = false` | `checked` signal becomes 3-state: `CheckState::{Checked, Unchecked, Indeterminate}` | `aria-checked="mixed"`, `data-indeterminate`, hidden input `.indeterminate` property (SetProperty op) |
| `parent: bool = false` | `config.parent` | the group's select-all checkbox (below) |
| `value: string` | `config.value` | identity within a group |

Indicator behavior: rendered always, visibility via `data-checked`-driven
CSS (or `data-hidden`); carries `data-starting-style`/`data-ending-style`
transition attrs (M7) for tick animations.

Keyboard: Space toggles; indeterminate → checked on first activation.

**Platform notes:** `:indeterminate` pseudo-class works on the hidden
native input — CSS can target `input:indeterminate + .indicator` as a
no-JS styling path; we still emit `data-indeterminate` for uniformity.

---

## checkbox-group

Single part (div, `role="group"`). The interesting machinery is the
PARENT checkbox.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `value: string[]` (+triple) | `(values, set_values)` (`Vec<String>`) | signal |
| `allValues: string[]` | `config.all_values` | static — required to compute the parent's tri-state |
| `disabled` | static | |

Captured behaviors:
- Child checkbox with value `v`: checked ⇔ `values.contains(v)`; its
  setter inserts/removes in the Vec signal.
- PARENT checkbox (`parent: true`): state is a `ctx.computed` —
  `Unchecked` when none, `Checked` when `values == all_values`,
  `Indeterminate` otherwise. Activating it writes all-or-none. This is the
  spec's showcase that computeds replace React's derived-state plumbing.
- Form submission: each child posts `name=value` per native semantics via
  its hidden input.

---

## radio + radio-group

radio parts: Root (span + hidden input), Indicator. group: one part
(div, `role="radiogroup"`).

| base-ui prop (group) | ours | static/signal |
|----------------------|------|---------------|
| `value` (+triple) | `(value, set_value)` (`Option<String>`) | signal |
| `name`, `form` | config | static — ONE hidden input strategy per GROUP (base-ui renders per-radio inputs; either satisfies forms — decide at impl, document) |
| `required`/`readOnly`/`disabled` | config | static |

radio (item): `value` (required, static), `disabled`/`readOnly`/`required`
per-item static overrides.

Keyboard (the family's M5 flagship — captured exactly):
- Arrows move focus AND select (WAI-ARIA radio pattern — unlike
  toggle-group where arrows only move focus).
- Orientation-aware (↑↓ + ←→ both work in base-ui's group), loops.
- Tab enters the group on the CHECKED radio (or first if none) — single
  tab stop (roving tabindex).
- Space selects the focused, unchecked radio.

Data attributes: radio gets the same checked/unchecked/disabled/readonly/
required + field six; group gets `data-disabled`.

**Platform notes:** native `<input type=radio>` + `accent-color` remains
the zero-JS fallback for plain forms; the headless version exists for
custom indicator geometry. Worth one line in component docs steering
people to native when they don't need custom visuals.

---

## Shapes

```rust
pub enum CheckState { Checked, Unchecked, Indeterminate }

pub fn switch(ctx, rcv, cfg: SwitchConfig, checked, set_checked) -> Html;
pub fn checkbox(ctx, rcv, cfg: CheckboxConfig, state: SignalGetter<CheckState>, set: SignalSetter<CheckState>, slots{indicator}) -> Html;
pub fn checkbox_group(ctx, rcv, cfg: CheckboxGroupConfig, values, set_values, children: Vec<Slot>) -> Html;
pub fn radio_group(ctx, rcv, cfg: RadioGroupConfig, value, set_value, children: Vec<Slot>) -> Html;
pub fn radio(cfg: RadioConfig, slots{indicator}) -> Html;   // wires into enclosing group via config
```

Machinery: M5 (radio-group roving+selecting variant), M6 (all), M7
(indicators). Tests: data-pair flips on the op stream, hidden-input
name/value/checked ops, parent tri-state computed, radio keyboard contract,
enclosing vs sibling label forms, morph survival of checked state.

## Reference CSS (vendored — the styling acceptance criteria)

[switch](../styling/switch.css) · [checkbox](../styling/checkbox.css) · [checkbox-group](../styling/checkbox-group.css) · [radio](../styling/radio.css)

Per [styling/README.md](../styling/README.md): each implementation must
satisfy its reference stylesheet's selectors (data-attributes, CSS vars)
with only the mechanical adaptations listed there; the adapted file becomes
the component's opt-in default stylesheet.

