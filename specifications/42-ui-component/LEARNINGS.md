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

## Machinery completeness — base-ui ported 1:1

All the JS-side behaviors were subsequently ported from the base-ui source
(`@formulas/src.UIFrameworks/src.baseui/base-ui/packages/react/src`) rather than
left as stubs — the testbed verifies them in-browser later:

- **M8 pointer-gestures** (`machinery::gestures` + number-field): hold-repeat
  (`usePressAndHold` 400/60 + 8px scroll-cancel), scrub (`NumberFieldScrubArea`
  pointer-lock + virtual cursor + `pixelSensitivity=2` + teleport), snap
  (`toValidatedNumber` directional/nearest), slider drag (`getFingerState`),
  drawer/toast swipe (movement vars + threshold dismiss).
- **M3 safe-polygon** (`machinery::hover`): full `safePolygon.ts` port —
  geometry, trough rect, slow-cursor heuristic, per-side quadrilateral.
- **M5 typeahead** on BOTH variants: `listbox` (virtual highlight) and
  `composite` (roving) — `useTypeahead` 750 ms reset + same-letter cycling.
- **M1 collision** (`machinery::position`): `fallbackAxisSide` (logical
  start/end perpendicular fallback), `sticky`, arrow centering + `data-uncentered`.

## Genuinely remaining (compositional, not base-ui-algorithm)

- Drawer `snapPoints` (rest-at-partial-height) — swipe-to-dismiss ships.
- Nav-menu single morphing Viewport (per-item panels ship); menubar
  cross-trigger open-intent.
- F6 generic `T` + `to_form_value` (v1 is `String`-valued).
- Select macOS item-alignment positioning mode (anchored-below ships).
