# Feature 09: Scoped Styles & Theme System

**Crate path:** `crates/foundation_wasm_ui/src/html_macro/scoped_css.rs` (scoped styles), `crates/foundation_wasm_ui/src/theme/` (theme system)
**Runtime:** `crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (scoped script hydration, MutationObserver)
**Decisions:** 019 (scoped script & style tags), 020 (theme system)
**Constraint:** All CSS transformation and utility class generation happens at compile-time. Zero runtime CSS processing in WASM.

Two compile-time CSS systems: (1) scoped `<style>` tags transformed via `:parent` pseudo-class replacement using `lightningcss`, and (2) Tailwind-style utility classes generated from a `#[derive(ThemeTokens)]` struct. Scoped styles replace `:parent` with the parent element's identity and prepend unscoped selectors. Theme tokens generate a static CSS string of atomic utility classes injected into `<head>` as the first DOM operation batch.

---

## 1. Scoped Styles — Compile-Time Transformation

### 1.1 Input format

```html
<div id="menu-tabs">
  <style scoped primal:style>
    :parent { background: white; }
    :parent:hover { opacity: 0.8; }
    :parent .title { font-size: 20px; }
  </style>
</div>
```

The `html!` macro detects `<style>` elements with the `primal:style` attribute. The style tag is removed from the HTML output and its CSS content is processed at compile-time.

### 1.2 Transformation algorithm (lightningcss AST walk)

```
fn transform_scoped_css(css: &str, parent: &ParsedNode) -> String:
    1. parent_selector = resolve_parent_identity(parent)   // "#menu-tabs" or ".my-class"
    2. stylesheet = lightningcss::StyleSheet::parse(css, ParserOptions::default())
    3. for rule in stylesheet.rules:
        match rule:
            CssRule::Style(style_rule):
                for selector in &mut style_rule.selectors:
                    classify_and_transform(selector, parent_selector)
            CssRule::Media(media_rule):
                recurse into media_rule.rules         // same walk inside @media
            _: leave as-is                            // @keyframes, @font-face unchanged
    4. css_string = stylesheet.to_css(PrinterOptions { minify: true })
    5. return css_string
```

### 1.3 Selector classification and transformation

```
fn classify_and_transform(selector: &mut Selector, parent_sel: &str):
    components = selector.components()               // flat list of selector parts
    match classify(components, parent_sel):
        ParentPseudo:
            replace :parent with parent_sel           // ":parent:hover" -> "#menu-tabs:hover"
        AlreadyScoped:
            leave as-is                               // "#menu-tabs .foo" already targets parent
        NestedAmpersand:
            leave as-is                               // "&:hover" inherits scoping from parent rule
        CustomProperty:
            leave as-is                               // "--primary: blue" is not a selector
        Unscoped:
            prepend parent_sel + " "                  // ".title" -> "#menu-tabs .title"
```

---

## 2. `:parent` Resolution

### 2.1 Parent identity determination

```
fn resolve_parent_identity(parent: &ParsedNode) -> Result<String, CompileError>:
    1. if parent has "id" attribute:
        return format!("#{}", parent.id)              // "#menu-tabs"
    2. if parent has "class" attribute:
        first_class = parent.class.split_whitespace().next()
        return format!(".{}", first_class)            // ".sidebar"
    3. return Err(CompileError::NoParentIdentity {
        span: style_tag.span,
        message: "scoped <style primal:style> requires parent to have an id or class attribute"
    })
```

### 2.2 Priority

`id` always takes priority over `class`. When a parent has both `id="nav"` and `class="menu wide"`, the scoping selector is `#nav`. This avoids ambiguity — ids are unique, classes are not.

---

## 3. Smart Prefixing — Per-Rule Logic via AST Walk

### 3.1 The five rules demonstrated

Given parent `<div id="panel" class="sidebar">`:

**Input CSS:**
```css
:parent { background: white; }
:parent:hover { opacity: 0.8; }
#panel .title { font-weight: bold; }
.sidebar > p { margin: 0; }
& .nested { color: red; }
.unscoped-child { padding: 8px; }
--color-accent: #ff0;
h2 { font-size: 18px; }
```

