//! Minimal wasm32 module exercising `foundation_wasm_ui`'s DOM host FFI end-to-end
//! against the real JS runtimes (`foundation-wasm.js` + `foundation-wasm-ui.js`'s
//! `domAbi` import fragment): DOM handle pre-allocation, the naked `as_dom`
//! invocation fast-path, and DOM handle retirement.
//!
//! `std` crate (default allocator + panic handler); the library crates stay `no_std`.

use foundation_wasm::abi::web::register_function;
use foundation_wasm::ExternalPointer;
use foundation_wasm_ui::wasm::dom::{allocate_dom_reference, drop_dom_reference, DomInvoke};

/// Pre-allocate a DOM-heap slot (`dom_allocate_external_pointer`) → its handle.
/// Slots 0–4 are reserved (self/heap/window/document/body), so the first
/// pre-allocation returns index 5.
#[no_mangle]
pub extern "C" fn dom_allocate() -> i64 {
    allocate_dom_reference().clone_inner() as i64
}

/// NAKED dom fast-path: `invoke_for_dom` rides `host_invoke_function_as_dom` — the
/// JS fn's returned node interns into the DOM heap and the HANDLE crosses raw.
/// The test puts the node on the registry context as `this.testNode`.
#[no_mangle]
pub extern "C" fn dom_invoke_for_dom() -> i64 {
    let f = register_function("function(){ return this.testNode; }");
    f.invoke_for_dom(&[]).clone_inner() as i64
}

/// Retires a DOM-heap handle (`host_dom_drop_external_pointer` round-trip).
#[no_mangle]
pub extern "C" fn dom_drop(handle: u64) {
    drop_dom_reference(ExternalPointer::pointer(handle));
}
