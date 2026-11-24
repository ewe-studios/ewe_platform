---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F31-platform-ui-components"
this_file: "specifications/52-tauri-foundation-platform/features/F31-platform-ui-components/feature.md"

status: completed
priority: high
created: 2026-07-21

depends_on:
  - "F29-platform-completeness"
  - "F24-script-injector"

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# F31 — Platform UI components in foundation_ui_components

## Problem

Remote pages rendered inside iframes and platform navigation toolbars were
hand-rolled as raw HTML strings in `responder.rs` and `floating-nav.js`. Every
route handler that renders remote content had to duplicate the wrapper HTML,
the sandboxed iframe boilerplate, the toolbar, and the scheme interceptor
script. Raw JS was fragile (no `DOMContentLoaded` guard, no scoped behavior).

## Solution

Two new headless UI components in `foundation_ui_components` that follow the
existing pattern: config struct + pure render function returning `Html`,
scoped JS behavior via `scoped_script()`.

### 1. `remote_page` — iframe wrapper with toolbar

Replaces the hand-rolled `wrap_remote()` in `responder.rs::RemoteProxy`.

```rust
pub struct RemotePageConfig {
    pub title: Cow<'static, str>,
    pub content: String,
    pub class: Cow<'static, str>,
    pub show_back: bool,
    pub show_home: bool,
    pub show_refresh: bool,
    pub home_route: Cow<'static, str>,
}

pub fn remote_page(config: RemotePageConfig) -> Html { ... }
```

Features:
- White-background wrapper, full-viewport iframe
- Toolbar with back/home/refresh buttons (data-attribute-driven)
- Sandboxed iframe with `srcdoc` for the remote content
- Scheme interceptor injected as scoped JS — catches `ewe://` clicks
- Single-pass HTML escaping for srcdoc safety
- No signals — pure component

### 2. `floating_nav` — persistent navigation toolbar

Replaces the hand-rolled `floating-nav.js` IIFE.

```rust
pub struct FloatingNavConfig {
    pub position: Cow<'static, str>,     // "top" (default) or "bottom"
    pub title: Cow<'static, str>,
    pub home_route: Cow<'static, str>,   // default "/app/"
    pub apps_route: Cow<'static, str>,   // default "/"
    pub visible: bool,
    pub class: Cow<'static, str>,
}

pub fn floating_nav(config: FloatingNavConfig) -> Html { ... }
```

Features:
- Data-attribute-driven buttons: `data-fn-button="back|home|refresh|apps"`
- Scoped JS wires click handlers → history.back() / location.href / location.reload()
- Custom events (`ewe:nav:back`, `ewe:nav:refresh`) for platform shell interception
- Scroll-based auto-hide (hides on scroll down, shows on scroll up)
- Public API via `window.__eweNav.setTitle/show/hide`
- Wrapped in a `<div>` for single-root html! compliance

## Requirements

### 1. `remote_page` component
- File: `backends/foundation_ui_components/src/remote_page.rs` (NEW)
- Registered in `lib.rs` with pub export
- `RemotePageConfig` with defaults (title="Remote", class="remote-page", all buttons shown)
- Scoped JS wiring for back/refresh buttons + scheme interceptor
- Single SRCDOC-safe HTML escaping

### 2. `floating_nav` component
- File: `backends/foundation_ui_components/src/floating_nav.rs` (NEW)
- Registered in `lib.rs` with pub export
- `FloatingNavConfig` with defaults (position="top", visible=true)
- `<div data-fn-wrapper>` wrapper for single-root compliance
- `__eweNav` global API for runtime title/show/hide

### 3. No signals
- Both components are pure — no `&Context` or `&SharedInstructionReceiver`
- Consumer CSS targets `data-*` attributes for styling
- Zero new dependencies beyond `foundation_wasm_ui` + `foundation_signals` (already deps)

### 4. Backward compatible
- The existing `RemoteProxy` in `responder.rs` can migrate to use `remote_page`
- The existing `floating-nav.js` runtime can be replaced by `floating_nav` component
- No breaking changes to the platform API

## Verification

```bash
# Compile check
cargo check -p foundation_ui_components

# Verify components exist in the module tree
cargo doc -p foundation_ui_components --no-deps

# Integration: RemoteProxy uses remote_page (future migration)
# Integration: ScriptInjector injects floating_nav on every page (future migration)
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_ui_components/src/remote_page.rs` | **NEW** — `remote_page` component |
| `backends/foundation_ui_components/src/floating_nav.rs` | **NEW** — `floating_nav` component |
| `backends/foundation_ui_components/src/lib.rs` | Add module declarations + pub exports |
