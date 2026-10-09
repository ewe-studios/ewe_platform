//! App Shell — Surface 3 WASM module (wasmtime).
//!
//! Compiles to wasm32-wasip1. The host (foundation_wasmtime) imports
//! `ewe::log` and calls exported functions directly in-process.
//!
//! This is a minimal demo. In production, a #[wasm_app] crate would
//! handle ewe:// routes, call session APIs through imports, and return
//! responses through WASM linear memory.

// Marked with #[wasm_app] so codegen discovers it.
// (In a real app this would be a proc macro; for now it's a comment
//  that `build_wasmtime_app` scans for via the `WasmApp` AnnotationKind.)

use std::io::Write;

/// The host calls this at startup. Writes a greeting to stdout (WASI).
#[no_mangle]
pub extern "C" fn init() {
    println!("[app-shell] Surface 3: wasmtime shell initialized");
    std::io::stdout().flush().ok();
}

/// Handle a request. Returns the response length in bytes via pointer.
/// The host reads the response from WASM memory at `response_ptr`.
#[no_mangle]
pub extern "C" fn handle_request(route_ptr: *const u8, route_len: u32) -> u32 {
    let route = unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(route_ptr, route_len as usize))
            .unwrap_or("/")
    };
    let response = format!("wasmtime shell handled: {route}");
    response.len() as u32
}
