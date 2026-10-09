# foundation_theme

Design-token themes + the single CSS generator shared by the `theme!{}` macro,
the `#[derive(ThemeTokens)]` form, and the runtime `Theme` builder — plus
**generative palette & Bézier-curve helpers** to make a designer's life joyous.

`no_std + alloc`. No dependency on the DOM/runtime crates — pure token → CSS.

## Contents
- [Why this crate](#why-this-crate)
- [Defining a theme](#defining-a-theme) — `theme!{}`, `Theme` builder, derive
- [`GeneratedTheme`](#generatedtheme)
- [Using it with `App`](#using-it-with-app)
- [The CSS it generates](#the-css-it-generates)
- [Calculable utility scales](#calculable-utility-scales)
- [Generative palettes (`palette`)](#generative-palettes-palette)
- [Bézier curves (`curve`)](#bézier-curves-curve)

---

## Why this crate

A theme is a table of design tokens (colors, spacing, radii…) and the CSS those
tokens imply (`:root` custom properties, a dark-mode block, utility classes).
That CSS must be generated **the same way** no matter how the tokens were
declared. This crate is that single source of truth:
`theme_css(&[ThemeToken]) -> String`. Everything else is a front-end over it.

> Full rationale: `specifications/39-foundation-wasm-ui/decisions/021-theme-macro-and-runtime.md`.

---

## Defining a theme

Three front-ends, identical output. Pick by *who reads the values*.

### 1. `theme!{}` macro — the headline API (compile-time)

Reads like a struct; values mostly unquoted; one macro to learn. (Lives in
`foundation_macros`, re-exported by `foundation_wasm_ui`.)

```rust
let theme = theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" },  // explicit dark
              secondary: "#10b981",                            // dark auto-derived
              bg: "#ffffff" }
    spacing { sm: 8px, md: 16px, lg: 24px }                    // unquoted dimensions
    radius  { sm: 4px }
    shadow  { soft: "0 1px 2px rgba(0,0,0,.1)" }               // quote multi-part values
};
```

- **Blocks** (a fixed, enforced set; unknown = compile error): `colors`,
  `spacing`, `padding`, `margin`, `radius`, `shadow`, `font_size`, `animation`.
- **Values**: a bare suffixed literal (`16px`, `250ms`, `1.5rem`) — no quotes;
  or a quoted string for colors / multi-part values; or `{ light, dark }`.
- Returns a `const`-capable `GeneratedTheme` with compile-time `&'static str` CSS.

### 2. `Theme` builder — runtime, no macro

For themes assembled from config/values at runtime:

```rust
use foundation_theme::Theme;

let theme = Theme::new()
    .color("primary", "#3b82f6", Some("#60a5fa"))
    .color("secondary", "#10b981", None)   // None → dark auto-derived
    .spacing("md", "16px")
    .radius("sm", "4px")
    .build();                              // -> GeneratedTheme (owned CSS)
```

### 3. `#[derive(ThemeTokens)]` — legacy, retained

Tokens in `#[token(...)]` attributes. Kept for back-compat; no longer the
headline API. Generates `Theme::CSS`, `Theme::new()`, `css_string()`.

---

## `GeneratedTheme`

```rust
let theme: GeneratedTheme = /* theme!{} or Theme::build() */;
theme.to_css();                 // &str — inject into <head>
theme.tokens();                 // &[ThemeToken] — inspect
theme.value("color", "primary") // Option<&str> — look up a token
```

`GeneratedTheme { css: Cow<'static,str>, tokens: Cow<'static,[ThemeToken]> }`
unifies the macro (`Cow::Borrowed`, `const fn from_static`) and the builder
(`Cow::Owned`, `from_tokens`).

---

## Using it with `App`

`App` (in `foundation_wasm_ui`) owns the theme and injects its CSS into `<head>`
on install:

```rust
let app = App::new().theme(theme! { colors { primary: "#3b82f6" } });
let t   = app.get_theme();   // Option<&GeneratedTheme>
```

---

## The CSS it generates

`theme_css` emits, in order:
1. `:root { --{category}-{name}: {light}; … }` custom properties;
2. a `@media (prefers-color-scheme: dark)` block — colors only; explicit dark
   verbatim, missing dark colors **auto-derived** at ~80% luminance
   (`#10b981 → #0d9467`); non-hex values left alone;
3. per-category utility classes — `color`→`.bg-/.text-/.border-`,
   `spacing`→`.p-/.m-`, `radius`→`.rounded-`, `shadow`→`.shadow-`, etc.;
4. the built-in utilities + the generated numeric scales (below).

---

## Calculable utility scales

Every numeric utility is a pure function of its index, so the set is complete
and predictable (no missing `opacity-35`). Named tokens (`.p-md`) and numeric
scales (`.p-16`) coexist by namespace.

| prefix | property | range / step | units (primary first) |
|--------|----------|--------------|------------------------|
| `opacity` | opacity | 0–100 / 5 | ratio (`.opacity-50` = .5) |
| `w` / `h` | width/height | 0–100 / 5 | `%` (bare) + `vw`/`vh` |
| `p` / `m` / `gap` | padding/margin/gap | 0–64 / 4 | `px` (bare) + `rem` |
| `text` | font-size | 8–72 / 2 | `px` (bare) + `rem`/`em`/`vh` |
| `border` | border-width | 0–8 / 1 | `px` (bare) + `rem`/`em` |
| `font` | font-weight | 100–900 / 100 | raw |

Bare class = primary unit (`.text-16` = 16px); unit-tagged twins let you pick
the measured property (`.text-56-rem`, `.h-50-vh`).

> Philosophy & how to add a scale:
> `specifications/39-foundation-wasm-ui/features/09-scoped-styles-theme/token-scale-design-guide.md`.

---

## Generative palettes (`palette`)

Stop hand-picking colors — generate harmonious sets and feed them to `Theme`.

```rust
use foundation_theme::palette::{pastel, gaming, ramp, dual_theme, Hsl};

let soft   = pastel(20.0, 5);            // 5 soft pastels, hues evenly spread
let neon   = gaming(0.0, 6);             // 6 vivid/neon colors
let shades = ramp(210.0, 0.6, 9, 0.42, 1.0); // a 9-step light→dark blue ramp,
                                              // lightness eased by a Bézier
let (light, dark) = dual_theme(210.0, 0.6, 5); // MIRRORED light & dark palettes
                                               // designed to pair as theme modes
let hex = Hsl::new(210.0, 0.6, 0.5).to_hex();  // "#3380cc"-ish
```

- `pastel` / `gaming` — even-hue sets at a soft / punchy `(s, l)`.
- `ramp` — single-hue shade scale, lightness distributed along a cubic Bézier.
- `dual_theme` — two opposite lightness curves → a light palette and a dark
  palette whose tokens are visual counterparts.
- `Hsl` — `to_hex()` / `to_rgb()`, `no_std`-safe (pure arithmetic, no trig).

Feed straight into a theme:

```rust
let [c0, c1, c2, c3, c4] = /* pastel(...) as array */;
let theme = Theme::new().color("brand-1", &c0, None).color("brand-2", &c1, None).build();
```

## Bézier curves (`curve`)

The math behind the eased ramps — also useful for SVG strokes and CSS easing.

```rust
use foundation_theme::curve::{cubic_bezier, ease, sample, svg_path, cubic_bezier_css, Point};

let p = cubic_bezier(Point::new(0.,0.), Point::new(0.,1.), Point::new(1.,1.), Point::new(1.,0.), 0.5);
let e = ease(0.5, 0.42, 1.0);                 // 1D ease value in [0,1]
let pts = sample(/* p0..p3 */, 24);           // 24 points along the curve
let d = svg_path(/* p0..p3 */);               // "M … C …" for an <svg> path
let css = cubic_bezier_css(0.42, 0.0, 0.58, 1.0); // "cubic-bezier(…)" timing fn
```

`no_std`-safe: cubic Béziers are pure polynomials (no `sqrt`/`powf`/trig).
