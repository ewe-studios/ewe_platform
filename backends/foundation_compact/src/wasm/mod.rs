//! Polyfilled `std::time` types for wasm32-unknown-unknown.
//!
//! - [`Instant`] — backed by `Performance.now()`
//! - [`SystemTime`], [`SystemTimeError`] — backed by `Date.now()`
//! - Re-exports `std::time::Duration` and `UNIX_EPOCH`

mod instant;
mod system_time;

// The clock the polyfills read: wasm-bindgen glue, or `abi` host imports (see the
// `wasm-bindgen` / `host-abi` features in Cargo.toml).
cfg_if::cfg_if! {
    if #[cfg(all(feature = "wasm-bindgen", not(feature = "host-abi")))] {
        mod js;
        use js as clock;
    } else {
        mod host_abi;
        use host_abi as clock;
    }
}

#[cfg(any(feature = "std", docsrs))]
#[cfg_attr(docsrs, doc(cfg(Web)))]
pub mod web;

pub use std::time::*;

pub use self::instant::Instant;
pub use self::system_time::{SystemTime, SystemTimeError};

/// See [`std::time::UNIX_EPOCH`].
pub const UNIX_EPOCH: SystemTime = SystemTime::UNIX_EPOCH;
