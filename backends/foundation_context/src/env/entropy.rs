//! The entropy effect: random bytes for keys, tokens and seeds.
//!
//! The entropy source is not the shared random number generator (RNG) of
//! [`Ctx`](crate::Ctx). A caller can draw from that stream too, so a key must
//! not come from it.

use foundation_compact::entropy;
use foundation_compact::rng::{ChaCha12Rng, Rng, SeedableRng};

use crate::sync::Mutex;

/// The source of random bytes for keys, tokens and seeds.
pub trait EntropySource: Send + Sync + 'static {
    /// Fills `dest` with random bytes.
    ///
    /// # Errors
    /// The platform source failed. A caller must not continue with a
    /// half-filled buffer.
    fn fill(&self, dest: &mut [u8]) -> Result<(), entropy::Error>;

    /// Generate a version four universally unique identifier (UUID) from this source.
    ///
    /// # Errors
    /// Returns the error from this source.
    fn uuid_v4(&self) -> Result<foundation_compact::uuid::UuidV4, entropy::Error> {
        foundation_compact::uuid::UuidV4::from_entropy(|bytes| self.fill(bytes))
    }
}

/// The operating-system source. Native: `getrandom`/`getentropy`. wasm:
/// `crypto.getRandomValues`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OsEntropy;

impl EntropySource for OsEntropy {
    fn fill(&self, dest: &mut [u8]) -> Result<(), entropy::Error> {
        foundation_compact::entropy::fill(dest)
    }
}

/// A seeded source. The same seed gives the same bytes on every run and every
/// platform.
///
/// The type is available in every build, so a test crate needs no extra
/// feature. Use it only in a test or in the deterministic simulation harness.
pub struct SeededEntropy {
    seed: u64,
    rng: Mutex<ChaCha12Rng>,
}

impl SeededEntropy {
    /// A source from `seed`. It cannot fail.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rng: Mutex::new(ChaCha12Rng::seed_from_u64(seed)),
        }
    }

    /// The seed of this source.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl EntropySource for SeededEntropy {
    fn fill(&self, dest: &mut [u8]) -> Result<(), entropy::Error> {
        // A poisoned lock still holds a valid generator. Recover it; the wasm
        // lock error type differs from the std one, so a `match` serves both.
        let mut rng = match self.rng.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        rng.fill_bytes(dest);
        Ok(())
    }
}
