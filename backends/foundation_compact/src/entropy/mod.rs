// Vendored from getrandom 0.4.2 (MIT OR Apache-2.0).
// Public API: fill(), fill_uninit(), u32(), u64().

use core::mem::MaybeUninit;

mod backends;
mod error;
pub(crate) mod util;

#[cfg(feature = "std")]
mod error_std_impls;

pub use crate::entropy::error::{Error, RawOsError};

#[cfg(getrandom_backend = "extern_impl")]
pub mod implementation {
    pub use crate::entropy::backends::extern_impl::{fill_uninit, u32, u64};
}

/// Fill `dest` with random bytes from the system's preferred random number source.
#[inline]
pub fn fill(dest: &mut [u8]) -> Result<(), Error> {
    fill_uninit(unsafe { util::slice_as_uninit_mut(dest) })?;
    Ok(())
}

/// Fill potentially uninitialized buffer `dest` with random bytes from
/// the system's preferred random number source and return a mutable
/// reference to those bytes.
#[inline]
pub fn fill_uninit(dest: &mut [MaybeUninit<u8>]) -> Result<&mut [u8], Error> {
    if !dest.is_empty() {
        backends::fill_inner(dest)?;
    }
    Ok(unsafe { util::slice_assume_init_mut(dest) })
}

/// Get random `u32` from the system's preferred random number source.
#[inline]
pub fn u32() -> Result<u32, Error> {
    backends::inner_u32()
}

/// Get random `u64` from the system's preferred random number source.
#[inline]
pub fn u64() -> Result<u64, Error> {
    backends::inner_u64()
}
