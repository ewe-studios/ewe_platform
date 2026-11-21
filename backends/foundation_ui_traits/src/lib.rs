//! Shared markup, template parts, and platform names.

#![cfg_attr(not(test), no_std)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]

extern crate alloc;

mod html;
mod markup;
mod parts;
mod platform_types;

pub use html::{
    AttrName, Html, HtmlTag, IntoAttrValue, IntoHtml, ATTR_CLASS, ATTR_ID, ATTR_NAMES, ATTR_STYLE,
    ATTR_VALUE, TAG_BUTTON, TAG_DIV, TAG_INPUT, TAG_NAMES, TAG_SPAN,
};
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
