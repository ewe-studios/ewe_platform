# 020 — Theme System: Tailwind-style utility classes generated from Rust

**Date:** 2026-06-08  
**Status:** Resolved

### Decision

A **theme system** generates Tailwind-style atomic utility classes at compile-time. All CSS is generated at build time and injected into `<head>` as a single `<style>` tag by the WASM runtime on initialization.

### Theme definition: struct + derive macro

```rust
#[derive(ThemeTokens)]
struct AppTheme {
    colors: Colors {
        primary: "#3b82f6",
        secondary: "#10b981",
    },
    spacing: Spacing {
        sm: "8px",
        md: "16px",
        lg: "24px",
    },
}
```

The derive macro generates at **compile-time**:
- A `Theme` struct with builder methods
- A static CSS string containing all utility class rules
- Sensible defaults for light/dark themes

### Usage in Context

```rust
fn init(ctx: &Context) {
    let theme = AppTheme::new();
    ctx.set_theme(theme);
}
```

When the WASM process starts, the theme CSS is the **first thing** sent to the JS side — included in the initial batch of DOM operations. The runtime injects it into `<head>`. One-shot: runs once, gone until page reload. No persistent server state.

### Applying theme classes

Users write raw utility class names directly in HTML, like Tailwind:

```rust
html! {
    <div class="bg-primary text-white p-md m-sm">
        Hello
    </div>
}
```

No `theme.classes()` wrapper needed — class names are just strings. The theme system generates the CSS that makes those classes work.

### Built-in utility classes

Following Tailwind's pattern, built-in utilities cover:

| Category | Examples |
|----------|----------|
| Layout | `relative`, `absolute`, `fixed`, `flex`, `grid`, `block`, `inline` |
| Spacing | `p-sm`, `p-md`, `p-lg`, `m-sm`, `m-md`, `m-lg` |
| Sizing | `w-full`, `h-screen`, `max-w-md` |
| Typography | `text-sm`, `text-lg`, `font-bold`, `text-center` |
| Colors | `bg-primary`, `text-white`, `border-secondary` |
| Borders | `rounded-sm`, `rounded-md`, `rounded-lg`, `border`, `border-2` |
| Shadows | `shadow-sm`, `shadow-md`, `shadow-lg` |
| Display | `hidden`, `visible`, `opacity-0`, `opacity-100` |

Theme-defined classes use the user's tokens. Built-in classes use sensible defaults (light/dark themes).

### Custom CSS alongside

**Without `primal:style`** — added as-is, not scoped:
```html
<style>
  .my-custom-class { color: red; }  /* global, not scoped */
</style>
```

**With `primal:style`** — scoped to parent element (id/class prefix):
```html
<div id="my-component">
  <style primal:style>
    .title { font-size: 20px; }
  </style>
</div>
<!-- Transformed to: -->
<style>#my-component .title { font-size: 20px; }</style>
```

### CSS generation crates

The derive macro uses a CSS parsing crate to generate and validate utility classes:
- `lightningcss` — full CSS parser/transformer. Heavy but compile-time only.
- `cssparser` — Mozilla's CSS tokenizer. Lighter, requires building AST ourselves.

**Recommendation:** `lightningcss` for full CSS transformation support (decision 019 notes extraction from the lightningcss source we already have).

### Why this design

- **Compile-time generation** — zero runtime cost for CSS generation
- **Tailwind-style utilities** — familiar, well-understood pattern
- **Theme as code** — struct + derive macro, type-safe
- **First-batch injection** — WASM sends CSS on init as the first DOM op, ensuring styles are ready before any content renders
- **Custom CSS alongside** — `primal:style` for scoped, plain `<style>` for global
- **All in Rust** — no JS-side CSS processing