**Output CSS after transformation:**
```css
/* Rule 1: :parent replaced with #panel */
#panel { background: white; }
#panel:hover { opacity: 0.8; }

/* Rule 2: already scoped — #panel and .sidebar match parent */
#panel .title { font-weight: bold; }
.sidebar > p { margin: 0; }

/* Rule 3: nested & inherits scoping from parent context */
& .nested { color: red; }

/* Rule 4: unscoped selectors prepended with parent id */
#panel .unscoped-child { padding: 8px; }
#panel h2 { font-size: 18px; }

/* Rule 5: custom properties left as-is */
--color-accent: #ff0;
```

### 3.2 Classification function

```
fn classify(components: &[Component], parent_sel: &str) -> SelectorKind:
    if components[0] == PseudoClass("parent"):
        return ParentPseudo
    if components[0] == Nesting:                     // the & token
        return NestedAmpersand
    if is_custom_property(components):
        return CustomProperty
    if first_simple_selector_matches(components, parent_sel):
        return AlreadyScoped                         // starts with #panel or .sidebar
    return Unscoped
```

`first_simple_selector_matches` checks whether the leading simple selector is the parent's id or any of the parent's classes.

---

## 4. Multiple Style Combining

Multiple `<style primal:style>` tags on the same parent are combined into a single CSS block at compile-time:

```html
<div id="menu-tabs">
  <style scoped primal:style>.title { font-size: 20px; }</style>
  <style scoped primal:style>.subtitle { color: gray; }</style>
</div>
```

**Combined output:**
```css
#menu-tabs .title { font-size: 20px; }
#menu-tabs .subtitle { color: gray; }
```

The macro collects all `primal:style` children, concatenates their CSS content, then runs the transformation once on the combined string. This produces one `<style>` element in the final DOM, reducing redundant style tags.

---

## 5. Scoped Script — `scope` Object API

### 5.1 Script format

```html
<div id="menu-tabs">
  <script scoped primal:script>
    function(scope) {
      let targets = scope.targets();
      scope.addEvent(targets[0], "click", () => { /* ... */ });
    }
  </script>
</div>
```

The JS runtime extracts `script.textContent`, creates a function via `new Function("scope", body)`, and invokes it with the scope object.

### 5.2 `scope` object methods

| Method | Returns | Description |
|--------|---------|-------------|
| `scope.targets()` | `Element[]` | Array of target elements. Default: `[parent]`. If `scoped="selector"`, elements matching selector within parent. |
| `scope.parent()` | `Element` | The parent DOM node containing the script tag. |
| `scope.querySelector(sel)` | `Element\|null` | Runs `parent.querySelector(sel)` — scoped to parent subtree. |
| `scope.addEvent(target, type, handler)` | `void` | Attaches event listener with auto-cleanup. Listener removed when element is disconnected. |

### 5.3 `primal` global helpers

Available inside scoped scripts:
- `primal.on(target, eventType, handler)` — event listener with auto-cleanup
- `primal.onclick(target, handler)` — shortcut for click
- `primal.onchange(target, handler)` — shortcut for change

### 5.4 Hydration lifecycle

Inside `<island>`: the island's `connectedCallback` executes scoped scripts.
Outside `<island>`: MutationObserver does NOT process scoped scripts. Scoped scripts outside islands are static — already transformed at compile-time, no runtime hydration needed for event wiring beyond `primal:on*` attribute scanning.

---

## 6. `#[derive(ThemeTokens)]` Macro — Input to Generated Output

### 6.1 Input struct

```rust
#[derive(ThemeTokens)]
struct AppTheme {
    colors: Colors {
        light: LightColors {
            primary: "#3b82f6",
            secondary: "#10b981",
            bg: "#ffffff",
        },
        // Option 1: omit entirely → all dark auto-generated from light
        // Option 2: Some { ... } with partial overrides
        dark: Some(DarkColors {
            primary: Some("#60a5fa"),   // explicit override
            secondary: None,            // auto-generate from light
            bg: Some("#1a1a2e"),        // explicit override
        }),
    },
    spacing: Spacing {     // no light/dark split — same for both
        xs: "4px",
        md: "16px",
    },
}
```

Three levels of control:
1. **No `dark` field** → all dark values auto-generated from light
2. **`dark: Some { ... }`** with some `None` → explicit where specified, auto-generate the rest
3. **`dark: Some { ... }`** with all `Some` → fully explicit

If a dark token is `None`, the macro auto-generates by inverting the light color:
- **Backgrounds** — lightened (dark mode needs lighter backgrounds)
- **Text/foreground** — darkened (light text on dark bg)
- **Non-color tokens** — no auto-generation, same value in both modes

### 6.2 Generated `Theme` struct

