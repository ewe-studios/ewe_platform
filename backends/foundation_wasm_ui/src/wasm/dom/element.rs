//! WHY: Allocating a DOM-typed external reference and invoking a host function that
//! returns a DOM node are DOM-specific operations. They wrap the host ABI but carry
//! DOM semantics (`ReturnTypeId::DOMObject`), so they belong here, not in the pure
//! ABI crate.
//!
//! WHAT: [`allocate_dom_reference`] (pre-allocate a host slot for a DOM node) and the
//! [`DomInvoke`] extension trait that adds [`DomInvoke::invoke_for_dom`] to
//! `foundation_wasm`'s `HostFunction`.
//!
//! HOW: Declares its own DOM host FFI (`dom_allocate_external_pointer`, imported from
//! the `abi` host module) with a non-WASM stub, and reuses `foundation_wasm`'s public
//! `abi::web::invoke` for the typed call. Moved from `foundation_wasm` (feature 00).

use foundation_wasm::raw_parts::RawParts;
use foundation_wasm::abi::web::HostFunction;
use foundation_wasm::{ExternalPointer, Params, ToBinary};

// ─── DOM-specific host FFI ─────────────────────────────────────────────────────

#[cfg(target_family = "wasm")]
#[link(wasm_import_module = "abi")]
extern "C" {
    /// Host import: pre-allocate an external-pointer slot earmarked for a DOM node.
    fn dom_allocate_external_pointer() -> u64;

    /// Host import: retire a DOM-heap external reference so the host can free the
    /// slot (reserved slots 0–4 refuse).
    fn host_dom_drop_external_pointer(handle: u64);

    /// Host import: invoke a registered function expecting a DOM node back — the
    /// host interns the node in its DOM heap and returns the heap handle NAKED
    /// (no reply encoding; megatron `as_dom` parity).
    fn host_invoke_function_as_dom(
        handler: u64,
        parameters_start: *const u8,
        parameters_length: u64,
    ) -> u64;
}

/// Non-WASM stubs so native builds (tests, tooling) link.
#[cfg(not(target_family = "wasm"))]
unsafe fn dom_allocate_external_pointer() -> u64 {
    0
}

#[cfg(not(target_family = "wasm"))]
unsafe fn host_dom_drop_external_pointer(_handle: u64) {}

#[cfg(not(target_family = "wasm"))]
unsafe fn host_invoke_function_as_dom(
    _handler: u64,
    _parameters_start: *const u8,
    _parameters_length: u64,
) -> u64 {
    0
}

/// WHY: Callers sometimes need a stable DOM reference before the node exists (e.g.
/// to wire it into a batch); pre-allocating avoids a later round-trip.
///
/// WHAT: Requests the host runtime to pre-allocate an external reference for a DOM
/// node and returns its [`ExternalPointer`].
///
/// HOW: Calls the host `dom_allocate_external_pointer` import and wraps the raw slot
/// id in an `ExternalPointer`.
///
/// # Panics
/// Never panics.
#[must_use]
pub fn allocate_dom_reference() -> ExternalPointer {
    unsafe { ExternalPointer::pointer(dom_allocate_external_pointer()) }
}

/// WHY: DOM references pin host-side nodes; long-lived apps must release them.
///
/// WHAT: Retires a DOM-heap external reference so the host can recycle the slot
/// (reserved slots — self/window/document/body — refuse).
///
/// HOW: Calls the host `host_dom_drop_external_pointer` import with the raw id.
pub fn drop_dom_reference(handle: ExternalPointer) {
    unsafe { host_dom_drop_external_pointer(handle.into_inner()) };
}

// ─── DOM invocation extension ──────────────────────────────────────────────────

/// WHY: `HostFunction` lives in the ABI crate and must stay DOM-free, but UI code
/// needs to invoke a host function that yields a DOM node. An extension trait adds
/// that capability without putting DOM types into `foundation_wasm`.
///
/// WHAT: Adds [`invoke_for_dom`](DomInvoke::invoke_for_dom) to `HostFunction`.
///
/// HOW: Implemented for `foundation_wasm`'s `HostFunction` using its public
/// `handler` and the DOM-specific `host_invoke_function_as_dom` import — the host
/// interns the returned node in its DOM heap and the handle crosses NAKED (no reply
/// encoding), exactly like the numeric `as_*` fast-paths.
pub trait DomInvoke {
    /// Invoke the host function and interpret its result as a DOM node reference.
    fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer;
}

impl DomInvoke for HostFunction {
    fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer {
        let param_bytes = params.to_binary();
        let param_raw = RawParts::from_vec(param_bytes);

        ExternalPointer::pointer(unsafe {
            host_invoke_function_as_dom(self.handler, param_raw.ptr, param_raw.length)
        })
    }
}
