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

use foundation_wasm::abi::web::{invoke, HostFunction};
use foundation_wasm::{ExternalPointer, Params, ReturnTypeHints, ReturnTypeId, ThreeState};

// ─── DOM-specific host FFI ─────────────────────────────────────────────────────

/// Host import: pre-allocate an external-pointer slot earmarked for a DOM node.
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
#[link(wasm_import_module = "abi")]
extern "C" {
    fn dom_allocate_external_pointer() -> u64;
}

/// Non-WASM stub so native builds (tests, tooling) link. Returns slot `0`.
#[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
unsafe fn dom_allocate_external_pointer() -> u64 {
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

// ─── DOM invocation extension ──────────────────────────────────────────────────

/// WHY: `HostFunction` lives in the ABI crate and must stay DOM-free, but UI code
/// needs to invoke a host function that yields a DOM node. An extension trait adds
/// that capability without putting DOM types into `foundation_wasm`.
///
/// WHAT: Adds [`invoke_for_dom`](DomInvoke::invoke_for_dom) to `HostFunction`.
///
/// HOW: Implemented for `foundation_wasm`'s `HostFunction` using its public
/// `handler` and the shared `abi::web::invoke`, requesting a `DOMObject` return.
pub trait DomInvoke {
    /// Invoke the host function and interpret its result as a DOM node reference.
    fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer;
}

impl DomInvoke for HostFunction {
    fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer {
        ExternalPointer::pointer(invoke(
            self.handler,
            params,
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::DOMObject)),
        ))
    }
}