```rust
pub struct AppTheme { /* private fields */ }

impl AppTheme {
    pub fn new() -> Self { /* default token values from struct literal */ }
    pub fn css_string(&self) -> &'static str { /* pre-generated CSS */ }
}
```

### 6.3 Generated static CSS string

```css
/* Light theme (default) */
:root {
  --color-primary: #3b82f6;
  --color-bg: #ffffff;
  --spacing-xs: 4px;
}

/* Dark theme — auto-generated where None, explicit where Some */
@media (prefers-color-scheme: dark) {
  :root {
    --color-primary: #60a5fa;    /* explicit */
    --color-bg: #1a1a2e;         /* explicit */
    --color-secondary: #0d8f6b;  /* auto-inverted from #10b981 */
  }
}

/* Utility classes */
.bg-primary { background-color: var(--color-primary); }
.text-primary { color: var(--color-primary); }
.p-md { padding: var(--spacing-md); }
```

---

## 7. CSS Class Generation — Naming Patterns

### 7.1 Token-derived classes

Pattern: `{category_prefix}-{token_name}` referencing CSS custom properties.

| Category | Prefix | Token `primary` | Generated class | CSS rule |
|----------|--------|-----------------|-----------------|----------|
| Colors (bg) | `bg-` | `primary` | `.bg-primary` | `background-color: var(--color-primary)` |
| Colors (text) | `text-` | `white` | `.text-white` | `color: var(--color-white)` |
| Colors (border) | `border-` | `secondary` | `.border-secondary` | `border-color: var(--color-secondary)` |
| Spacing (padding) | `p-` | `md` | `.p-md` | `padding: var(--spacing-md)` |
| Spacing (margin) | `m-` | `lg` | `.m-lg` | `margin: var(--spacing-lg)` |
| Borders | `rounded-` | `sm` | `.rounded-sm` | `border-radius: var(--border-radius-sm)` |
| Shadows | `shadow-` | `md` | `.shadow-md` | `box-shadow: var(--shadow-md)` |

### 7.2 Built-in utility classes (static, not token-dependent)

| Category | Class name | CSS rule |
|----------|-----------|----------|
| Layout | `.relative` | `position: relative` |
| Layout | `.absolute` | `position: absolute` |
| Layout | `.fixed` | `position: fixed` |
| Layout | `.flex` | `display: flex` |
| Layout | `.grid` | `display: grid` |
| Layout | `.block` | `display: block` |
| Layout | `.inline` | `display: inline` |
| Sizing | `.w-full` | `width: 100%` |
| Sizing | `.h-screen` | `height: 100vh` |
| Sizing | `.max-w-md` | `max-width: 768px` |
| Typography | `.text-sm` | `font-size: 0.875rem` |
| Typography | `.text-lg` | `font-size: 1.125rem` |
| Typography | `.font-bold` | `font-weight: 700` |
| Typography | `.text-center` | `text-align: center` |
| Borders | `.border` | `border-width: 1px` |
| Borders | `.border-2` | `border-width: 2px` |
| Display | `.hidden` | `display: none` |
| Display | `.visible` | `visibility: visible` |
| Display | `.opacity-0` | `opacity: 0` |
| Display | `.opacity-100` | `opacity: 1` |

---

## 8. First-Batch Injection — DomOps Sequence

When WASM initializes, the theme CSS is sent as the **first** DOM operation batch, before any content. This guarantees styles are in `<head>` before the first element renders.

### 8.1 Exact DomOps generated

```rust
// Rust side: theme injection during Runtime::init()
let theme = AppTheme::new();
let css_content = theme.css_string();        // &'static str from derive macro

receiver.queue(DomOp::CreateElement {
    node_id: THEME_STYLE_NODE_ID,            // reserved node_id (e.g. 0)
    tag: "style".into(),
    class: "".into(),
});
receiver.queue(DomOp::SetAttribute {
    node_id: THEME_STYLE_NODE_ID,
    name: "id".into(),
    value: "primal-theme".into(),
});
receiver.queue(DomOp::SetText {
    node_id: THEME_STYLE_NODE_ID,
    text: css_content.into(),                // full CSS string
});
receiver.queue(DomOp::AppendChild {
    parent_id: HEAD_NODE_ID,                 // reserved node_id for <head>
    child_id: THEME_STYLE_NODE_ID,
});
```

### 8.2 JS side result

```html
<head>
  <style id="primal-theme">
    :root { --color-primary: #3b82f6; /* ... all custom properties */ }
    .bg-primary { background-color: var(--color-primary); }
    /* ... all utility classes */
  </style>
  <!-- subsequent content renders after this -->
</head>
```

