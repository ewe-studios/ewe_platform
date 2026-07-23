// Default generator and entry point functions.
// Fork-safe global generator using forkguard + our vendored RNG.
// Original: https://github.com/scru128/rust (MIT OR Apache-2.0)

#![cfg(feature = "global_gen")]

use std::sync;

use crate::ids::{Generator, generator::RandSource};
use crate::ids::Id;

/// Generates a new SCRU128 ID object using the global generator.
///
/// This function is thread-safe; multiple threads in a process can call it concurrently without
/// breaking the monotonic order of generated IDs. On Unix, this function resets the generator
/// state when a process fork is detected to avoid collisions across processes.
pub fn new() -> Id {
    static G: sync::LazyLock<sync::Mutex<GlobalGenInner>> = sync::LazyLock::new(Default::default);
    G.lock()
        .expect("could not lock global generator")
        .get_mut()
        .generate()
}

/// Generates a new SCRU128 ID encoded in the 25-digit canonical string representation using the
/// global generator.
///
/// This function is thread-safe.
#[must_use]
pub fn new_string() -> String {
    new().into()
}

/// A thin wrapper to reset the state when a process fork is detected.
#[derive(Debug, Default)]
struct GlobalGenInner {
    guard: forkguard::Guard,
    generator: Generator<GlobalGenRng>,
}

impl GlobalGenInner {
    fn get_mut(&mut self) -> &mut Generator<GlobalGenRng> {
        if self.guard.detected_fork() {
            self.reset_generator();
        }
        &mut self.generator
    }

    #[cold]
    fn reset_generator(&mut self) {
        self.generator.reset_state();
        self.generator.rand_source_mut().reseed();
    }
}

/// A CSPRNG backed by our vendored `StdRng` + `SysRng` for reseeding.
#[derive(Debug)]
struct GlobalGenRng {
    inner: crate::rng::StdRng,
}

impl Default for GlobalGenRng {
    fn default() -> Self {
        use rand_core::SeedableRng;
        Self {
            inner: crate::rng::StdRng::try_from_rng(&mut crate::rng::SysRng)
                .expect("failed to seed global gen RNG"),
        }
    }
}

impl GlobalGenRng {
    fn reseed(&mut self) {
        use rand_core::SeedableRng;
        if let Ok(rng) = crate::rng::StdRng::try_from_rng(&mut crate::rng::SysRng) {
            self.inner = rng;
        }
    }
}

impl RandSource for GlobalGenRng {
    fn next_u32(&mut self) -> u32 {
        use crate::rng::Rng;
        self.inner.next_u32()
    }
}
