# Reference styles (vendored from base-ui, MIT)

One CSS file per component, copied from base-ui's hero demo
(`docs/src/app/(docs)/react/components/<component>/demos/hero/css-modules/
index.module.css`; each file's header comment records its exact source —
popover uses the detached-triggers demo, preview-card and accordion use
their shared demo stylesheets, and `radio.css` covers radio-group since the
radio hero demo IS a radio group).

## Why these are in the spec

1. **They are the acceptance criteria for our data-attribute contract.**
   Every selector here (`[data-checked]`, `[data-side="top"]`,
   `[data-starting-style]`, `var(--anchor-width)`,
   `var(--toast-swipe-movement-y)`, …) must be satisfiable against OUR
   rendered markup with only mechanical adaptation (class names →
   our stable part classes, CSS-module composition → plain classes).
   A component implementation is not done until its reference stylesheet,
   adapted, produces the base-ui demo's look and motion.
2. **They are the load-bearing geometry we'd otherwise lose** — thumb
   translate distances, popup transform-origin animations, toast stacking
   transforms, panel height transitions. "Headless" never meant "no
   reference styling"; it means styling is REPLACEABLE.
3. **They seed the optional default stylesheet tier** (features.md §5):
   `foundation_ui_components` ships these (adapted, tokenized via
   ThemeTokens custom properties instead of hand-rolled
   prefers-color-scheme) as opt-in per-component CSS — batteries included,
   removable.

## Adaptation rules (when porting a file)

- `.PascalCase` module classes → our stable part classes
  (`.Switch` → `.switch`, `.Thumb` → `.switch-thumb`).
- `composes:` / CSS-module specifics → expanded plain CSS.
- Hard-coded `oklch(...)` light/dark pairs → `var(--color-*)` tokens
  (ThemeTokens), keeping the demo values as token defaults.
- Keep every attribute/var selector EXACTLY — that's the contract under
  test.
