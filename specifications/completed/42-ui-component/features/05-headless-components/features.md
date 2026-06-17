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
/// Static config — plain values, no signals. Text/class fields are
/// `Cow<'static, str>` (see §8.1): `&'static str` literals cost nothing,
/// owned `String`/dynamic values are accepted too. NEVER `&'static str`
/// (rejects runtime strings) and NEVER a config lifetime (config is moved
/// into `'static` effect closures, so a borrow can't be captured).
pub struct SwitchConfig {
    pub name: Cow<'static, str>,
    pub required: bool,
    pub disabled: bool,
    pub class: Option<Cow<'static, str>>,
    // …
}

/// fn parts: root composes thumb; state in/out via signals. The whole body
/// is `html!` — reactive attrs (`aria-checked={checked.get()}`) and
/// Option-valued attrs (`data-checked={checked.get().then_some("")}`, §8.2)
/// mean NO hand-written effects and NO manual DomOp queueing.
pub fn switch(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: SwitchConfig,
    checked: SignalGetter<bool>, set_checked: SignalSetter<bool>,
) -> Html {
    html! { ctx, rcv,
        <button class="switch" role="switch"
                aria-checked={checked.get()}
                data-checked={checked.get().then_some("")}    // present iff on
                data-unchecked={(!checked.get()).then_some("")}
                primal:onclick={set_checked /* toggled via EventData */}>
            <span class="switch-thumb"></span>
        </button>
        // + a visually-hidden native <input> carrying name/value (forms/AT)
    }
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
| M7 | **Transitions** | From the old draft, kept: CSS-class choreography (`enter-from/to`, `leave-from/to`) + `transitionend`; `data-starting-style`/`data-ending-style` attributes (base-ui's convention) so pure CSS transitions work on mount/unmount. Also owns ELEMENT MEASUREMENT into CSS custom properties (`--collapsible-panel-height`, `--active-tab-*`, `--anchor-width`…) — the headless animation contract. |
| M8 | **Pointer gestures** | Shared drag/swipe/scrub module: slider thumb drag (ships WITH slider), drawer swipe + snap points, number-field scrub, toast swipe-dismiss (all deferred except slider). Pointer capture, axis locking, movement CSS vars. |

M1/M3/M5/M7 are JS-runtime modules with Rust-side attribute contracts;
M2 is mostly platform; M4 partially platform; M6 is Rust.

**Exact algorithms, constants, and defaults live in
[machinery.md](machinery.md)** (cold-review fixes: collision three-knob
semantics, typeahead 750ms prefix matching, scroll-lock technique,
hover-intent safe polygon + per-component delays, data-instant,
hold-repeat constants, the event reason taxonomy, RTL policy).
machinery.md WINS where a family doc is vaguer.

## 4. The catalog (all base-ui components, grouped by family)

**The full review lives in [families/](families/)** — every component,
every prop, every behavior and data-attribute captured from the base-ui
autogenerated references, each translated to our config/signal/slot shapes
with a PLATFORM VERDICT (the new-CSS/HTML-first policy: prefer
`<dialog>`, `popover`, CSS anchor positioning, `<details name>`,
`@starting-style`, `scrollbar-color`, `:user-invalid`, customizable
`<select>` … where Baseline allows; ship our machinery as the portable
fallback behind the SAME markup contract; document native recipes
alongside every component they can replace):

- [F1-primitives.md](families/F1-primitives.md) — button, toggle, toggle-group, separator, avatar
- [F2-selection-controls.md](families/F2-selection-controls.md) — switch, checkbox(+group), radio(+group)
- [F3-disclosure.md](families/F3-disclosure.md) — collapsible, accordion, tabs
- [F4-overlays.md](families/F4-overlays.md) — dialog, alert-dialog, drawer, popover, tooltip, preview-card, toast (+ the full positioning API = the M1 spec)
- [F5-menus.md](families/F5-menus.md) — menu, context-menu, menubar, navigation-menu, toolbar
- [F6-pickers.md](families/F6-pickers.md) — select, combobox, autocomplete
- [F7-form.md](families/F7-form.md) — field, fieldset, form, input, number-field, otp-field (defines M6)
- [F8-indicators-surfaces.md](families/F8-indicators-surfaces.md) — progress, meter, slider, scroll-area, skeleton

The tables below are the one-screen index; the family docs are
authoritative where they differ (two corrections already: accordion has NO
roving focus — base-ui deprecated it following the APG update; dialog
backdrop is `::backdrop`, not a DOM part).

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
| **accordion** | Root/Item/Header/Trigger/Panel | `open_items` signal (single/multiple static mode); plain tab stops (post-APG: NO roving focus); items = `Vec` children of item parts |
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

## 5. The styling contract (how a component is styled)

Modeled on base-ui's demo CSS (each demo ships css-modules + tailwind
variants; the css-modules form is the reference idiom: plain CSS, state
via attribute selectors — `.Switch[data-checked] { … }`), adapted to our
stack. **The reference stylesheets themselves are VENDORED into
[styling/](styling/)** (one file per component, MIT, source recorded per
file) — they are the styling acceptance criteria AND the load-bearing
geometry (thumb translate distances, transform-origin animations, toast
stacking transforms) that "headless" would otherwise silently drop; each
family doc links its files, and [styling/README.md](styling/README.md)
defines the mechanical adaptation rules (module classes → our part
classes, oklch literals → ThemeTokens). Adapted files ship as the OPT-IN
default stylesheet per component.

1. **Stable part classes, overridable.** Every part renders a default
   class (`switch`, `switch-thumb`, `dialog-popup`, `menu-item`…); config
   carries optional per-part overrides (`SwitchConfig.class`,
   `…thumb_class`). Default markup is styleable out of the box; design
   systems can rename. (base-ui's per-part `className` prop, minus React.)
2. **State styles target data-attributes, never classes.** The component
   NEVER toggles style classes; it toggles the documented data-attributes
   (family docs are the contract). Consumer CSS:
   `.switch[data-checked] { … }`, `.field-control[data-invalid] { … }`,
   `[data-side="top"] .arrow { … }`. Morph-safe and uniform.
3. **Machinery CSS variables are styling API.** M7 measurements
   (`--collapsible-panel-height`, `--active-tab-*`), M1 anchor vars
   (`--anchor-width`, `--available-height`, `--transform-origin`), M8
   movement vars (`--toast-swipe-movement-*`) — documented per family;
   animations/layout read them, JS never writes styles directly beyond
   these vars + the var-driven properties.
4. **Where CSS lives — three supported tiers:**
   - plain stylesheets (the base-ui idiom; everything is classes +
     attributes, so `to_markup` SSR pages style identically);
   - `<style primal:style>` scoped blocks (spec-39 F09) for styling a
     component INSTANCE at the use site without global leakage — no
     base-ui equivalent;
   - `#[derive(ThemeTokens)]` custom properties for design tokens with
     auto dark mode — component CSS references `var(--color-…)` tokens
     instead of hand-rolling `prefers-color-scheme` per rule (which is
     what base-ui's demos do).
5. **Focus styles are consumer CSS** via `:focus-visible` (the components
   guarantee correct focus TARGETS, never outline styles).
6. Each family's implementation feature must ship a styled reference
   example per component (the "headless claim" acceptance test from §6):
   the ADAPTED vendored stylesheet from [styling/](styling/), so ours and
   base-ui's demo can be visually compared rule for rule.

## 6. Implementation order

1. **Machinery**: M6 → M5 → M3 → M1 → M2 → M7 (M4 rides M2), each with JS
   tests + a proving component (M6→switch, M5→tabs, M1/M3→popover).
2. Families: F1+F2 (forms foundation) → F3 → F4 → F7 → F5 → F8 → F6 (needs
   feature 01 `<For>` — schedule feature 01 before F6 at the latest).
3. Each family gets its own feature dir (`05a-…` style or sequential
   numbers) with per-component acceptance tests: ARIA table compliance,
   keyboard contract, data-attribute contract (asserted on the op stream),
   morph survival, and a styled example proving the headless claim.

## 7. Crate

`backends/foundation_ui_components` (name from the old draft, kept):
`foundation_wasm_ui` + `foundation_signals` deps only; no_std-compatible
like its dependencies; components are functions + config/slot structs —
NO component trait (settled, feature 00 §2).

## 8. Amendments (2026-06-13, post-F1 review)

The first F1 + M6 pass landed the catalog's SHAPE but most of it was
non-functional scaffolding (effects with empty `let _ = sig.get();` bodies,
`callback_id = 0` placeholders, `_ctx`/`_rcv` unused, `Html{}` hand-built
AND DomOps hand-queued in parallel — duplicating `html!`). These amendments
fix the patterns BEFORE the catalog scales, and the finish-100% rule
applies: a component lands fully wired or not at all.

### 8.1 Config string fields are `Cow<'static, str>`

All text/class/label/aria config fields are `Cow<'static, str>` (optionals
`Option<Cow<'static, str>>`), constructed via `impl Into<Cow<'static, str>>`.

- **Rejected: `&'static str`.** Forces string literals; a caller can't pass
  an i18n string, a formatted label, or any runtime `String`.
- **Rejected: a config lifetime (`Config<'a>`).** Config values are *moved
  into `'static` effect closures* and pushed as `Cow<'static, str>` onto the
  attribute layer (`html!` already uses `Cow<'static, str>` — see
  `html_macro/codegen.rs`). A non-`'static` borrow cannot be captured, so a
  lifetime param fights the architecture and goes viral across 40 components.
- **Chosen: `Cow<'static, str>`.** `&'static str` literals stay borrowed
  (zero alloc); owned `String`/dynamic values convert in. Matches the wire
  attribute representation exactly — no extra conversion at emit time.

### 8.2 Lift the per-component effect boilerplate into `html!`

Components must NOT hand-build `Html{}`, hand-queue `CreateElement`/
`RegisterNode`/`SetAttribute`, or hand-write `ctx.effect` blocks for
attributes. `html!`'s reactive form already emits ids, mount ops, and one
effect per dynamic attribute (`codegen.rs:699`). Two capabilities make it
sufficient for the whole catalog:

- **Reactive attributes (already present):** `aria-checked={checked.get()}`
  re-runs an effect on change and queues `SetAttribute`. Use directly.
- **Option-valued dynamic attributes (NEW — the one macro change):** a
  dynamic attribute whose value is `Option`-like emits `SetAttribute` on
  `Some(v)` and `RemoveAttribute` on `None`. This is the missing primitive —
  the **presence data-attribute contract** (§1: `data-checked` present when
  on, ABSENT when off) cannot be expressed by always-`SetAttribute`
  (`data-checked="false"` still matches `[data-checked]` in CSS — a bug).
  Spelling: `data-checked={on.then_some("")}`. Also clears optional aria
  (`aria-label={cfg.aria_label}` where the value is `Option<Cow>`).
  Mechanically: the generated effect branches on an `IntoAttrValue` trait —
  `Option<T>` → Some/None, plain `T` → always-set (back-compat). Boolean
  attrs that are presence-style use `.then_some("")`; string-valued reactive
  attrs are unchanged.

- **Static interpolation `attr=[expr]` (NEW — the no-clone form):** `{expr}`
  attributes are REACTIVE — re-evaluated in an `FnMut` effect on signal change
  — so an owned value (`class`, `aria-label` from `Cow` config) would be moved
  out of the effect and force a `.clone()`. The bracket form `attr=[expr]`
  evaluates ONCE at build (no effect): the value moves in exactly once, no
  clone, no per-render effect. Rule of thumb: **`[…]` for static config
  values, `{…}` for signal reads** (`class=[class]`, `aria-pressed={on.get()}`,
  `data-checked={on.get().then_some("")}`). Both honor Option-valued semantics
  (`None` omits / removes).

After 8.1 + 8.2, a component is its `html!` block plus signal/slot plumbing:
no `{...}` clone-and-effect blocks except where a single effect legitimately
touches MULTIPLE nodes/attributes at once (rare; document why).

### 8.3 Correctness fixes folded into the F1 refactor

- `toggle`: drop `role="button"` — a native `<button>` + `aria-pressed` IS
  the toggle contract (base-ui adds no role); export `ToggleSlots`.
- `data-disabled`/`data-pressed`/`data-*` everywhere: presence-toggled via
  §8.2, never `"true"/"false"` strings.
- `avatar`: wire real `load`/`error` event callbacks to the status signal;
  honor `fallback_delay_ms`.
- `toggle_group`: real reactive items + M5 roving focus + ids/DOM ops.
- M6 `field`: effects emit the six data-attributes for real; setters wire to
  control focus/blur/input events; validator runs per `ValidationMode`.
