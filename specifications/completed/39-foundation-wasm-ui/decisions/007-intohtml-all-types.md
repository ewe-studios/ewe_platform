# 007 — IntoHtml trait: implement for all common types

**Date:** 2026-06-08
**Status:** Resolved

### Decision

`IntoHtml` trait implemented for all common types so `{expr}` in `html!` works without manual conversion:

### Core implementations
- `Html` — identity (no-op)
- `Vec<Html>` — flattened into children
- `impl Iterator<Item = Html>` — collected into children
- `Option<Html>` — `Some` = child, `None` = nothing
- `&str`, `String`, `&Html` — text content

### Primitives (Display → text)
- `usize`, `isize`, `u8`-`u64`, `i8`-`i64`
- `bool` — renders as-is (`true`/`false`)
- `f32`, `f64`

### Crate structure

`IntoHtml` and `Html` live in **`foundation_ui_traits`** — a shared crate that both `foundation_signals` and `foundation_wasm_ui` depend on (decision 012).

### Signal implementation
- `&Signal<T: IntoHtml + Clone>` — reads signal value, returns `IntoHtml::into(value)`
- **Implementation lives in `foundation_signals`** (which depends on `foundation_ui_traits`):
  ```rust
  // foundation_signals:
  impl<T: IntoHtml + Clone> IntoHtml for &Signal<T> {
      fn into_html(&self) -> Html {
          self.get().into_html()
      }
  }
  ```

### Why this works
- No boilerplate for users — `{count}` just works
- `Option<Html>` enables conditional rendering: `{if condition { Some(html! {...}) } else { None }}`
- `Signal<T>` auto-converts — `{my_signal}` reads and renders without explicit `.get()`
