// Adapter bridging rand_core::Rng → RandSource for the SCRU128 generator.

use super::generator::{Generator, RandSource, StdSystemTime};
use rand_core::Rng;

/// An adapter that implements [`RandSource`] for any [`Rng`] type.
#[derive(Clone, Debug, Default)]
pub struct Adapter<T>(pub T);

impl<T: Rng> RandSource for Adapter<T> {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
}

impl<T: Rng> Generator<Adapter<T>> {
    /// Creates a generator with a specified RNG that implements [`Rng`].
    ///
    /// The RNG should be cryptographically strong and securely seeded.
    pub const fn with_rng(rng: T) -> Self {
        Self::with_rand_and_time_sources(Adapter(rng), StdSystemTime)
    }
}
