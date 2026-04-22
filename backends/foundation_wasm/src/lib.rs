#![allow(clippy::pedantic)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::similar_names)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_continue)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::type_complexity)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::must_use_candidate)]
#![no_std]

extern crate alloc;

mod base;
mod error;
mod frames;
mod intervals;
mod jsapi;
mod mem;
mod ops;
mod registry;
mod schedule;
mod wrapped;

pub use base::*;
pub use error::*;
pub use frames::*;
pub use intervals::*;
pub use jsapi::*;
pub use mem::*;
pub use ops::*;
pub use registry::*;
pub use schedule::*;
pub use wrapped::*;
