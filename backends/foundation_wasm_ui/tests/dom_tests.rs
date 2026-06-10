//! WHY: Feature 00 moves the DOM reference surface out of `foundation_wasm` into
//! `foundation_wasm_ui`. These tests pin the well-known DOM handle slot ordering and
//! the native behaviour of `allocate_dom_reference`, and prove the `DomInvoke`
//! extension is wired onto `HostFunction`.
//!
//! WHAT: Constant slot values, `allocate_dom_reference` (native stub → slot 0), and a
//! type-level check that `DomInvoke::invoke_for_dom` exists for `HostFunction`.
//!
//! HOW: Asserts via `ExternalPointer::into_inner`. `invoke_for_dom` is referenced as
//! a function pointer (compile/link check) rather than called, to avoid the native
//! host-invocation stub.

use foundation_wasm::abi::web::HostFunction;
use foundation_wasm::{ExternalPointer, Params};
use foundation_wasm_ui::wasm::dom::{
    allocate_dom_reference, DomInvoke, DOM_BODY, DOM_DOCUMENT, DOM_SELF, DOM_THIS, DOM_WINDOW,
};

#[test]
fn dom_constants_have_reserved_slot_ordering() {
    assert_eq!(DOM_SELF.into_inner(), 0);
    assert_eq!(DOM_THIS.into_inner(), 1);
    assert_eq!(DOM_WINDOW.into_inner(), 2);
    assert_eq!(DOM_DOCUMENT.into_inner(), 3);
    assert_eq!(DOM_BODY.into_inner(), 4);
}

#[test]
fn allocate_dom_reference_uses_native_stub_slot_zero() {
    // On non-WASM targets the host import is stubbed to return slot 0.
    assert_eq!(allocate_dom_reference().into_inner(), 0);
}

#[test]
fn dom_invoke_is_implemented_for_host_function() {
    // Type-level proof the extension trait is wired onto HostFunction, without
    // triggering the native host-invocation stub.
    let _f: fn(&HostFunction, &[Params]) -> ExternalPointer = HostFunction::invoke_for_dom;
}
