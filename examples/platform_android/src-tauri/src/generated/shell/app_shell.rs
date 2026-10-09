// Generated — wasmtime shell for "app_shell" (F33, Surface 3)
// Route prefix: /shell/
//
// The compiled WASM binary lives in shell_wasm/app-shell.wasm.
// Loads via include_bytes! and wraps in a WasmtimeBuilder.

use foundation_wasmtime::WasmtimeBuilder;

#[must_use]
pub fn builder() -> WasmtimeBuilder {
    let wasm_bytes: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shell_wasm/app-shell.wasm"
    ));
    WasmtimeBuilder::new(wasm_bytes).with_name("app_shell")
}
