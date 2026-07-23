//! App Shell — Surface 3 WASM module (wasmtime, F33).
//!
//! Compiles to wasm32-wasip1. The host (foundation_wasmtime) imports
//! `ewe::log` and calls exported functions directly in-process.
//!
//! Bundled in `public/app-shell/v{version}/` alongside every other app (F40)
//! and loaded at runtime through the asset manager — so an OTA can replace
//! the shell without a native rebuild or a store release.

use foundation_macros::wasm_app;
use std::io::Write;

/// The host calls this at startup. Writes a greeting to stdout (WASI).
/// `extern = "true"` tells the macro to emit `#[no_mangle] pub extern "C"`.
#[wasm_app(extern = "true")]
fn init() {
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
