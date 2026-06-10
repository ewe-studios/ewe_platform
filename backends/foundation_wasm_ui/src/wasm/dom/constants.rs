//! WHY: The host reserves fixed external-pointer slots for the ambient DOM objects
//! (`self`, `this`, `window`, `document`, `body`) so WASM can reference them without
//! a round-trip allocation. These are DOM concepts, so they live in
//! `foundation_wasm_ui`, not the ABI crate.
//!
//! WHAT: The five well-known [`ExternalPointer`] DOM handles.
//!
//! HOW: Each is a compile-time `ExternalPointer::pointer(N)` matching the host's
//! reserved slot ordering. Moved verbatim from `foundation_wasm` (feature 00).

use foundation_wasm::ExternalPointer;

/// The element a controller/handler is bound to (`self`).
pub const DOM_SELF: ExternalPointer = ExternalPointer::pointer(0);
/// The current event target (`this`).
pub const DOM_THIS: ExternalPointer = ExternalPointer::pointer(1);
/// The global `window` object.
pub const DOM_WINDOW: ExternalPointer = ExternalPointer::pointer(2);
/// The `document` object.
pub const DOM_DOCUMENT: ExternalPointer = ExternalPointer::pointer(3);
/// The `<body>` element.
pub const DOM_BODY: ExternalPointer = ExternalPointer::pointer(4);
