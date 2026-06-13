# Theming & scoped styles

## The theme system

A theme is a table of design tokens (colors, spacing, radii, …) and the CSS they
imply (`:root` custom properties, a dark `@media` override, utility classes). It's
generated **one way** regardless of how tokens are declared.

### `theme!{}` — the headline API

```rust
use foundation_wasm_ui::{theme, App};

let app = App::new().theme(theme! {
    colors  { primary: { light: "#3b82f6", dark: "#60a5fa" },   // explicit dark
              accent:  "#10b981" }                              // dark auto-derived
    spacing { sm: 8px, md: 16px }                               // bare suffixed literals
    radius  { card: 8px }
    shadow  { card: "0 1px 3px rgba(0,0,0,0.1)" }               // quoted CSS value
    font_size { base: 1rem }
    animation { fast: 250ms }
});
```

The enforced blocks are: `colors`, `spacing`, `padding`, `margin`, `radius`,
`shadow`, `font_size`, `animation` (unknown blocks are a compile error). Each entry
value is a quoted string (verbatim CSS), a bare suffixed literal (`16px`, `250ms`,
`1.5rem`), or the `{ light, dark }` brace form.

### Runtime builder (no macro)

```rust
use foundation_theme::Theme;
let theme = Theme::new()
    .color("primary", "#3b82f6", Some("#60a5fa"))
    .spacing("md", "16px")
    .radius("card", "8px")
    .token("custom-category", /* … */)   // escape hatch for custom categories
    .build();                            // -> GeneratedTheme
let app = App::new().theme(theme);
```

### Generated classes & scales

Named token classes follow the category: colors → `.bg-*`/`.text-*`/`.border-*`,
spacing → `.p-*`/`.m-*`, radius → `.rounded-*`, shadow → `.shadow-*`, font-size →
`.text-*`, animation → `.duration-*`. Plus a fixed set of **calculable utility
scales** (computed from the index):

```html
<div class="opacity-50 text-56-rem h-50-vh p-16 m-8 w-full font-bold">…</div>
```

### How it works

`theme!{}`, `#[derive(ThemeTokens)]` (legacy), and `Theme::build()` all funnel into
one generator, `foundation_theme::theme_css(&[ThemeToken])`. Output: `:root`
custom properties, a dark `@media` block (colors only — explicit `dark` verbatim,
otherwise auto-derived at ~80% luminance from `#rrggbb`/`#rgb`), per-category
utility classes, and the calculable scales.

`App::theme(generated)` takes ownership and **immediately** queues the stylesheet's
head-injection ops (`inject_theme_css`), so the theme CSS reaches `<head>` before
the first content batch. `App::get_theme()` returns `Option<&GeneratedTheme>`.

### Gotchas

- **Not a full utility framework.** Scales are a fixed, calculable set; one-off
  values use a named token or inline `style=""` — there's no `[arbitrary]` syntax.
- Dark auto-derivation applies only to **colors given as hex**; `rgb()`/`var()` or
  non-color tokens get no auto dark value.
- Injection is one-shot at startup — the theme isn't reactive (changing tokens
  means re-injecting).
- `#[derive(ThemeTokens)]` is legacy — prefer `theme!{}` or the builder.

## Scoped styles

A `<style primal:style>` child holds a CSS **string literal** scoped to its parent
component at compile time:

```rust
let h = html! {
    <div class="counter">
        <style primal:style>{r#"
            .display { font-size: 2rem; }
            :parent { border: 1px solid #ccc; }
        "#}</style>
        <span class="display">{count.get()}</span>
    </div>
};
```

At compile time the macro extracts every `<style primal:style>` child, combines
their CSS per parent, and rewrites selectors against the parent's identity (`#id`
wins, else first `.class`); `:parent` targets the host. In the **pure** form the
rewritten CSS stays inline as `<style data-primal-scoped>`; in the **reactive**
form it's queued as head-injection ops.

### Gotchas

- The content **must be a single quoted string literal** (raw CSS doesn't tokenize
  as Rust).
- The parent must have an `id` or `class` to scope against (else a compile error).
- It's not dynamic — fixed at compile time, not signal-driven.

See also: **[getting started](./getting-started.md)** ·
**[internals](./internals.md)** (reserved node ids).
