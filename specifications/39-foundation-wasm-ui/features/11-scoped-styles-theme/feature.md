# Feature 11: Scoped Styles & Theme System

## Description

Two compile-time CSS systems: (1) scoped `<style>` tags transformed via `:parent` pseudo-class replacement, and (2) Tailwind-style utility classes generated from a `#[derive(ThemeTokens)]` struct. All CSS generated at compile-time, injected into `<head>` as first batch on WASM init.

**Decisions:** 019, 020

## Module

- Compile-time: `crates/foundation_wasm_ui/src/html_macro/scoped_css.rs` + `crates/foundation_wasm_ui/src/theme/`
- Runtime: `foundation-wasm-ui.js` (runtime fallback for dynamic content)

## Scoped Style Tags

### Compile-time transformation

```html
<div id="menu-tabs">
  <style scoped primal:style>
    :parent { background: white; }
    :parent:hover { opacity: 0.8; }
    :parent .title { font-size: 20px; }
  </style>
</div>
<!-- Transformed: -->
<style>
  #menu-tabs { background: white; }
  #menu-tabs:hover { opacity: 0.8; }
  #menu-tabs .title { font-size: 20px; }
</style>
```

`:parent` parsed by `lightningcss` as valid pseudo-class (unknown but accepted), replaced at compile-time. If parent has no id, uses first class.

### Smart prefixing

1. `:parent` → replaced with parent id/class
2. Selectors matching parent's id/classes → left as-is (already scoped)
3. Nested CSS (`&`) → inherits scoping
4. Other selectors → prepended with parent id/class
5. Custom properties (`--primary`) → left as-is

### Multiple styles combined

```html
<div id="menu-tabs">
  <style scoped primal:style>.title { font-size: 20px; }</style>
  <style scoped primal:style>.subtitle { color: gray; }</style>
</div>
<!-- Combined: -->
<style>#menu-tabs .title { font-size: 20px; } #menu-tabs .subtitle { color: gray; }</style>
```

### Scoped Script Tags

```html
<div id="menu-tabs">
  <script scoped primal:script>
    function(scope) {
      let targets = scope.targets();
      primal.on(targets, "click", () => { ... });
    }
  </script>
</div>
```

Script format: `function(scope){...}` body. JS runtime extracts `textContent`, hydrates via `new Function("scope", ...)`.

**Scope object:**
| Method | Returns |
|--------|---------|
| `scope.targets()` | Array of target elements |
| `scope.parent()` | Parent DOM node |
| `scope.querySelector(sel)` | Query scoped to parent |

**`primal` global** in scoped scripts:
- `primal.on(target, eventType, handler)` — event listener with auto-cleanup
- `primal.onclick(target, handler)` — shortcut
- `primal.onchange(target, handler)` — shortcut

## Theme System

### Theme definition

```rust
#[derive(ThemeTokens)]
struct AppTheme {
    colors: Colors {
        primary: "#3b82f6",
        secondary: "#10b981",
    },
    spacing: Spacing {
        sm: "8px", md: "16px", lg: "24px",
    },
}
```

Derive macro generates at **compile-time**:
- `Theme` struct with builder methods
- Static CSS string with all utility class rules
- Light/dark theme defaults

### CSS generation

The derive macro produces a static CSS string covering:

| Category | Examples |
|----------|----------|
| Layout | `relative`, `absolute`, `flex`, `grid`, `block` |
| Spacing | `p-sm`, `p-md`, `m-sm`, `m-md` |
| Sizing | `w-full`, `h-screen`, `max-w-md` |
| Typography | `text-sm`, `text-lg`, `font-bold` |
| Colors | `bg-primary`, `text-white`, `border-secondary` |
| Borders | `rounded-sm`, `border`, `border-2` |
| Shadows | `shadow-sm`, `shadow-md`, `shadow-lg` |
| Display | `hidden`, `opacity-0`, `opacity-100` |

### First-batch injection

WASM sends theme CSS as the **first DOM operation** on initialization. Injected into `<head>` before any content renders.

### Usage in HTML

```rust
html! {
    <div class="bg-primary text-white p-md m-sm">Hello</div>
}
```

Raw class names — no `theme.classes()` wrapper needed.

### Custom CSS alongside

**Without `primal:style`** → global, not scoped.
**With `primal:style`** → scoped to parent element.

## Dependencies

- Feature 03 (html! macro — for compile-time transform)
- Feature 07 (runtime fallback for dynamic content)

## Testing

- Scoped style: `:parent` → replaced with correct id/class
- Scoped style: nested `&` → inherits scoping
- Scoped style: custom properties → left as-is
- Multiple styles → combined into one
- Scoped script: `function(scope){...}` → executed with correct scope
- Theme: derive macro → CSS string generated
- Theme: utility classes → correct CSS rules
- Theme injection: first batch → CSS in `<head>` before content
