//! WHY: `foundation_signals` needs `IntoHtml` to implement on `Signal<T>` and
//! `foundation_wasm_ui` needs the DOM-op vocabulary and encoders — neither may
//! depend on the other (decision 012). This crate is the shared middle layer:
//! pure types and pure codecs, `no_std`, zero dependencies, compiles for any
//! Rust target.
//!
//! WHAT: The [`IntoHtml`] trait + [`Html`] tree, [`Part`] descriptors, the
//! 19-variant [`DomOp`] enum with [`HtmlTag`]/[`AttrName`] wire ids, the
//! [`ProtocolEncoder`] contract with [`ColumnarEncoder`]/[`JsonEncoder`], and the
//! 6-byte [`Envelope`]. (Protocol byte 0 — Custom Binary — is the
//! `foundation_wasm` batch-instructions stream per decision 022; its
//! `ProtocolHandler` lives in `foundation_wasm_ui`.)
//!
//! HOW: One module per concern (feature 01 spec section 8); everything re-
//! exported flat so consumers write `use foundation_ui_traits::DomOp;`.

#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

extern crate alloc;

mod columnar_encoder;
mod dom_op;
mod encoder;
mod envelope;
mod html;
mod json_encoder;
mod markup;
mod parts;
mod platform_types;

pub use columnar_encoder::{ColumnarBatch, ColumnarEncoder};
pub use dom_op::{DomOp, MorphAction, TargetSelector};
pub use encoder::{
    row_view, DecodeError, DecodeResult, ProtocolEncoder, Row, OP_COUNT, PROTOCOL_ARROW,
    PROTOCOL_CUSTOM_BINARY, PROTOCOL_JSON, PROTOCOL_VERSION,
};
pub use envelope::{encode_with_envelope, Envelope, ENVELOPE_SIZE};
pub use html::{
    AttrName, Html, HtmlTag, IntoAttrValue, IntoHtml, ATTR_CLASS, ATTR_ID, ATTR_NAMES, ATTR_STYLE,
    ATTR_VALUE, TAG_BUTTON, TAG_DIV, TAG_INPUT, TAG_NAMES, TAG_SPAN,
};
pub use json_encoder::JsonEncoder;
pub use parts::{AttrPart, EventPart, Part, TextPart};
pub use platform_types::*;

/// Hidden re-exports for `foundation_macros::html!` codegen: the generated
/// code must name alloc types without knowing whether the calling crate is
/// `std` or `no_std` (where `::std` paths would not resolve).
#[doc(hidden)]
pub mod __macro {
    pub use alloc::borrow::Cow;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

