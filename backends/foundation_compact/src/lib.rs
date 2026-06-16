//! Cross-platform substrate: time + vendored entropy + RNG + scru128 IDs.
//!
//! On `wasm32-unknown-unknown`, `std::time` panics — this crate provides drop-in
//! replacements backed by JavaScript's `Performance.now()` and `Date.now()`.
//! On all other targets (native, emscripten, WASI), re-exports `std::time`.
//!
//! Also provides:
//! - **`entropy`** — vendored OS entropy (getrandom backends for all targets)
//! - **`rng`** — ChaCha-based CSPRNG seeded from entropy (no external rand/getrandom)
//! - **`ids`** — scru128 monotonic IDs (folded from `foundation_rng`)
//!
//! # Usage
//!
//! ```
//! use foundation_compact::Instant;
//!
//! let now = Instant::now();
//! let elapsed = now.elapsed();
//! ```

#[macro_use]
extern crate cfg_if;

// ─── Time ───────────────────────────────────────────────────────────────────────

#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
mod wasm;
#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
pub use wasm::*;

#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
mod std;
#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
pub use std::*;

// ─── Entropy ────────────────────────────────────────────────────────────────────

pub mod entropy;

// ─── RNG ────────────────────────────────────────────────────────────────────────

pub mod rng;

// ─── IDs ────────────────────────────────────────────────────────────────────────

pub mod ids;
