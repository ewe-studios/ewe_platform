//! Cross-platform substrate: time + vendored entropy + RNG + scru128 IDs.
//!
//! On `wasm32-unknown-unknown`, `std::time` panics — this crate provides drop-in
//! replacements backed by JavaScript's `Performance.now()` and `Date.now()`.
//! On all other targets (native, emscripten, WASI), re-exports `std::time`.
//!
//! On `wasm32-unknown-unknown` the JavaScript calls (time and entropy) go through
//! one of two backends: `wasm-bindgen` glue (the default feature) or, with the
//! `host-abi` feature, plain `abi` host imports that `foundation_wasm`'s
//! `foundation-wasm.js` runtime implements, so the module carries no wasm-bindgen
//! imports. `host-abi` wins when both are enabled.
//!
//! Also provides:
//! - **`entropy`** — vendored OS entropy (getrandom backends for all targets)
//! - **`rng`** — ChaCha-based CSPRNG seeded from entropy (no external rand/getrandom)
//! - **`ids`** — scru128 monotonic IDs (folded from `foundation_rng`)
//! - **`hash`** — deterministic, wasm-safe content hashing (foldhash, no getrandom)
//!
//! # Usage
//!
//! ```
//! use foundation_compact::Instant;
//!
//! let now = Instant::now();
//! let elapsed = now.elapsed();
//! ```

// ─── Time ───────────────────────────────────────────────────────────────────────

#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
mod wasm;
#[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))]
pub use wasm::*;

#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
mod std;
#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"))))]
pub use std::*;

pub mod timestamp;

// ─── Entropy ────────────────────────────────────────────────────────────────────

pub mod entropy;

// ─── RNG ────────────────────────────────────────────────────────────────────────

pub mod rng;

// ─── IDs ────────────────────────────────────────────────────────────────────────

pub mod ids;

// ─── Crypto ────────────────────────────────────────────────────────────────────

pub mod crypto;

// ─── Hash ──────────────────────────────────────────────────────────────────────

pub mod hash;

// ─── Tracing ─────────────────────────────────────────────────────────────────

pub mod trace;

// ─── Send Wrapper ──────────────────────────────────────────────────────────────

pub mod send_wrapper;
pub use send_wrapper::SendWrapper;
