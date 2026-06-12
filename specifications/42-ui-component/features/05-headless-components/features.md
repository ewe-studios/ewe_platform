# Feature 05: Headless Components — the catalog (base-ui review, built our way)

Merged from the initial headless-ui draft (kept as
[initial-headlessui-draft.md](initial-headlessui-draft.md)) and the full
base-ui source review (2026-06-13,
`@formulas/src.UIFrameworks/src.baseui/base-ui`). Depends on feature 00
(slots), 03 (App), 04 (mount negotiation); the pickers/menus families also
need feature 01 (`<Show>`/`<For>`).

## 1. What the base-ui review found (and what we adopt vs. reject)

base-ui's anatomy: every component is a tree of small PARTS
(`Root/Trigger/Popup/Positioner/Arrow/…`), state flows through React context,
each part renders a real element carrying **state data-attributes**
(`data-checked`, `data-disabled`, `data-invalid`, …) that CSS targets, and
form controls render a visually-hidden native `<input>` for form/AT
integration.

**Adopt (it fits us better than it fits React):**

- **Parts as the unit of composition.** Ours are FUNCTIONS (`switch_root`,
  `switch_thumb`), composed with feature-00 slot structs — explicit
  arguments where base-ui needs context providers.
- **The data-attribute styling contract, wholesale.** State is expressed as
  presence-attributes on the part's element (`data-checked` when on,
  `data-unchecked` when off, `data-disabled`, `data-readonly`,
  `data-required`, and the field set `data-valid/invalid/touched/dirty/
  filled/focused` — harvested from `stateAttributesMapping`). For us each
  one is a tiny effect emitting `SetAttribute`/`RemoveAttribute`; it's
  morph-safe (attributes ride elements, which F07 preserves by id) and
  CSS-native (no class soup). This is THE styling interface of the catalog —
  headless means "we ship behavior + data-attributes, you ship CSS".
- **Hidden-native-input pattern** for switch/checkbox/radio/slider/select:
  the styled part is a `span`/`button`, a visually-hidden real `<input>`
  beside it carries name/value/validity for forms and AT.
- **The field state machine** (valid/touched/dirty/filled/focused +
  validation modes) as the one shared form story (§3.M6).
- **WAI-ARIA contracts per component** (the old draft's table was right;
  extended per component below).

**Reject (React solutions to React problems):**

- React context for parts wiring → explicit slot structs + arguments
  ("it's all just functions").
- `useRender`/render-props polymorphism → not needed; a part function
  returns `Html`, callers compose.
- Controlled/uncontrolled prop dance (`checked` + `defaultChecked` +
  `onCheckedChange`) → a signal IS the controlled state; one
  `SignalGetter<T>`/`SignalSetter<T>` pair replaces all three props.
- floating-ui's JS portal/layer system → prefer the PLATFORM: native
  `<dialog>`/`showModal` and the `popover` attribute give top-layer,
  light-dismiss, and focus containment for free (§3.M2); a small positioner
  module covers anchored placement (§3.M1).
- CSP/direction providers → `dir` is static config where it matters.

**The plan.md reminder applied:** for every component below, the spec marks
state as **static** (set once: variant, size, required, name, orientation,
labels) vs **signal** (genuinely dynamic: checked, open, value, pressed,
active index, validity). Don't go signal mad — `disabled` defaults to
static; a component that needs it dynamic takes a getter explicitly.

## 2. The component shape (the pattern every entry follows)

```rust
/// Static config — plain values, no signals.
pub struct SwitchConfig {
    pub name: &'static str,
    pub required: bool,
    pub disabled: bool,
    // …
}

/// fn parts: root composes thumb; state in/out via signals.
pub fn switch(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: SwitchConfig,
    checked: SignalGetter<bool>, set_checked: SignalSetter<bool>,
) -> Html {
    html! { ctx, rcv,
        <button class="switch" role="switch" primal:onclick={set_checked /* toggled via EventData */}>
            <span class="switch-thumb"></span>
        </button>
    }
    // + effects: aria-checked, data-checked/data-unchecked
    // + hidden input carrying name/value (form integration)
}
```

Slot-bearing components add a `XxxSlots` struct (feature 00); parts that
callers may replace are slots, parts that are structural are built-in.

## 3. Shared machinery (build FIRST — every family leans on these)

