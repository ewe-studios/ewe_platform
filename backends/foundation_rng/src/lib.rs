//! foundation_rng: RNG + scru128 IDs for foundation_ai.
//!
//! Combines `getrandom` (crate) with scru128 (copied verbatim from
//! <https://github.com/scru128/rust>), providing platform-specific
//! `TimeSource` implementations.
//!
//! ## Features
//!
//! - `native` (default) — uses `getrandom` + `std::time::SystemTime`
//! - `js-wasmbindgen` — uses `js_sys::crypto` + `js_sys::Date::now()`
//! - `wasm-runtime` — uses `foundation_wasm` crypto + time APIs

#![cfg_attr(not(feature = "std"), no_std)]

pub mod time;

// ─── scru128 (copied verbatim) ──────────────────────────────────────────────

mod global_gen;
#[cfg(feature = "global_gen")]
pub use global_gen::{new, new_string};

pub mod generator;
pub use generator::{Generator, StdSystemTime};

pub mod id;
pub use id::{FieldError, Id, ParseError};

/// The maximum value of 48-bit `timestamp` field.
const MAX_TIMESTAMP: u64 = 0xffff_ffff_ffff;

/// The maximum value of 24-bit `counter_hi` field.
const MAX_COUNTER_HI: u32 = 0xff_ffff;

/// The maximum value of 24-bit `counter_lo` field.
const MAX_COUNTER_LO: u32 = 0xff_ffff;

// ─── Re-export getrandom ─────────────────────────────────────────────────────

pub use getrandom::{fill, fill_uninit, u32, u64, Error as RngError};

// ─── Convenience ─────────────────────────────────────────────────────────────

/// Generate a new scru128 ID using a thread-local RNG.
#[cfg(all(feature = "std", feature = "sys_rng"))]
pub fn new_scru128() -> Id {
    use crate::generator::with_rand010::Adapter;
    use crate::generator::StdSystemTime;
    use std::cell::RefCell;

    type DefaultGen = Generator<Adapter<rand::rngs::ThreadRng>, StdSystemTime>;
    thread_local! {
        static GEN: RefCell<DefaultGen> =
            RefCell::new(Generator::with_rand010(rand::rng()));
    }
    GEN.with_borrow_mut(|g| g.generate())
}

/// Generate a new scru128 ID as a string using the default generator.
#[cfg(all(feature = "std", feature = "sys_rng"))]
pub fn new_scru128_string() -> String {
    new_scru128().to_string()
}
