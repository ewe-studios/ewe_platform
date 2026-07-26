# Adding Native Capabilities — foundation_wasm_ui

The UI framework doesn't add native capabilities directly — that's the role of `foundation_platform_native`. This crate provides the WASM-side infrastructure for calling those capabilities.

## Pattern: calling a native capability from html! buttons

```rust
html! {
    <button onclick={move |_| {
        spawn_local(async move {
            let result = Modal::present(PresentArgs { /* ... */ }).await;
            // Update reactive state based on result
        });
    }}>
        "Present Modal"
    </button>
}
```

For details on creating new native capabilities (Kotlin/Swift helpers, Rust IPC handlers, WASM wrappers), see [`foundation_platform_native/docs/adding_native_capabilities/`](../../foundation_platform_native/docs/adding_native_capabilities/).
