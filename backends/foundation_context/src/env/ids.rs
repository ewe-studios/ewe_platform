//! The identifier effect: a source of new, sortable IDs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_compact::entropy;
use foundation_compact::ids::{Adapter, Generator, Id, TimeSource};
use foundation_compact::rng::{ChaCha12Rng, SeedableRng};

use super::entropy::EntropySource;
use crate::sync::Mutex;
use crate::Clock;

/// A source of new, sortable IDs.
pub trait IdSource: Send + Sync + 'static {
    /// A new ID. It does not fail after construction.
    fn next_id(&self) -> Id;
}

/// scru128 IDs from an injected clock and an injected entropy source.
///
/// The source reads one 32-byte seed from the entropy source at construction.
/// Later calls to [`IdSource::next_id`] cannot fail. With a simulated clock and
/// a [`SeededEntropy`](super::entropy::SeededEntropy), the IDs are the same on
/// every run.
pub struct Scru128Source {
    generator: Mutex<Generator<Adapter<ChaCha12Rng>, ClockTime>>,
}

impl Scru128Source {
    /// Takes a 32-byte seed from `entropy` one time.
    ///
    /// # Errors
    /// The entropy source failed.
    pub fn new(clock: Arc<dyn Clock>, entropy: &dyn EntropySource) -> Result<Self, entropy::Error> {
        let mut seed = [0u8; 32];
        entropy.fill(&mut seed)?;
        Ok(Self {
            generator: Mutex::new(Generator::with_rand_and_time_sources(
                Adapter(ChaCha12Rng::from_seed(seed)),
                ClockTime(clock),
            )),
        })
    }
}

impl IdSource for Scru128Source {
    fn next_id(&self) -> Id {
        // A poisoned lock still holds a valid generator. Recover it; the wasm
        // lock error type differs from the std one, so a `match` serves both.
        let mut generator = match self.generator.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        generator.generate()
    }
}

/// A [`TimeSource`] over the injected clock.
struct ClockTime(Arc<dyn Clock>);

impl TimeSource for ClockTime {
    fn unix_ts_ms(&mut self) -> u64 {
        // scru128 requires a positive timestamp. A simulated clock can start at
        // the UNIX epoch, where the true value is zero, so clamp to 1.
        let millis = foundation_compact::time::unix_millis(self.0.wall_now());
        u64::try_from(millis).unwrap_or(1).max(1)
    }
}

/// A counter: `1`, `2`, `3`, … as the low bits of an [`Id`].
///
/// The ID is deterministic, so a test can assert on the value. Use it only in
/// a test; it is not for production.
pub struct CounterSource {
    next: AtomicU64,
}

impl CounterSource {
    /// A source whose first ID is `start`.
    #[must_use]
    pub fn new(start: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
        }
    }
}

impl IdSource for CounterSource {
    fn next_id(&self) -> Id {
        Id::from_u128(u128::from(self.next.fetch_add(1, Ordering::Relaxed)))
    }
}

impl Default for CounterSource {
    fn default() -> Self {
        Self::new(1)
    }
}