### 8.3 Scoped style injection

Scoped styles from `primal:style` tags follow the same pattern but target `<head>` (not the parent element). Each scoped style block generates:

1. `CreateElement { tag: "style" }` with a generated node_id
2. `SetText` with the transformed, scoped CSS content
3. `AppendChild` to `<head>`

---

## 9. Light/Dark Theme — CSS Custom Properties

### 9.1 Light theme (default)

Custom properties are defined on `:root` from the `light` struct fields.

### 9.2 Dark theme via `prefers-color-scheme`

The derive macro generates a `@media` block. For each dark token:
- `Some(value)` → use explicit value
- `None` → auto-invert the corresponding light value

```rust
colors: Colors {
    light: LightColors {
        primary: "#3b82f6",
    },
    dark: DarkColors {
        primary: Some("#60a5fa"),  // explicit
        bg: None,                  // auto-invert from light bg
    },
}
```

### 9.3 Manual theme switching

Override via class on `<html>`:
```rust
receiver.queue(DomOp::AddClass { node_id: HTML_ROOT_ID, class: "dark".into() });
```

When `.dark` is present on `<html>`, a second CSS block applies:
```css
.dark { --color-primary: #60a5fa; --color-bg: #1a1a2e; }
```
This allows programmatic switching without relying on the media query.

---

## 10. Error Cases

| # | Condition | Behavior |
|---|-----------|----------|
| 1 | `primal:style` on parent with no id or class | Compile error: `"scoped <style primal:style> requires parent to have an id or class attribute"` |
| 2 | Invalid CSS inside `primal:style` | `lightningcss` parse error surfaced as compile error with span |
| 3 | `:parent` used outside `primal:style` | Ignored — `:parent` is only meaningful inside scoped style tags |
| 4 | Empty `primal:style` tag | No CSS generated, no `<style>` element emitted. Warning: `"empty scoped style tag"` |
| 5 | `ThemeTokens` struct with no categories | Compile error: `"ThemeTokens requires at least one token category"` |
| 6 | Duplicate token name across categories | Compile error: `"duplicate token name 'sm' in spacing and borders"` |
| 7 | Invalid CSS value in token (e.g., `primary: "not-a-color"`) | Accepted — values are opaque strings. Validation is CSS-level, not Rust-level. |
| 8 | Multiple `#[derive(ThemeTokens)]` structs | Each generates independent CSS. Only one should be registered via `ctx.set_theme()`. |

