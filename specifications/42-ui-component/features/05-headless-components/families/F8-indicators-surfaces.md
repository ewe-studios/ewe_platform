# F8 — Indicators & surfaces: progress, meter, slider, scroll-area, skeleton

Source: base-ui `types.md` references (2026-06-13).

## progress

Parts: Root / Track / Indicator / Label / Value.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `value: number \| null` (null = indeterminate) | `(value)` getter — `Option<f64>` | signal |
| `min = 0` / `max = 100` | static | |
| `format: Intl.NumberFormatOptions` + `locale` | static — Value part text ("75%") | |
| `getAriaValueText` / `aria-valuetext` | optional formatter config | |

Behaviors: `role="progressbar"` + `aria-valuenow/min/max/valuetext`;
Indicator width driven by an effect →
`SetStyle(width, calc%)` (or inset-inline for RTL correctness — captured);
states `data-progressing` / `data-complete` / `data-indeterminate`;
Label auto-wires `aria-labelledby`.

**Platform:** native `<progress>` + `accent-color` is the simple recipe;
indeterminate animation styling on native is limited — headless justified
for custom tracks. Document both.

## meter

Same anatomy/props minus indeterminate; `role="meter"`. Native `<meter>`
(with its low/high/optimum coloring) as the simple recipe.

## slider

Parts: Root / Control / Track / Indicator / Thumb / Value / Label.

| base-ui prop | ours | static/signal |
|--------------|------|---------------|
| `value: number \| number[]` triple + `onValueCommitted` | `(value, set_value)` — `f64` or `Vec<f64>` (range mode static) + `on_commit` | signal |
| `min = 0` / `max = 100` / `step = 1` / `largeStep = 10` | static | PageUp/Down use largeStep |
| `minStepsBetweenValues = 0` | static | range-thumb separation |
| `thumbAlignment: 'center' \| 'edge'` | static | thumb-at-extremes geometry |
| `thumbCollisionBehavior: 'swap' \| 'stop'` | static | range thumbs crossing |
| `orientation`, `locale`, `format`, `name`/`form` | static | Value part formats; hidden range inputs per thumb for forms |
| `disabled` | static | |

Behaviors: each Thumb contains a hidden native `<input type="range">`
(focus + form + AT carrier — captured pattern); keyboard on thumb:
arrows ±step, PageUp/Down ±largeStep, Home/End → min/max (native-ish via
the hidden input, intercepted for format); pointer drag on Control moves
the NEAREST thumb (gesture module — drag ships WITH slider v1, it is the
component; shared module also serves drawer-swipe/scrub later); CSS vars
for thumb position/percent on Root so Track/Indicator style in pure CSS;
`data-dragging`, `data-orientation`, field data-attrs.

**Platform:** `<input type=range>` + `accent-color` for the basic case;
new `::slider-track`/`::slider-thumb`/`::slider-fill` pseudo-elements
(Chrome experimental) noted as future replacement — not Baseline, watch.

## scroll-area

Parts: Root / Viewport / Content / Scrollbar(×2) / Thumb / Corner.
Custom-styled scrollbars over native scrolling.

| base-ui prop | ours |
|--------------|------|
| Root `overflowEdgeThreshold {xStart,xEnd,yStart,yEnd}` | static — how close to an edge before edge-attrs flip |
| Scrollbar `orientation`, `keepMounted` | static |

Captured data-attribute surface (rich — it's the styling API):
`data-has-overflow-x/y`, `data-overflow-x/y-start/end` (fade/shadow
edges), `data-scrolling` (show-while-scrolling bars), `data-hovering`,
scrollbar `data-orientation`; CSS vars `--scroll-area-thumb-width/height`
+ corner sizes.

Behaviors: native scroll retained (viewport is the scroller —
accessibility + momentum free); thumbs sync via scroll events
(JS presentation module; rAF-throttled); thumb drag scrolls; bars
overlay without layout shift.

**This is a PURE-presentation component**: NO Rust signals at all —
markup from Rust (pure form works), behavior entirely in the JS runtime
module keyed off `data-scroll-area`. The catalog's proof that "component"
≠ "signals".

**Platform:** `scrollbar-width` + `scrollbar-color` (Baseline 2024) cover
slim/colored bars with ZERO JS — REQUIRED first recommendation in docs;
`scrollbar-gutter: stable` for layout. The component exists only for
fully custom bars (overlay styling, fade-on-idle). Scroll-driven
animations (`animation-timeline: scroll()`) can replace `data-scrolling`
styling where supported — recipe documented.

## skeleton (ours — kept from the original draft; no base-ui equivalent)

`skeleton(shape: SkeletonShape)` — `Line{width}`, `Circle{size}`,
`Block{w,h}`, or wrapping children with `data-loading`. Fully static,
pure-capable; CSS does the shimmer. `aria-hidden="true"` + the REAL
loading semantics belong to the replaced region (`aria-busy` on the
container — documented pattern, F1 button's `loading` consistency).

## Shapes

```rust
pub fn progress(ctx, rcv, cfg: ProgressConfig, value: SignalGetter<Option<f64>>, slots{label}) -> Html;
pub fn meter(ctx, rcv, cfg, value: SignalGetter<f64>, slots{label}) -> Html;
pub fn slider(ctx, rcv, cfg: SliderConfig, value, set_value, slots{value_label}) -> Html;
pub fn scroll_area(cfg: ScrollAreaConfig, children: Vec<Html>) -> Html;   // PURE
pub fn skeleton(shape: SkeletonShape) -> Html;                            // PURE
```

Machinery: pointer-gestures (slider drag — first consumer), M6 (slider
forms), M7. JS runtime: scroll-area module. Tests: progress/meter aria +
style ops per value change incl. RTL, slider keyboard matrix + drag
(JS suite with synthetic pointer events) + range collision modes +
hidden-input form serialization, scroll-area attr flips on synthetic
scroll/overflow, skeleton pure markup via `to_markup`.
