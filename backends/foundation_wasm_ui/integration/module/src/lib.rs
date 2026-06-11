//! Minimal wasm32 module exercising `foundation_wasm_ui`'s DOM host FFI end-to-end
//! against the real JS runtimes (`foundation-wasm.js` + `foundation-wasm-ui.js`'s
//! `domAbi` import fragment): DOM handle pre-allocation, the naked `as_dom`
//! invocation fast-path, and DOM handle retirement.
//!
//! `std` crate (default allocator + panic handler); the library crates stay `no_std`.

use foundation_ui_traits::DomOp;
use foundation_wasm::abi::web::register_function;
use foundation_wasm::ExternalPointer;
use foundation_wasm_ui::wasm::dom::{allocate_dom_reference, drop_dom_reference, DomInvoke};
use foundation_wasm_ui::{BatchInstructionsV1, InstructionReceiver};

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

/// Ship a DomOp batch over the CUSTOM BINARY protocol (byte 0 = the batch
/// instructions format, decision 022) through the LIVE LOOP: a global-arena
/// InstructionReceiver (decision 030) flushes once, BatchInstructionsV1 packs the
/// Operations stream + texts pool into one envelope slot, and host_apply ships it —
/// the JS dispatcher routes byte 0 into the BatchInstructions runtime, where the
/// registered BATCH_OP_APPLY_DOM operation applies each op to the DOM.
#[no_mangle]
pub extern "C" fn emit_batch_dom_ops() {
    let mut receiver = InstructionReceiver::with_global_arena(Box::new(BatchInstructionsV1::new()));
    receiver.queue(DomOp::SetText {
        node_id: 5,
        text: "batch hi".into(),
    });
    receiver.queue(DomOp::RemoveNode { node_id: 6 });
    receiver.flush();
}
