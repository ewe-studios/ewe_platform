# Shipping — features, bundling, testing

## Cargo features

| feature | default | effect |
|---------|---------|--------|
| `arrow` | **on** | `ArrowIpcV2` / `App::arrow()` — real Apache Arrow IPC (wire v2) |
| `embedded-js` | off | `embedded` module: runtime JS (+ `apache-arrow.js`) as `&str` consts |

The crate is `#![no_std]` on wasm32. Build tooling (`build_tools`, `cli`, the
`ewe-wasm-bundle` binary) and the `server` module are **target-gated, not
feature-gated**: every native build carries them; wasm32 builds never see them.

## Building a wasm bundle

Annotate entrypoints, then let the build pipeline do discovery → compile →
wrappers → bundles:

```rust
#[wasm_bin]      pub fn app()   { /* App wiring */ }   // main-thread app
#[wasm_worker]   pub fn heavy() { /* … */ }            // web-worker entrypoint
#[wasm_service]  pub fn api()   { /* … */ }            // service-style entrypoint
```

```bash
# This crate's own CLI (built on every native build):
ewe-wasm-bundle plan  --target path/to/your-crate
ewe-wasm-bundle build --target path/to/your-crate --release --output dist
```

The output has the `.wasm`, a JS wrapper per entrypoint (with a
`__EWE_WASM_LOAD__` seam), optional single-file bundles (wasm as base64), and
runtime assets.

## Serving without a bundler

The `embedded-js` feature embeds the runtime assets in your server binary as
`&'static str` consts — serve them with any in-memory handler (e.g.
`foundation_http`'s `StaticAssetHandler`):

```rust
# #[cfg(feature = "embedded-js")]
use foundation_wasm_ui::embedded::{FOUNDATION_WASM_UI_JS, APACHE_ARROW_JS, FOUNDATION_WASM_JS};
```

## Testing

```bash
# Rust (the dev profile uses Cranelift — test with uat):
cargo test --profile uat -p foundation_wasm_ui
# The JS runtime suite:
node --test backends/foundation_wasm_ui/integration/test/
```

`App::mock()` makes the loop assertable without a browser (see
[internals](./internals.md#testing-the-loop)); `foundation_browser` drives a real
Chromium (see [server & protocols](./server-and-protocols.md#live-browser-testing)).

## Current limits

- **No `Component` lifecycle trait** — "it's all just functions". Composition is
  `Render`/`Slot` + `<Fragment>`; structural reactivity is `<Show>`/`<For>`.
- Pure-form `{slot}` values are evaluated once — values, not bindings.
- The signal runtime is single-threaded by design.
- `<For>`'s reorder path is correct but may over-move (LIS minimal-move pass is
  future work).

Design history and per-feature verification live in
[`specifications/39-foundation-wasm-ui/`](../../../specifications/39-foundation-wasm-ui/)
and [`specifications/42-ui-component/`](../../../specifications/42-ui-component/).
