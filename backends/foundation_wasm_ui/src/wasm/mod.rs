//! WHY: wasm32-specific glue — DOM bindings, animation hooks, and (later) custom
//! element registration that only make sense in a browser/WASM host.
//!
//! WHAT: Container for the WASM-host-facing modules.
//!
//! HOW: Re-exports `dom` so callers reach `foundation_wasm_ui::wasm::dom::*`.

pub mod dom;
