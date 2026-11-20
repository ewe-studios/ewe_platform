#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

mod align;
mod error;
mod error_ext;
mod finish;
// The FlatBuffers compiler generates this module.
#[allow(
    clippy::all,
    clippy::pedantic,
    unused_imports,
    dead_code,
    non_snake_case
)]
mod generated;
mod limits;
mod read;

pub use align::{create_aligned_vector, Aligned8};
pub use error::{FrameKindName, WireError};
pub use finish::{finish_frame, finish_signed_state};
pub use generated::ewe::ui::*;
pub use limits::WireLimits;
pub use read::{expect_kind, frame_kind, read_frame, read_signed_state};

pub const SCHEMA_VERSION: u16 = 1;
pub const FRAME_IDENTIFIER: &str = "EWF1";
pub const STATE_IDENTIFIER: &str = "EWS1";