| id | machinery | what / how (ours) |
|----|-----------|--------------------|
| M1 | **Anchored positioning** | JS module in the runtime: position popup relative to anchor (side/align/offset/collision-flip), exposed as `data-side`/`data-align` for CSS arrows. Replaces floating-ui. Components request it via a `primal:position` attribute contract; observer keeps it fresh on scroll/resize. |
| M2 | **Layering** | Native first: `<dialog>.showModal()` for modal (focus containment + top-layer + ::backdrop free), `popover` attribute for non-modal top-layer with light-dismiss. JS runtime gets `DomOp`-reachable helpers (open/close via SetProperty). No portal system. |
| M3 | **Dismiss** | One JS dismiss manager: Escape, outside-click, focus-out, anchor-gone → fires the component's close callback (wired as a `primal:on*`-style callback id). Layered (innermost dismisses first). |
| M4 | **Focus** | Trap (modal dialogs — mostly free via `showModal`), initial-focus targeting, RESTORE to trigger on close. |
| M5 | **Roving focus / composite** | Arrow-key navigation over a part set (menus, tabs, radio-group, toolbar, listbox): one `tabindex=0` member, arrows move it (orientation-aware, Home/End, optional typeahead). JS module keyed off a `data-composite` container attribute. |
| M6 | **Field state** | Rust-side: `FieldState` signals (valid/touched/dirty/filled/focused) + validation hooks; every form control emits the field data-attributes and wires the hidden native input. |
| M7 | **Transitions** | From the old draft, kept: CSS-class choreography (`enter-from/to`, `leave-from/to`) + `transitionend`; `data-starting-style`/`data-ending-style` attributes (base-ui's convention) so pure CSS transitions work on mount/unmount. |

M1/M3/M5/M7 are JS-runtime modules with Rust-side attribute contracts;
M2 is mostly platform; M4 partially platform; M6 is Rust.

## 4. The catalog (all base-ui components, grouped by family)

Status per component starts UNSPEC'D; each family becomes its own
implementation feature (one dir per family) when work starts, with this
section as its requirements seed.

### F1 — Primitives (no machinery)
| component | parts (base-ui → ours) | state | notes |
|-----------|------------------------|-------|-------|
| **button** | Button → `button(config, slots)` | static (variant/disabled); `loading` optional signal → `aria-busy`, `data-loading` | keyboard free (native button) |
| **separator** | Separator → `separator(orientation)` | all static | `role="separator"`, `aria-orientation` |
| **avatar** | Root/Image/Fallback → `avatar(config, slots{fallback})` | `loaded` internal signal (image load/error event flips to fallback) | |
| **toggle** | Toggle → `toggle(config, pressed, set_pressed)` | `pressed` signal | `aria-pressed`, `data-pressed` |
| **toggle-group** | ToggleGroup → `toggle_group(config, value, set_value, children)` | `value` signal (single/multi) | M5 roving focus |

### F2 — Selection controls (M6 field state, hidden input)
| component | parts | state |
|-----------|-------|-------|
| **switch** | Root/Thumb | `checked` signal; name/required/readonly static; `data-checked/unchecked` + field attrs; `role="switch"` |
| **checkbox** | Root/Indicator | `checked` signal incl. indeterminate (`data-indeterminate`, `aria-checked="mixed"`) |
| **checkbox-group** | Group (+parent checkbox) | `values` signal; parent tri-state COMPUTED from children (a `ctx.computed` — exactly what computeds are for) |
| **radio** | Root/Indicator | within-group only |
| **radio-group** | Group | `value` signal; M5 roving focus (arrows move AND select); one hidden input for the group |

### F3 — Disclosure (M7 transitions)
| component | parts | state |
|-----------|-------|-------|
| **collapsible** | Root/Trigger/Panel | `open` signal; `data-open/closed` on root+panel, `aria-expanded` on trigger; panel height via `--collapsible-panel-height` custom property (base-ui trick, adopt) |
| **accordion** | Root/Item/Header/Trigger/Panel | `open_items` signal (single/multiple static mode); M5 for header arrows; items = `Vec` children of item parts |
| **tabs** | Root/List/Tab/Indicator/Panel | `selected` signal; activation mode static (auto/manual); M5 roving; `data-selected`, `aria-selected`, panels `data-hidden` (visibility toggling — no structural reactivity needed) |

### F4 — Overlays (M1-M4, M7)
| component | parts | state |
|-----------|-------|-------|
| **dialog** | Root/Trigger/Portal/Backdrop/Popup/Title/Description/Close/Viewport → native `<dialog>` + slots{title, description, children} | `open` signal; modal static flag; M4 restore; `aria-labelledby/describedby` wired to title/description slot ids |
| **alert-dialog** | thin variant: `role="alertdialog"`, no light dismiss, initial focus on least-destructive (config) |
| **drawer** | dialog + swipe-area/indent parts | `open` signal + M7; swipe = JS gesture module (DEFER swipe to its own iteration; render side-anchored dialog first) |
| **popover** | Root/Trigger/Positioner/Popup/Arrow/… → `popover` attribute + M1 | `open` signal; side/align static; `data-side/align` for arrow CSS |
| **tooltip** | + Provider(delay grouping) | `open` signal driven by hover/focus JS (delay/grouping in M3-adjacent JS); NEVER focusable content |
| **preview-card** | popover variant with hover intent | |
| **toast** | Provider/Viewport/Root/Title/Description/Action/Close + manager | toast LIST → needs feature 01 `<For>`; manager = Rust struct over signals (create/dismiss/timeout/swipe later); viewport `popover` non-modal |

### F5 — Menus (M1, M3, M5)
| component | parts | state |
|-----------|-------|-------|
| **menu** | Root/Trigger/Positioner/Popup/Item/Group/GroupLabel/CheckboxItem/RadioGroup/RadioItem/Separator/SubmenuRoot… | `open` signal; items are CHILDREN slots (static set; dynamic item sets wait for `<For>`); M5 typeahead; checkbox/radio items reuse F2 state shapes |
| **context-menu** | menu opened at pointer coords (M1 point-anchor mode) | |
| **menubar** | horizontal M5 composite of menu roots | |
| **navigation-menu** | menu family + viewport-swap animation; hover intent | DEFER detail to family feature |
| **toolbar** | Root/Button/Group/Input/Link/Separator — pure M5 composite | no open state |

### F6 — Pickers (F5 machinery + M6 + feature 01 `<For>`)
| component | parts | state |
|-----------|-------|-------|
| **select** | Root/Trigger/Value/Icon/Portal/Positioner/Popup/List/Item/ItemText/ItemIndicator/Group/… | `value`+`open` signals; hidden input; `aria-activedescendant` pattern; options via `<For>` when dynamic |
| **combobox** | + Input/Chips/Chip/ChipRemove/Clear/Status/Empty/Row/Collection | `query`+`value`+`open`+`filtered` (computed!) signals; filtering = `ctx.computed` over query — client-side; server-side via mount-stream (feature 04) |
| **autocomplete** | combobox variant (input-anchored list) | |

### F7 — Form (M6)
| component | parts | state |
|-----------|-------|-------|
| **field** | Root/Label/Control/Description/Error/Validity | the M6 carrier: label wiring (auto ids), error shows on `Show`-style visibility (data-attr until feature 01), validation modes static |
| **fieldset** | Root/Legend | static |
| **form** | Form | submit interception via callback, aggregate validity computed, errors map signal |
| **input** | Input | `value` signal via `primal:onchange` setter (G21 two-way path is ALREADY built for exactly this) |
| **number-field** | Root/Group/Decrement/Input/Increment/ScrubArea(+Cursor) | `value` signal; min/max/step static; scrub = JS pointer module, DEFER with drawer-swipe |
| **otp-field** | Root/Input×n | `value` signal; per-cell focus choreography (M5 variant) |

### F8 — Indicators & surfaces
| component | parts | state |
|-----------|-------|-------|
| **progress** | Root/Track/Indicator/Label/Value | `value` signal → width style + `aria-valuenow` + `data-complete`; indeterminate static mode |
| **meter** | same shape, `role="meter"` | |
| **slider** | Root/Control/Track/Indicator/Thumb/Value/Label | `value` signal (or values for range); drag = JS pointer module shared with scrub; keyboard arrows native-ish via hidden range input |
| **scroll-area** | Root/Viewport/Content/Scrollbar/Thumb/Corner | pure JS-side presentation (scroll math), `data-scrolling/hovering`; Rust side is just markup — candidate for a PURE component (no signals at all) |
| **skeleton** *(ours, kept from old draft)* | `skeleton(shape)` | fully static |

## 5. Implementation order

1. **Machinery**: M6 → M5 → M3 → M1 → M2 → M7 (M4 rides M2), each with JS
   tests + a proving component (M6→switch, M5→tabs, M1/M3→popover).
2. Families: F1+F2 (forms foundation) → F3 → F4 → F7 → F5 → F8 → F6 (needs
   feature 01 `<For>` — schedule feature 01 before F6 at the latest).
3. Each family gets its own feature dir (`05a-…` style or sequential
   numbers) with per-component acceptance tests: ARIA table compliance,
   keyboard contract, data-attribute contract (asserted on the op stream),
   morph survival, and a styled example proving the headless claim.

## 6. Crate

`backends/foundation_ui_components` (name from the old draft, kept):
`foundation_wasm_ui` + `foundation_signals` deps only; no_std-compatible
like its dependencies; components are functions + config/slot structs —
NO component trait (settled, feature 00 §2).