**G37 resolved — `lightningcss` dependency:** Use the `lightningcss` Rust crate
(`lightningcss = "1"` from npm's parcel-bundler/lightningcss). It compiles to WASM via napi and
is available as a pure-Rust crate. Used at compile-time only (macro processing), not shipped to
the browser. The CSS AST is parsed, transformed (`:parent` → parent selector), and serialized
back to a minified CSS string.

**G38 resolved — reserved node_id values:** `HEAD_NODE_ID = 0`, `BODY_NODE_ID = 1`,
`HTML_ROOT_ID = 2`. These are registered by the init batch before any component mounts.
Macro-assigned primal-ids start at `1000` (component prefix starts at 1000, template IDs start at 0
within each template, so the first runtime ID is `1000:0 = 10000` with the prefix formula).
No collision possible.

**G39 resolved — ThemeTokens dark naming:** Dark tokens use `Some(value)` for explicit overrides, `None` for auto-inversion. Struct groups by mode: `light: { ... }`, `dark: { ... }`. No naming convention needed.

---

## 11. Integration Points

| Feature | Interaction |
|---------|-------------|
| F01 (foundation_ui_traits) | `DomOp::CreateElement`, `SetText`, `SetAttribute`, `AppendChild` used for style injection |
| F03 (html! macro) | Macro detects `primal:style` and `primal:script` attributes, delegates to `scoped_css.rs` |
| F04 (InstructionReceiver) | Theme and scoped style DomOps queued via `receiver.queue()` |
| F06 (Web Components) | `<island>` connectedCallback executes scoped scripts within its subtree |
| F08 (Event Runtime) | MutationObserver handles `primal:on*` on dynamic content; does NOT handle scoped styles/scripts |
| F10 (Build Pipeline) | `lightningcss` is a compile-time dependency; not shipped to browser |

---

## 12. File Ownership

```
crates/foundation_wasm_ui/src/
├── html_macro/
│   ├── scoped_css.rs         # transform_scoped_css(), resolve_parent_identity(), classify_and_transform()
│   └── scoped_script.rs      # compile-time script tag extraction (textContent preservation)
├── theme/
│   ├── mod.rs                # re-exports
│   ├── derive.rs             # #[derive(ThemeTokens)] proc macro impl
│   ├── tokens.rs             # Token category structs (Colors, Spacing, Borders, Shadows)
│   ├── css_gen.rs            # generate_css_string(): token -> utility class CSS rules
│   ├── builtins.rs           # BUILTIN_UTILITIES: static CSS for layout, sizing, typography, display
│   └── dark.rs               # generate_dark_css(): auto-lightening, prefers-color-scheme block
├── assets/
│   └── foundation-wasm-ui.js # scope object creation, primal global, scoped script hydration
```

---

## 13. Refactoring Strategy

1. **Phase 1 — Scoped CSS transformer:** Implement `scoped_css.rs` with `lightningcss` parsing. Unit test all five selector classification rules against hardcoded CSS strings. No macro integration yet.
2. **Phase 2 — Parent resolution:** Implement `resolve_parent_identity()`. Test id-priority, class-fallback, and error-on-neither cases.
3. **Phase 3 — Macro integration:** Wire `scoped_css.rs` into the `html!` macro. When the macro encounters `<style primal:style>`, extract CSS, resolve parent, transform, and emit DomOps for style injection.
4. **Phase 4 — Multiple style combining:** Collect multiple `primal:style` children per parent, concatenate CSS, single transform pass.
5. **Phase 5 — ThemeTokens derive macro:** Implement `derive.rs` and `css_gen.rs`. Input struct parsed, CSS string generated at compile-time. Test generated CSS contains expected custom properties and utility classes.
6. **Phase 6 — Built-in utilities and dark theme:** Add `builtins.rs` (static utilities) and `dark.rs` (prefers-color-scheme generation). Combine with token CSS into single output string.
7. **Phase 7 — First-batch injection:** Wire theme CSS into `Runtime::init()`. Verify DomOps sequence: CreateElement, SetAttribute, SetText, AppendChild to head.
8. **Phase 8 — Scoped script hydration:** Implement scope object in JS runtime. Wire into `<island>` connectedCallback. Test scope.targets(), scope.parent(), scope.addEvent().

---

## 14. Dependencies

**Compile-time:** `lightningcss` (CSS parsing and transformation), `proc-macro2` + `quote` + `syn` (derive macro).
**Generated code:** `foundation_ui_traits` (DomOp, Html), `foundation_wasm_ui` (InstructionReceiver, Runtime).
**Runtime JS:** `foundation-wasm-ui.js` (scope object, primal global, island connectedCallback).

---

## 15. Testing

### Scoped CSS Transformation (tests 1-8)

| # | Input CSS | Parent | Expected output |
|---|-----------|--------|-----------------|
| 1 | `:parent { background: white; }` | `<div id="menu-tabs">` | `#menu-tabs { background: white; }` |
| 2 | `:parent:hover { opacity: 0.8; }` | `<div id="menu-tabs">` | `#menu-tabs:hover { opacity: 0.8; }` |
| 3 | `:parent .title { font-size: 20px; }` | `<div id="menu-tabs">` | `#menu-tabs .title { font-size: 20px; }` |
| 4 | `.title { color: red; }` (unscoped) | `<div id="panel">` | `#panel .title { color: red; }` |
| 5 | `#panel .title { font-weight: bold; }` | `<div id="panel">` | `#panel .title { font-weight: bold; }` (unchanged) |
| 6 | `& .nested { color: red; }` | `<div id="panel">` | `& .nested { color: red; }` (unchanged) |
| 7 | `--accent: #ff0;` (custom property) | `<div id="panel">` | `--accent: #ff0;` (unchanged) |
| 8 | `h2 { font-size: 18px; }` (bare element) | `<div class="card">` | `.card h2 { font-size: 18px; }` |

### Parent Resolution (tests 9-12)

| # | Parent element | Resolved selector | Notes |
|---|---------------|-------------------|-------|
| 9 | `<div id="nav">` | `#nav` | id takes priority |
| 10 | `<div class="sidebar wide">` | `.sidebar` | first class used |
| 11 | `<div id="nav" class="menu">` | `#nav` | id over class |
| 12 | `<div>` (no id, no class) | Compile error | `"requires parent to have an id or class"` |

### Multiple Style Combining (tests 13-14)

| # | Scenario | Verify |
|---|----------|--------|
| 13 | Two `primal:style` on same parent | Combined into single CSS block, both rules scoped |
| 14 | Three `primal:style` with overlapping selectors | All three combined, scoping applied once to concatenated CSS |

### Scoped Script (tests 15-19)

| # | Scenario | Verify |
|---|----------|--------|
| 15 | `scope.targets()` default | Returns `[parent]` as single-element array |
| 16 | `scope.parent()` | Returns the DOM element containing the script tag |
| 17 | `scope.querySelector(".title")` | Scoped to parent subtree, returns matching child |
| 18 | `scope.addEvent(el, "click", fn)` | Listener fires on click; removed on disconnect |
| 19 | Script inside `<island>` | Executed by island's connectedCallback, not MutationObserver |

### ThemeTokens Derive Macro (tests 20-24)

| # | Scenario | Verify |
|---|----------|--------|
| 20 | Basic `AppTheme::new()` | Returns Theme with default token values |
| 21 | `css_string()` contains `:root` block | All custom properties present: `--color-primary`, `--spacing-sm`, etc. |
| 22 | `css_string()` contains token-derived classes | `.bg-primary`, `.text-white`, `.p-md`, `.m-lg`, `.rounded-sm`, `.shadow-md` present |
| 23 | `css_string()` contains built-in utilities | `.flex`, `.grid`, `.hidden`, `.w-full`, `.font-bold`, `.text-center` present |
| 24 | `dark_css_string()` contains media query | `@media (prefers-color-scheme: dark)` block with overridden custom properties |

### CSS Class Generation (tests 25-30)

| # | Token category | Token name | Expected class + rule |
|---|---------------|------------|----------------------|
| 25 | colors | `primary` | `.bg-primary { background-color: var(--color-primary); }` |
| 26 | colors | `white` | `.text-white { color: var(--color-white); }` |
| 27 | spacing | `md` | `.p-md { padding: var(--spacing-md); }` and `.m-md { margin: var(--spacing-md); }` |
| 28 | borders | `lg` | `.rounded-lg { border-radius: var(--border-radius-lg); }` |
| 29 | shadows | `sm` | `.shadow-sm { box-shadow: var(--shadow-sm); }` |
| 30 | colors | `secondary` | `.border-secondary { border-color: var(--color-secondary); }` |

### First-Batch Injection (tests 31-34)

| # | Scenario | Verify |
|---|----------|--------|
| 31 | Runtime init with theme | First DomOp batch contains `CreateElement { tag: "style" }` |
| 32 | Style element attributes | `SetAttribute { name: "id", value: "primal-theme" }` present in batch |
| 33 | CSS content | `SetText` op contains full CSS string from `css_string()` |
| 34 | Append to head | `AppendChild { parent_id: HEAD_NODE_ID, child_id: THEME_STYLE_NODE_ID }` is last op |

### Light/Dark Theme (tests 35-38)

| # | Scenario | Verify |
|---|----------|--------|
| 35 | Default light theme | `:root` block has original token values |
| 36 | `prefers-color-scheme: dark` | `@media` block overrides `--color-primary` with lighter variant |
| 37 | Explicit `_dark` override | `primary_dark: "#60a5fa"` used instead of auto-generated value |
| 38 | Manual `.dark` class switch | `AddClass { class: "dark" }` on html root activates dark properties |

### Error Cases (tests 39-43)

| # | Scenario | Expected |
|---|----------|----------|
| 39 | `primal:style` on `<div>` with no id/class | Compile error with span pointing to style tag |
| 40 | Invalid CSS in scoped style | `lightningcss` parse error surfaced as compile error |
| 41 | Empty `primal:style` tag | Warning emitted, no style element generated |
| 42 | ThemeTokens struct with zero categories | Compile error: `"requires at least one token category"` |
| 43 | Duplicate token name across categories | Compile error naming the duplicate and both categories |

### Integration (tests 44-47)

| # | Scenario | Verify |
|---|----------|--------|
| 44 | html! with `primal:style` + theme classes | Scoped CSS and theme utilities both present in output |
| 45 | Theme injection before content | Theme style DomOps precede any element creation DomOps |
| 46 | Scoped style in `<island>` | Style processed at compile-time, applied via connectedCallback |
| 47 | End-to-end: signal change re-renders element with theme class | Element retains utility class, theme CSS still in `<head>` |
