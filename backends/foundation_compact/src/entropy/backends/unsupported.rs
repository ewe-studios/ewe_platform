//! Implementation that errors at runtime.
use crate::entropy::Error;
use core::mem::MaybeUninit;

pub use crate::entropy::util::{inner_u32, inner_u64};

pub fn fill_inner(_dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    Err(Error::UNSUPPORTED)
}
