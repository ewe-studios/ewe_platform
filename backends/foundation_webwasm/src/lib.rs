//! Polyfilled `std::time` APIs for wasm32-unknown-unknown.
//!
//! On wasm32 targets, `std::time::Instant::now()` and `std::time::SystemTime::now()`
//! panic because the platform has no native time API. This crate provides drop-in
//! replacements backed by JavaScript's `Performance.now()` and `Date.now()`.
//!
//! On all other targets, this crate simply re-exports `std::time` with zero overhead.
//!
//! # Features
//!
//! - `std` — enables std feature on dependencies (for optimized instruction output)
//! - `serde` — implements `Serialize`/`Deserialize` for `Instant` and `SystemTime`
//! - `msrv` — enables use of `f64.nearest` instruction for better `Instant::now()` performance
//!   (requires Rust >= 1.77)
//!
//! # Usage
//!
//! ```
//! use foundation_webwasm::Instant;
//!
//! let now = Instant::now();
//! let elapsed = now.elapsed();
//! ```

#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
mod wasm;
#[cfg(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none")))]
pub use wasm::*;

#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
mod std;
#[cfg(not(all(target_arch = "wasm32", any(target_os = "unknown", target_os = "none"))))]
pub use std::*;
