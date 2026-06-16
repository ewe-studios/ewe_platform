// RNG module: CSPRNG seeded from vendored entropy.
// Uses rand_core + rand_chacha from crates.io (pure computation, wasm-safe).
// Vendors only the glue: SysRng, ThreadRng, StdRng, SmallRng.

#[cfg(feature = "std")]
mod thread;
mod std_rng;
mod small;
mod xoshiro128plusplus;
mod xoshiro256plusplus;

pub mod sys_rng;

// Re-export rand_core traits for downstream use
pub use rand_core::{self, CryptoRng, Rng, SeedableRng, TryCryptoRng, TryRng};

// Re-export rand_chacha types
pub use rand_chacha::{self, ChaCha8Rng, ChaCha12Rng, ChaCha20Rng};

// Our types
pub use sys_rng::{SysError, SysRng};
pub use std_rng::StdRng;
pub use small::SmallRng;
pub use xoshiro128plusplus::Xoshiro128PlusPlus;
pub use xoshiro256plusplus::Xoshiro256PlusPlus;

#[cfg(feature = "std")]
pub use thread::ThreadRng;

/// Access a fast, pre-initialized thread-local CSPRNG.
///
/// Shorthand for [`ThreadRng::default()`]. Works on all targets including wasm.
#[cfg(feature = "std")]
pub fn rng() -> ThreadRng {
    thread::rng()
}
