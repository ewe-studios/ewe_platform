# Spec-42 LEARNINGS — feature 05 headless components

Captured while building the `foundation_ui_components` catalog (F1–F8 +
machinery M1–M7). These are the non-obvious decisions and gotchas a future
implementer (or feature 06 auth-UI) needs.

## Machinery delivery = scoped scripts (the spine)

Every JS behavior is a `function(scope){…}` in a `<script scoped primal:script>`
node the component embeds in its root (`machinery::scoped_script`). The existing
spec-39 hydrator runs it once with `scope` (`scope.parent()` = the component
root, `scope.addEvent` auto-cleans on disconnect) and removes it. Consequences:

- **No `register_function`, no `cfg(wasm32)` gating, no mount-timing dance.** A
  behavior is just markup. Works for SSR (`to_markup`) and the reactive runtime
  (the MutationObserver hydrates inserted subtrees).
- **JS → Rust state changes go through a synthesized `.click()`** on a hidden
  element whose `primal:onclick` the component already wired (e.g.
  `set_open(false)`). M3 dismiss, M5 move-and-select, M7 complete, hover, OTP,
  dialog cancel all use this one bridge — no JS↔signal FFI needed.
- **Open-state seam:** most popup machinery (M2 scroll-lock, M4 focus-trap, M7
  transitions, dialog driver) watches `data-open` via a `MutationObserver`; the
  component's open-signal effect toggles `data-open`/`data-closed`.

## GOTCHA: the `html!` macro reserves the `primal:` prefix

The macro only accepts `primal:on*`, `primal:style`, `primal:script` — any other
`primal:foo` is a compile error (`unknown primal attribute`). So machinery INPUT
attributes a component emits cannot be `primal:`-namespaced from the macro. We
moved them to `data-*`: `data-anchor`, `data-prefer-side`, `data-prefer-align`,
`data-dialog-mode`. (Positioner OUTPUTS `data-side`/`data-align` are unchanged —
they're the final post-collision placement.) `primal:script` is still emitted,
but only by `scoped_script`, which builds the `Html` struct directly.

## GOTCHA: reactive `{expr}` attributes are one closure EACH

The macro wraps every `{…}` attribute value in its own `FnMut` reactive closure.
A getter/closure referenced by N attributes is moved N times → use **one clone
per attribute usage** (the `field_state` pattern: `d_valid`, `d_invalid`, …).
Same for `Option<SignalGetter>` — clone inside the closure before `.map`/
`.and_then` (the closure is re-invoked).

## GOTCHA: reactive child slots are TEXT-only

`html! { {expr} }` child slots render TEXT; element/`Html` children embedded that
way silently render nothing. Wrap every element embed in `<Fragment>{…}</Fragment>`
(applies to slot output, built children, AND machinery `<script>` nodes). Text
content takes `IntoHtml` — `&str` literals and `Html::text(…)` work; bare `Cow`
does not, so use `Html::text(cow)` inside a Fragment.

## `<For>` constraints

`<For each={…}>` is `Fn() -> Vec<T>` (reactively tracked), so `T: Clone`:
- Reactive list: `each={getter.get()}` (the macro wraps it; recomputes on change).
- Static list: `each={items.clone()}` (NOT a bare move — needs to clone per call).
- If the item type holds a `Slot` (not `Clone`), it can't go through `<For>` —
  render the static list **eagerly** with a map + `<Fragment>` (see `nav_menu`,
  `menu`). `<For>` is for value-typed, reactive lists (toast, select, combobox).

## Structured slot enums beat markup slots for owned-ARIA components

`menu`'s `MenuEntry` and the pickers' `PickItem` are DATA, not markup, because
the component owns roles/highlight/typeahead. `MenuEntry::Item` carries a
`Box<dyn Fn()>` action (not a pre-built `Callback`) so the menu can compose
`action()` + `set_open(false)` — the renderer must consume entries BY VALUE to
move the box into the click callback.

## Pure components work (no signals)

`separator`, `scroll_area`, `skeleton` use the pure `html! { … }` form (no
ctx/rcv). `scroll_area` proves "component ≠ signals": all behavior is a JS module
keyed off `data-scroll-area`.

## Deliberately deferred (spec-sanctioned follow-ups)

- **M8 pointer-gestures** (slider drag, number-field scrub/hold-repeat,
  drawer swipe/snap, toast swipe) — API reserved (`SliderConfig`, `NumberField`,
  `DrawerSide`), gestures land later in one shared module.
- **M3 safe-polygon** hover (submenu/hoverable popups) — timer hover ships; the
  polygon is the refinement.
- **M5 typeahead** for the roving (composite) variant — virtual-highlight
  (listbox) variant HAS typeahead; the roving variant's is the follow-up.
- **M1** `fallbackAxisSide` / `sticky` / arrow centering / select item-alignment.
- **F6 generic `T`** + `to_form_value` (v1 is `String`-valued).
- **Nav-menu single morphing Viewport**; menubar cross-trigger roving/open-intent.
