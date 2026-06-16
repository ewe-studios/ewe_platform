// SysRng: rand_core::TryRng backed by our vendored entropy module.
// Replaces getrandom::SysRng — works on all targets including wasm.

use crate::entropy;
use rand_core::{TryCryptoRng, TryRng};

/// System random number generator — entropy direct from the OS.
///
/// Zero-sized type. Each call goes straight to the platform's entropy source
/// (via [`crate::entropy::fill`]), so it is secure but relatively slow.
/// For bulk generation, prefer [`super::ThreadRng`] (which reseeds from this).
#[derive(Clone, Copy, Debug, Default)]
pub struct SysRng;

/// Error type for [`SysRng`] — re-exported from [`crate::entropy::Error`].
pub type SysError = entropy::Error;

impl TryRng for SysRng {
    type Error = SysError;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, SysError> {
        entropy::u32()
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, SysError> {
        entropy::u64()
    }

    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), SysError> {
        entropy::fill(dest)
    }
}

impl TryCryptoRng for SysRng {}
