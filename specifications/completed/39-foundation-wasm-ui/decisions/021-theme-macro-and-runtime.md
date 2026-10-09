# 021 — Theme: one `theme!{}` macro + shared runtime codegen

**Date:** 2026-06-13
**Status:** Resolved
**Supersedes the ergonomics of:** [020](020-theme-system.md) (the `#[derive(ThemeTokens)]`
form stays, but is no longer the headline API)

### Problem

Decision 020 sketched a theme as a struct with nested literal values:

```rust
#[derive(ThemeTokens)]
struct AppTheme { colors: Colors { primary: "#3b82f6" } }   // NOT valid Rust
```

That exact form is **impossible as a `#[derive]`**: a derive receives a *type
definition* (`field: Type`), and you cannot put *values* (`primary:
"#3b82f6"`) in a type position. The shipped workaround moved values into
`#[token(...)]` attributes on unit fields (`primary: ()`), which works but is
attribute-soup: users must learn macro/attribute mechanics for something they
just want to *declare*.

Root cause (worth stating once): **a macro only ever sees tokens, never a
value.** A `#[derive]` sees the definition; a function-like macro sees whatever
literal token stream you write between its braces. So:

- to read token *values* at COMPILE time → they must be written **inline** in a
  function-like macro (`theme!{ … }`), not in a separate const it references;
- to read them at RUNTIME → instantiate a normal struct and hand the **value**
  to a function. Both are valid; they differ only in *who* reads the values.

### Decision

**One** function-like macro, `theme!{}`, plus a shared codegen crate so the
macro, the legacy derive, and a runtime builder all generate identical CSS.

#### `theme!{}` — the headline API (compile-time)

Struct-like feel, enforced blocks, values mostly unquoted:

```rust
let theme = theme! {
    colors {
        primary:   { light: "#3b82f6", dark: "#60a5fa" },  // explicit dark
        secondary: "#10b981",                              // dark auto-derived
        bg:        "#ffffff",
    }
    spacing { sm: 8px, md: 16px, lg: 24px }   // unquoted dimension tokens
    radius  { sm: 4px, md: 8px }
    shadow  { soft: "0 1px 2px rgba(0,0,0,.1)" }   // multi-part values: quote
    animation { fast: 150ms, base: 250ms }
};
```

- **Blocks are a fixed, known set** — `colors`, `spacing`, `padding`,
  `margin`, `radius`, `shadow`, `font_size`, `animation`. An unknown block name
  is a COMPILE ERROR (this is the "enforcement"); each block maps to a token
  *category*.
- **Values**: a bare suffixed literal where Rust lexes it as one token
  (`16px`, `250ms`, `1.5rem`, `2`) — stringified verbatim, no quotes. Colors
  and multi-part values (`#`, spaces, `rgba(...)`) stay quoted string literals.
- **Per-token dark**: the brace form `name: { light: "...", dark: "..." }`. A
  plain value auto-derives dark for colors (~80% luminance), same as 020.
- **Returns** a `GeneratedTheme` — `const`-capable — holding the parsed token
  table AND the compile-time `&'static str` CSS, with `to_css()`. Inspectable
  and reviewable.

#### Runtime builder — no macro at all

For themes assembled from config/values at runtime:

```rust
let theme = Theme::new()
    .color("primary", "#3b82f6", Some("#60a5fa"))
    .color("secondary", "#10b981", None)
    .spacing("md", "16px")
    .build();   // -> GeneratedTheme (Cow::Owned css), via the SAME theme_css()
```

Same output type (`GeneratedTheme`) so everything downstream is uniform.

#### `App` owns the theme

```rust
let app = App::new().theme(theme!{ … });   // App injects CSS into <head> on boot
let t   = app.get_theme();                  // Option<&GeneratedTheme>
```

`App::theme(GeneratedTheme)` stores it and queues the head-injection sequence
(reusing `inject_theme_css`, widened to `&str`). `App::get_theme()` reads it
back.

### Architecture — the shared codegen

New crate **`foundation_theme`** (`no_std` + `alloc`):

- `ThemeToken { name, category, light, dark }` (all `Cow<'static, str>`), with
  `const fn` constructors so the macro can emit it in `const` position.
- `GeneratedTheme { css: Cow<'static,str>, tokens: Cow<'static,[ThemeToken]> }`
  + `const fn from_static(...)`, `to_css()`, and `Theme` runtime builder →
  `build()`.
- `theme_css(&[ThemeToken]) -> String` — the ONE generator (`:root` custom
  properties, dark `@media` block with auto-derivation, per-category utility
  classes, the built-in utility set). Lifted out of the proc-macro.

Front-ends, all calling `theme_css`:

| front-end | reads values | when | css type |
|-----------|--------------|------|----------|
| `theme!{}` macro | inline literals | compile | `&'static str` |
| `#[derive(ThemeTokens)]` (legacy, kept) | `#[token]` attrs | compile | `&'static str` |
| `Theme` builder | struct fields | runtime | `String` |

`foundation_macros` gains a normal dep on `foundation_theme` and calls
`theme_css` at expansion time, embedding the result as a string literal.
`foundation_wasm_ui` re-exports `theme!`, `Theme`, `GeneratedTheme`,
`ThemeToken`.

### Why

- **One macro to learn**, and it reads like a struct — the 020 ergonomic goal,
  now reachable because a *function-like* macro can hold values inline.
- **No mandatory macro** — the runtime builder covers config-driven themes.
- **Single source of truth** for CSS generation; the derive stays for
  back-compat but is no longer load-bearing.
- **`App` ownership** keeps `Context` (in `foundation_signals`) UI-agnostic.
