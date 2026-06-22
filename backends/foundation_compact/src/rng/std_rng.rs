// Vendored from rand 0.10.1 (MIT OR Apache-2.0).
// Adapted: uses rand_chacha::ChaCha12Rng instead of chacha20::ChaCha12Rng.

use core::convert::Infallible;
use rand_core::{SeedableRng, TryCryptoRng, TryRng};

use rand_chacha::ChaCha12Rng as Rng;

/// A strong, fast (amortized), non-portable CSPRNG.
///
/// Uses `ChaCha12` internally. Non-portable: the algorithm may change in future
/// versions. For a portable generator, use [`rand_chacha`] directly.
///
/// # Seeding
///
/// ```ignore
/// use foundation_compact::rng::{StdRng, SysRng};
/// use rand_core::SeedableRng;
/// let rng = StdRng::try_from_rng(&mut SysRng).unwrap();
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct StdRng(pub(crate) Rng);

impl TryRng for StdRng {
    type Error = Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        self.0.try_next_u32()
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        self.0.try_next_u64()
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
        self.0.try_fill_bytes(dst)
    }
}

impl SeedableRng for StdRng {
    type Seed = [u8; 32];

    #[inline]
    fn from_seed(seed: Self::Seed) -> Self {
        StdRng(Rng::from_seed(seed))
    }
}

impl TryCryptoRng for StdRng {}
