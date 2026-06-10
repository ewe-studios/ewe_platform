//! WHY: DOM references (the well-known `self`/`window`/`document`/`body` handles
//! and DOM-typed invocation) are DOM concepts that must NOT live in the pure ABI
//! crate (decision 015). They move here, built on `foundation_wasm`'s public ABI.
//!
//! WHAT: DOM constant handles ([`constants`]) and DOM element helpers
//! ([`element`] — `allocate_dom_reference`, the `DomInvoke` extension trait).
//!
//! HOW: Each submodule uses only `foundation_wasm`'s public types
//! (`ExternalPointer`, `HostFunction`, `Params`, return-type hints) and declares
//! its own DOM-specific host FFI.

pub mod constants;
pub mod element;

pub use constants::*;
pub use element::*;
