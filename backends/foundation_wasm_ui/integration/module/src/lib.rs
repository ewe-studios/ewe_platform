//! Minimal wasm32 module exercising `foundation_wasm_ui`'s DOM host FFI end-to-end
//! against the real JS runtimes (`foundation-wasm.js` + `foundation-wasm-ui.js`'s
//! `domAbi` import fragment): DOM handle pre-allocation, the naked `as_dom`
//! invocation fast-path, and DOM handle retirement.
//!
//! `std` crate (default allocator + panic handler); the library crates stay `no_std`.

use foundation_wasm_ui::ops;
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

std::thread_local! {
    static RELEASES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static SCOPED_APP: std::cell::RefCell<Option<foundation_wasm_ui::App>> = const { std::cell::RefCell::new(None) };
}

/// Mount one local scope through the same Rust receiver and event bridge as a browser app.
#[no_mangle]
pub extern "C" fn mount_scoped_button() {
    let app = foundation_wasm_ui::App::new();
    foundation_wasm_ui::install_app_bridge(&app);
    app.ctx().on_dispose(|| RELEASES.set(RELEASES.get() + 1));
    app.receiver().register_scope(&[77], app.ctx());
    let receiver = app.receiver();
    receiver.queue(ops::create_element(77, &foundation_ui_traits::HtmlTag::from_static("button"), ""));
    receiver.queue(ops::register_node(77));
    receiver.queue(ops::append_child(1, 77));
    app.stabilize().expect("deliver local button");
    SCOPED_APP.replace(Some(app));
}

#[no_mangle]
pub extern "C" fn local_scope_releases() -> u32 {
    RELEASES.get()
}

/// Render the same button with either public transport.
fn emit_parity(app: foundation_wasm_ui::App) {
    let receiver = app.receiver();
    let button = foundation_ui_traits::HtmlTag::from_static("button");
    receiver.queue(ops::create_element(88, &button, "parity"));
    receiver.queue(ops::register_node(88));
    receiver.queue(ops::set_text(88, "rendered"));
    receiver.queue(ops::append_child(foundation_wasm_ui::BODY_NODE_ID,88));
    app.stabilize().expect("deliver parity button");
}

#[no_mangle]
pub extern "C" fn emit_parity_frame() { emit_parity(foundation_wasm_ui::App::new()); }
