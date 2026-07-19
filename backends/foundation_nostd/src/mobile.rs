//! WHY: `EmbeddableDirectory` embeds files as `&'static [u8]` at compile time —
//! every OTA app update needs a full `.so` rebuild. Mobile apps (F22) need
//! disk-backed asset serving so `.wasm` + `.js` bundles can be replaced
//! on-device without touching the native binary.
//!
//! WHAT: [`MobileDirectory`] — a no_std-compatible trait for compile-time
//! file metadata. The [`MobileDisk`] extension trait (in `foundation_platform`)
//! adds the `std`-backed `read_utf8_for` / `read_utf16_for` disk methods.
//! The `#[derive(MobileDirectory)]` macro implements both traits.
//!
//! HOW: Keep the base trait metadata-only so it compiles everywhere. The
//! disk I/O methods live where `std` is already available.

use crate::embeddable::FileInfo;

pub trait MobileDirectory {
    /// Static file metadata populated at compile time from the `#[source]`
    /// directory scan.
    const FILES_METADATA: &'static [FileInfo];

    /// The runtime asset root directory path.
    fn root_str(&self) -> &str;

    /// Iterate over compile-time file metadata.
    fn info_iter(&self) -> core::slice::Iter<'static, FileInfo> {
        Self::FILES_METADATA.iter()
    }

    /// Look up a file's metadata by source path.
    fn info_for(&self, source: &str) -> Option<&FileInfo> {
        Self::FILES_METADATA
            .iter()
            .find(|i| i.source_path == source || i.source_path_from_parent == source)
    }
}
