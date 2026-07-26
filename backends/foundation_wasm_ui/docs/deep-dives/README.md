# Deep Dives — foundation_wasm_ui

## Reactive Architecture

```
Signal<T>                     Computed<T>
  │                              │
  ├─ value: T                    ├─ derive fn: () -> T
  ├─ subscribers: Vec<Callback>  ├─ dependencies: Vec<Signal>
  │                              │
  ├─ get() → &T                  ├─ get() → T (recomputed if dirty)
  ├─ set(new: T)                 │
  │   └─ notify subscribers      │
  ├─ update(fn)                  │
  │   └─ set(fn(&mut T))         │
  └─ subscribe(cb)               └─ (auto-subscribes to deps)
```

`Signal::set()` triggers a cascade: signal → subscribers → DOM updates. `Computed` tracks which signals it reads during derivation and auto-resubscribes when those signals change.

## JavaScript bridge

The WASM runtime connects to JavaScript through a shared `Runtime` handle:

```javascript
// foundation-wasm-ui.js (bundled)
const runtime = {
    memory: wasmInstance.exports.memory,
    bridge: wasmInstance,
    signals: new Map(),          // signal_id → WebAssembly callback
    eventDispatcher: {},         // DOM event → signal routing
};
```

`signalDeliver(runtime)` writes event data to WASM memory and calls `runtime.bridge.exports.invoke_signal_callback(slot_id)`, which Rust decodes and dispatches to the correct subscriber.

## html! macro internals

The `html!` macro expands at compile time into `HtmlNode` construction:

```rust
html! { <div class="foo">{text}</div> }
// expands roughly to:
HtmlNode::element("div")
    .attr("class", "foo")
    .child(HtmlNode::text(text.get()))
```

Reactive attributes (`{signal}` in attribute position) create subscriptions that update the DOM when the signal changes — no virtual DOM diffing needed.

## EventDispatcher → signal bridge

```
1. User clicks button in WebView
2. DOM `onclick` fires
3. `EventDispatcher.buildListener("my_handler")` generates:
   → javascript:signalDeliver(runtime, handlerRef, eventData)
4. signalDeliver writes EventData to WASM linear memory
5. Calls runtime.bridge.exports.invoke_signal_callback(handlerRef)
6. Rust `SignalCallback::invoke(handler_ref)` looks up the handler
7. Calls the registered Rust closure
8. Closure updates a Signal (e.g. count.set(count.get() + 1))
9. Signal notifies subscribers → DOM re-renders affected text nodes
```

## PlatformSchemeInterceptor

`embedded::PLATFORM_SCHEME_INTERCEPTOR_JS` is injected into every page by `build_tools/mod.rs`:

```javascript
// Intercepts clicks on ewe://localhost/... links
document.addEventListener('click', function(e) {
    var link = e.target.closest('a[href^="ewe://"]');
    if (link) {
        e.preventDefault();
        // Route through Tauri IPC instead of browser navigation
        window.__TAURI_INTERNALS__.invoke('plugin:ewe-platform|navigate', {
            url: link.href
        });
    }
});
```

This ensures all `ewe://` links go through the platform's route engine rather than the system browser.
