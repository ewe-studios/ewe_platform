//! Entropy for OS-less wasm through the `abi` host import `host_fill_random`.
//!
//! The no-wasm-bindgen counterpart of `wasm_js`: `foundation_wasm`'s
//! `foundation-wasm.js` runtime implements the import with
//! `crypto.getRandomValues()`, so the module carries no wasm-bindgen imports.
use crate::entropy::Error;
use core::mem::MaybeUninit;

pub use crate::entropy::util::{inner_u32, inner_u64};

#[cfg(not(all(target_family = "wasm", any(target_os = "unknown", target_os = "none"),)))]
compile_error!("`host_abi` backend can be enabled only for OS-less WASM targets!");

#[link(wasm_import_module = "abi")]
unsafe extern "C" {
    /// Fill `len` bytes of linear memory at `ptr` with cryptographically secure
    /// random bytes. Returns `0` on success and non-zero when the host has no
    /// secure source (no Web Crypto). The host splits the request to fit its
    /// source's limits (Web Crypto takes at most 65536 bytes per call) and copies
    /// through a scratch buffer when memory is shared.
    fn host_fill_random(ptr: u64, len: u64) -> u32;
}

pub fn fill_inner(dest: &mut [MaybeUninit<u8>]) -> Result<(), Error> {
    if dest.is_empty() {
        return Ok(());
    }
    #[allow(clippy::as_conversions)]
    let (ptr, len) = (dest.as_mut_ptr() as usize as u64, dest.len() as u64);
    // SAFETY: `ptr..ptr + len` is `dest`, writable linear memory we hold an
    // exclusive borrow of for the whole call; the host only writes inside it.
    let status = unsafe { host_fill_random(ptr, len) };
    if status == 0 {
        Ok(())
    } else {
        Err(Error::WEB_CRYPTO)
    }
}

impl Error {
    /// The host has no secure random source (no Web Crypto API).
    pub(crate) const WEB_CRYPTO: Error = Self::new_internal(10);
}
