//! Polyfilled `std::time` types for wasm32-unknown-unknown.
//!
//! - [`Instant`] — backed by `Performance.now()`
//! - [`SystemTime`], [`SystemTimeError`] — backed by `js_sys::Date::now()`
//! - Re-exports `std::time::Duration` and `UNIX_EPOCH`

mod instant;
mod js;
mod system_time;

#[cfg(any(feature = "std", docsrs))]
#[cfg_attr(docsrs, doc(cfg(Web)))]
pub mod web;

pub use std::time::*;

pub use self::instant::Instant;
pub use self::system_time::{SystemTime, SystemTimeError};

/// See [`std::time::UNIX_EPOCH`].
pub const UNIX_EPOCH: SystemTime = SystemTime::UNIX_EPOCH;
