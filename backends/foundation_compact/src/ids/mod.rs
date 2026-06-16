// SCRU128 monotonic IDs — folded from foundation_rng.
// Original: https://github.com/scru128/rust (MIT OR Apache-2.0)

#[cfg(not(feature = "std"))]
use core as std;

mod rand_adapter;
#[cfg(feature = "global_gen")]
mod global_gen;

pub mod generator;
pub mod id;

pub use generator::{Generator, RandSource, StdSystemTime, TimeSource};
pub use id::{FieldError, Id, ParseError};
pub use rand_adapter::Adapter;

#[cfg(feature = "global_gen")]
pub use global_gen::{new, new_string};

/// The maximum value of 48-bit `timestamp` field.
const MAX_TIMESTAMP: u64 = 0xffff_ffff_ffff;

/// The maximum value of 24-bit `counter_hi` field.
const MAX_COUNTER_HI: u32 = 0xff_ffff;

/// The maximum value of 24-bit `counter_lo` field.
const MAX_COUNTER_LO: u32 = 0xff_ffff;

/// Generate a new SCRU128 ID using a thread-local generator.
///
/// Uses our vendored entropy + ChaCha12 RNG + cross-platform SystemTime.
/// Works on all targets including `wasm32-unknown-unknown`.
#[cfg(feature = "std")]
pub fn new_scru128() -> Id {
    use std::cell::RefCell;

    type DefaultGen = Generator<Adapter<crate::rng::ThreadRng>, StdSystemTime>;
    thread_local! {
        static GEN: RefCell<DefaultGen> = RefCell::new(
            Generator::with_rand_and_time_sources(
                Adapter(crate::rng::rng()),
                StdSystemTime,
            )
        );
    }
    GEN.with_borrow_mut(|g| g.generate())
}

/// Generate a new SCRU128 ID as a 25-digit canonical string.
///
/// Works on all targets including `wasm32-unknown-unknown`.
#[cfg(feature = "std")]
pub fn new_scru128_string() -> String {
    new_scru128().to_string()
}
