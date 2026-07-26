# Getting Started — foundation_wasm_ui

WASM-native UI framework — `html!` macro, reactive signals, DOM operations, and the WASM bundle build pipeline.

## Key modules

| Module | Purpose |
|---|---|
| `html_macro` | `html!` DSL — compile-time HTML templating macro |
| `reactive` | Reactive signals, computed values, effects |
| `events` | `EventDispatcher` — DOM event → signal/callback wiring |
| `runtime` | WASM runtime bootstrapping, JS bridge initialization |
| `build_tools` | `WasmBundleGenerator` — build-time WASM bundling |
| `app` | `#[wasm_app]` proc macro — WASM app scaffolding |
| `wasm/dom` | DOM manipulation primitives (create element, set attribute, etc.) |

## html! macro

```rust
use foundation_wasm_ui::html;

let node = html! {
    <div class="container">
        <h1>"Hello, " {name} "!"</h1>
        <button onclick={move |_| count.update(|c| *c + 1)}>
            "Clicked " {count} " times"
        </button>
    </div>
};
```

**Rules:**
- `{expr}` slots are **text-only** — renders as `textContent`
- For element/`Html` values, use `<Fragment>{expr}</Fragment>`
- `primal:` prefix is reserved (use `data-*` for machinery inputs)
- Reactive attributes get one getter-clone each

## Reactive signals

```rust
use foundation_wasm_ui::reactive::{Signal, Computed};

let count = Signal::new(0);
let doubled = Computed::new(move || *count.get() * 2);

// Read
let current = count.get();

// Write
count.set(5);
count.update(|c| *c += 1);

// Subscribe
count.subscribe(move |new_val| {
    log::info!("count changed to {new_val}");
});
```

## EventDispatcher

Wires DOM events to reactive signal callbacks:

```rust
let dispatcher = EventDispatcher::new(runtime);

// Register a callback
dispatcher.register("my_button_click", move |_event_data| {
    // handle the event
});

// In HTML:
// <button onclick={dispatcher.build_listener("my_button_click")}>Click</button>
```

On Android WebView, `EventDispatcher.deliver` calls `signalDeliver(runtime)` which writes to the WASM linear memory and invokes the signal callback via the JS bridge.

## WASM Bundle Build Pipeline

```rust
// build.rs
use foundation_wasm_ui::build_tools::WasmBundleGenerator;

fn main() {
    WasmBundleGenerator::new()
        .with_app_name("platform_dashboard")
        .with_version("0.1.0")
        .generate()
        .expect("WASM bundle generation failed");
}
```

Produces:
```
public/app/v0.1.0/
  ├── platform_dashboard.wasm
  ├── platform_dashboard.js
  ├── bundle.js
  └── index.html
```

## #[wasm_app] macro

Scaffolds a WASM app entry point:

```rust
#[wasm_app]
pub fn init() {
    // app initialization — DOM setup, signal wiring, IPC registration
}
```

Expands to: WASM memory setup, JS bridge initialization, WASI host runtime boot, and `#[wasm_bindgen(start)]` entry point.
