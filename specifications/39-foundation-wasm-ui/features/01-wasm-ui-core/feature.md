# Feature 01: WASM UI Core — Crate Split

## Description

Create the `foundation_wasm_ui` crate and perform a clean split from `foundation_wasm`. 

**foundation_wasm** becomes the pure runtime/ABI — memory management, binary message system, handle/pointer allocation, function calling API, generic timers. All DOM/window/animation concepts are removed.

**foundation_wasm_ui** is created as a new crate that owns ALL DOM/window/animation bindings. It depends on foundation_wasm and adds: signals, templates, components, Arrow-format DOM batching, web component bridge.

## Changes to foundation_wasm

### Files removed/moved:
- `src/frames.rs` → moves to `foundation_wasm_ui/src/wasm/animation.rs`
- `src/jsapi.rs` DOM constants (`DOM_SELF`, `DOM_THIS`, `DOM_WINDOW`, `DOM_DOCUMENT`, `DOM_BODY`) → moves to `foundation_wasm_ui/src/wasm/dom/constants.rs`
- `src/jsapi.rs` `allocate_dom_reference()` → moves to `foundation_wasm_ui/src/wasm/dom/element.rs`
- `src/jsapi.rs` `invoke_for_dom()`, `invoke_for_object()` → moves to `foundation_wasm_ui/src/wasm/dom/element.rs`
- `src/jsapi.rs` `ReturnTypeId::DOMObject` variant → moves to `foundation_wasm_ui/src/wasm/dom/types.rs`
- `src/jsapi.rs` `host_cache_string` wrapper → moves to `foundation_wasm_ui/src/wasm/text_cache.rs`
- `runtime/foundation-wasm.js` → rewritten as `foundation-wasm.js` (standard comm only)

### Files unchanged (stay in foundation_wasm):
- `src/base.rs` — ReturnTypeId, ReturnTypeHints, ReturnValues
- `src/error.rs` — Error types
- `src/intervals.rs` — Timer registry (generic, not DOM-specific)
- `src/mem.rs` — MemoryAllocations, MemoryId
- `src/ops.rs` — Params, Instructions, batch encoding
- `src/registry.rs` — Callback/pointer registry
- `src/schedule.rs` — Schedule registry
- `src/wrapped.rs` — Wrappers

### Cargo.toml changes:
- Update `description` to: "Low-level WASM-JS ABI for memory management, function invocation, and binary messaging. No DOM, no window, no UI concepts."

## New foundation_wasm_ui Crate

### Cargo.toml:
```toml
[package]
name = "foundation_wasm_ui"
version = "0.0.1"
edition.workspace = true

[dependencies]
foundation_wasm = { workspace = true }
foundation_nostd = { workspace = true }
foundation_macros = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[features]
default = []
signals = []
templates = ["signals"]
components = ["signals", "templates"]
arrow = []
full = ["signals", "templates", "components", "arrow"]
```

### Initial Module Structure:
```
src/
├── lib.rs                # Feature-gated re-exports
├── shared/
│   ├── mod.rs
│   ├── signal.rs         # (Feature 02 — stub for now)
│   ├── template.rs       # (Feature 03 — stub for now)
│   ├── component.rs      # Component trait, ComponentHost
│   ├── controller.rs     # (later)
│   ├── directive.rs      # (later)
│   └── arrow/            # (Feature 04 — stub for now)
└── wasm/
    ├── mod.rs
    ├── dom/
    │   ├── constants.rs  # DOM_SELF, DOM_WINDOW, etc. (moved)
    │   ├── element.rs    # DOM operations (moved)
    │   └── event.rs      # Event listeners (moved)
    ├── animation.rs      # Animation frames (moved)
    ├── text_cache.rs     # String interning (moved)
    ├── dom_bridge.rs     # (later)
    └── custom_elements.rs # (later)
```

## JS SDK Split

### foundation-wasm.js (rewritten from existing runtime)
- `MemoryAllocations` class — create_allocation, dispose_allocation, get, clear
- `Instructions` encoder/decoder
- `FunctionRegistry` — register_function, invoke_as_*, invoke_async
- `CallbackRegistry` — register_callback, invoke_callback, unregister_callback
- `TimerRegistry` — schedule_timeout, schedule_interval, hook_up_animation_frames
- `Batch API` — host_batch_apply, host_batch_returning_apply
- Rewritten for clarity — clean standard communication only

### foundation-wasm-ui.js (new)
- `ArrowParser` — TypedArray column views from ArrayBuffer
- `ArrowDomApplicator` — apply(buffer) → real DOM, 15 operation types
- `NodeRegistry` — nodeId → DOM Element
- `SignalBridge` — bindInput(), bindChange()
- `ComponentRegistry` — register(tagName, ...), WasmElement class
- `EventDispatcher` — user actions → WASM
- `SSEClient` — server → DOM patches

## Dependencies

- No new dependencies for foundation_wasm (removes some)
- foundation_wasm_ui adds: foundation_wasm, foundation_nostd, foundation_macros, serde, serde_json

## Testing

- foundation_wasm compiles without DOM types
- foundation_wasm_ui compiles, depends on foundation_wasm
- No circular dependencies
- foundation-wasm.js loads, provides memory/batch/invoke APIs
- foundation-wasm-ui.js loads, depends on foundation-wasm.js
