# foundation_ui_components

Headless, composable UI components for [`foundation_wasm_ui`](../foundation_wasm_ui/README.md).
**"Headless" = we ship behavior + an accessibility & data-attribute contract; you
ship the CSS.** Components are plain functions + config/slot structs — no component
trait (composition is function composition). `no_std`-compatible.

```rust
use foundation_ui_components::{toggle, ToggleConfig, ToggleSlots};

let (pressed, set_pressed) = ctx.signal(false);
let html = toggle(&ctx, &rcv, ToggleConfig::default(), &pressed, set_pressed, ToggleSlots::default());
```

```css
/* you own the look — target the documented data-attributes */
.toggle[data-pressed] { background: var(--color-primary); }
```

---

## Start here

**▶ [Getting started — zero to 100](./docs/getting-started.md)** — drop in a
component, style it, wire events, build a validated form, reach for overlays.

## The catalog at a glance

Eight families ship (spec-42 feature 05):

| Family | Components |
|--------|-----------|
| **F1 primitives** | `button` · `toggle` · `toggle_group` · `separator` · `avatar` |
| **F2 selection** | `switch` · `checkbox` (+ `checkbox_group`) · `radio_group` |
| **F3 disclosure** | `collapsible` · `accordion` · `tabs` |
| **F4 overlays** | `dialog` · `alert_dialog` · `drawer` · `popover` · `tooltip` · `preview_card` · `toast` |
| **F5 menus** | `menu` · `context_menu` · `menubar` · `navigation_menu` · `toolbar` |
| **F6 pickers** | `select` · `combobox` · `autocomplete` |
| **F7 form** | `field` · `input` · `fieldset` · `form` · `number_field` · `otp_field` |
| **F8 surfaces** | `progress` · `meter` · `slider` · `scroll_area` · `skeleton` |

## Documentation

| Guide | What it covers |
|-------|----------------|
| [Getting started](./docs/getting-started.md) | The zero-to-100 tutorial |
| [Conventions](./docs/conventions.md) | Component shape, config, `[expr]`/`{expr}`, data-attribute contract, events |
| [Catalog](./docs/catalog.md) | Every component, with F1 detailed |
| [Forms & fields](./docs/forms-and-fields.md) | `field` lifecycle + `FieldBinding` |
| [Machinery](./docs/machinery.md) | Positioning, dismiss, focus-trap, transitions, roving focus |

New to the underlying reactive model (signals, the `html!` macro, events)? Read the
**[`foundation_wasm_ui` getting started](../foundation_wasm_ui/docs/getting-started.md)**
first.

## Install

```toml
[dependencies]
foundation_ui_components = { workspace = true }
```

---

Catalog spec + build notes:
[`specifications/42-ui-component/`](../../specifications/42-ui-component/).
