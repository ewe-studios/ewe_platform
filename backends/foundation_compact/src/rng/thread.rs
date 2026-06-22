// Vendored from rand 0.10.1 (MIT OR Apache-2.0).
// Adapted: uses rand_chacha::ChaCha12Core + our SysRng (vendored entropy).
// Tracks block count ourselves since rand_chacha Core doesn't expose get_block_pos.

use core::{cell::UnsafeCell, convert::Infallible};
use std::fmt;
use std::rc::Rc;
use std::thread_local;

use super::sys_rng::{SysError, SysRng};
use rand_core::SeedableRng;
use rand_core::block::{BlockRng, Generator};
use rand_core::{TryCryptoRng, TryRng};

// Reseed after 1024 blocks = 64 KiB of output (16 words/block × 4 bytes × 1024).
const RESEED_BLOCK_THRESHOLD: u64 = 1024;

type Core = rand_chacha::ChaCha12Core;
type Results = <Core as Generator>::Output;

struct ReseedingCore {
    inner: Core,
    blocks_generated: u64,
}

impl Generator for ReseedingCore {
    type Output = Results;

    #[inline]
    fn generate(&mut self, results: &mut Results) {
        self.blocks_generated += 1;
        if self.blocks_generated >= RESEED_BLOCK_THRESHOLD {
            self.try_to_reseed();
        }
        self.inner.generate(results);
    }
}

impl ReseedingCore {
    fn reseed(&mut self) -> Result<(), SysError> {
        Core::try_from_rng(&mut SysRng).map(|result| {
            self.inner = result;
            self.blocks_generated = 0;
        })
    }

    #[cold]
    #[inline(never)]
    fn try_to_reseed(&mut self) {
        if let Err(e) = self.reseed() {
            panic!("could not reseed ThreadRng: {e}");
        }
    }
}

/// A reference to the thread-local CSPRNG.
///
/// Obtained via [`super::rng()`]. Uses `ChaCha12` internally, automatically
/// reseeded from [`SysRng`] (our vendored OS entropy) every 64 KiB of output.
/// Works on all targets including `wasm32-unknown-unknown`.
///
/// Not `Send` or `Sync` — stays on the creating thread.
#[derive(Clone)]
pub struct ThreadRng {
    rng: Rc<UnsafeCell<BlockRng<ReseedingCore>>>,
}

impl ThreadRng {
    /// Immediately reseed the generator.
    ///
    /// # Errors
    ///
    /// Returns [`SysError`] if the platform entropy source is unavailable.
    pub fn reseed(&mut self) -> Result<(), SysError> {
        let rng = unsafe { &mut *self.rng.get() };
        rng.reset_and_skip(0);
        rng.core.reseed()
    }
}

impl fmt::Debug for ThreadRng {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "ThreadRng {{ .. }}")
    }
}

thread_local!(
    static THREAD_RNG_KEY: Rc<UnsafeCell<BlockRng<ReseedingCore>>> = {
        Rc::new(UnsafeCell::new(BlockRng::new(ReseedingCore {
            inner: Core::try_from_rng(&mut SysRng).unwrap_or_else(|err| {
                panic!("could not initialize ThreadRng: {err}")
            }),
            blocks_generated: 0,
        })))
    }
);

/// Access a fast, pre-initialized thread-local CSPRNG.
///
/// Returns a handle to the thread-local [`ThreadRng`], seeded from
/// our vendored OS entropy. Works on all targets including wasm.
pub fn rng() -> ThreadRng {
    let rng = THREAD_RNG_KEY.with(std::clone::Clone::clone);
    ThreadRng { rng }
}

impl Default for ThreadRng {
    fn default() -> ThreadRng {
        rng()
    }
}

impl TryRng for ThreadRng {
    type Error = Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Infallible> {
        let rng = unsafe { &mut *self.rng.get() };
        Ok(rng.next_word())
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Infallible> {
        let rng = unsafe { &mut *self.rng.get() };
        Ok(rng.next_u64_from_u32())
    }

    #[inline]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Infallible> {
        let rng = unsafe { &mut *self.rng.get() };
        rng.fill_bytes(dest);
        Ok(())
    }
}

impl TryCryptoRng for ThreadRng {}
