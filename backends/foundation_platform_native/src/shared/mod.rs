//! Shared wire-format types — compile on ALL targets (wasm32, native,
//! mobile). Both `native/` and `wasm/` import from here so the structs
//! stay in one place.

#[cfg(feature = "modal")]
pub mod modal_types;
#[cfg(feature = "dialog")]
pub mod dialog_types;
