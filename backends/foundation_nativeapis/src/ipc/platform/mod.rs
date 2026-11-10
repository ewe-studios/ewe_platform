/// Platform-specific IPC transports.
///
/// | Platform | Transport | Mechanism |
/// |----------|-----------|-----------|
/// | Linux | Unix domain sockets (`SOCK_SEQPACKET`) | Abstract socket addresses, `SCM_RIGHTS` for FD passing |
/// | macOS | Mach ports | `mach_msg`, `mach_port` operations |
/// | Windows | Named pipes | `CreateNamedPipe`, `ConnectNamedPipe` |
use std::{
    io,
    ops::RangeBounds,
    slice,
    sync::atomic::{AtomicU32, Ordering},
};

use super::Error;

#[cfg(target_os = "linux")]
pub type Object = self::linux::fd::Fd;
#[cfg(target_os = "macos")]
pub type Object = self::macos::MachPort;
#[cfg(target_os = "windows")]
pub type Object = self::windows::Handle;

#[cfg(target_os = "linux")]
pub(crate) use self::linux::{
    look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote,
};
#[cfg(target_os = "macos")]
pub(crate) use self::macos::{
    look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote,
};
#[cfg(target_os = "windows")]
pub(crate) use self::windows::{
    look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote,
};

mod bus_controller_impl;
pub(crate) use bus_controller_impl::BusController;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

/// Zero-copy shared memory region.
///
/// Uses platform-specific shared memory mechanisms:
/// - Linux: `memfd_create()` + `mmap()`
/// - macOS: `mach_make_memory_entry_64()` + `vm_map()`
/// - Windows: `CreateFileMapping()` + `MapViewOfFile()`
pub struct MemoryRegion {
    header: MappedRegion,
    buffer_size: u64,
    buffer: Option<MappedRegion>,
    obj: Object,
}

impl Drop for MemoryRegion {
    fn drop(&mut self) {
        let _ = self.ref_count_inner(-1);
    }
}

impl MemoryRegion {
    const HEADER_REFERENCE_COUNT: usize = 4;
    const HEADER_BUFFER_SIZE: usize = 8;

    const fn header_length() -> usize {
        Self::HEADER_REFERENCE_COUNT + Self::HEADER_BUFFER_SIZE
    }

    /// Create a new shared memory region of `size` bytes.
    ///
    /// Returns `None` when the operating system cannot create the region.
    ///
    /// # Panics
    ///
    /// Panics when the operating system creates the region but cannot map its header.
    #[must_use]
    pub fn new(size: usize) -> Option<Self> {
        let real_size = Self::header_length() + size;
        let obj = Self::obj_new(real_size)?;

        unsafe {
            let mut header = MappedRegion::from_object(&obj, 0, Self::header_length())
                .expect("MappedRegion::from_object");

            let rc = header_ref_count(&header);
            rc.store(1, Ordering::SeqCst);

            header
                .as_mut()
                .as_mut_ptr()
                .add(Self::HEADER_REFERENCE_COUNT)
                .cast::<u64>()
                .write_unaligned(size as u64);

            Some(Self {
                header,
                buffer_size: size as u64,
                buffer: None,
                obj,
            })
        }
    }

    /// Reconstruct a `MemoryRegion` from an `Object` (fd/handle) passed via IPC.
    ///
    /// # Panics
    ///
    /// Panics when the operating system cannot map the region header.
    #[must_use]
    pub fn from_object(obj: Object) -> Self {
        let mr = unsafe {
            let header = MappedRegion::from_object(&obj, 0, Self::header_length())
                .expect("MappedRegion::from_object");

            let buffer_size = header
                .as_slice()
                .as_ptr()
                .add(Self::HEADER_REFERENCE_COUNT)
                .cast::<u64>()
                .read_unaligned();

            Self {
                header,
                buffer_size,
                buffer: None,
                obj,
            }
        };
        let _ = mr.ref_count_inner(1);
        mr
    }

    /// Get the underlying object (fd/handle).
    #[must_use]
    pub fn object(&self) -> &Object {
        &self.obj
    }

    /// Duplicate this `MemoryRegion` (dups the underlying fd/handle, increments ref count).
    ///
    /// # Errors
    ///
    /// Returns the operating system error when the fd/handle cannot be duplicated.
    pub fn duplicate(&self) -> io::Result<Self> {
        self.object().duplicate().map(Self::from_object)
    }

    /// Map a range of the user data area (after the header).
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot map the range.
    pub fn map(&mut self, range: impl RangeBounds<usize>) -> Result<&mut [u8], Error> {
        let (offset, size) = super::util::range_to_offset_size(range);
        let size = size.unwrap_or_else(|| {
            usize::try_from(self.buffer_size - offset as u64).unwrap_or(usize::MAX)
        });
        let real_offset = Self::header_length() + offset;

        let need_remap = self
            .buffer
            .as_ref()
            .is_none_or(|buffer| real_offset != buffer.offset || size != buffer.as_slice().len());

        if need_remap {
            let buffer = unsafe { MappedRegion::from_object(&self.obj, real_offset, size)? };
            return Ok(self.buffer.insert(buffer).as_mut());
        }

        match self.buffer.as_mut() {
            Some(buffer) => Ok(buffer.as_mut()),
            // `need_remap` is true when there is no buffer, so this arm cannot run.
            None => Err(Error::MemoryRegionMapping),
        }
    }

    /// Add `val` to the atomic ref count and return the previous count.
    ///
    /// A `val` of zero only reads the count.
    #[must_use]
    pub fn ref_count_inner(&self, val: i32) -> u32 {
        let rc = header_ref_count(&self.header);

        match val.cmp(&0) {
            std::cmp::Ordering::Equal => rc.load(Ordering::SeqCst),
            std::cmp::Ordering::Greater => rc.fetch_add(val.unsigned_abs(), Ordering::SeqCst),
            std::cmp::Ordering::Less => rc.fetch_sub(val.unsigned_abs(), Ordering::SeqCst),
        }
    }

    /// Get the ref count (alias for `ref_count_inner(0)`).
    #[must_use]
    pub fn ref_count(&self) -> u32 {
        self.ref_count_inner(0)
    }

    /// Total size of the user-accessible buffer (excluding header).
    #[must_use]
    pub fn buffer_size(&self) -> u64 {
        self.buffer_size
    }
}

/// A mapped portion of a shared memory object.
///
/// Holds a `&'static mut [u8]` slice obtained from platform-specific `mmap` or `vm_map`.
/// On drop, unmaps the memory.
struct MappedRegion {
    offset: usize,
    buffer: &'static mut [u8],
}

impl Drop for MappedRegion {
    fn drop(&mut self) {
        unsafe {
            let aligned_offset = trunc_page(self.offset);
            let adjustment_for_alignment = self.offset - aligned_offset;
            Self::unmap(
                self.buffer.as_mut_ptr().sub(adjustment_for_alignment),
                adjustment_for_alignment + self.buffer.len(),
            );
        }
    }
}

impl MappedRegion {
    /// Map a portion of an object at the given offset and size.
    /// The mapping is page-aligned internally.
    ///
    /// # Safety
    ///
    /// `obj` must be a valid shared memory object. It must be at least `offset + size` bytes long.
    unsafe fn from_object(obj: &Object, offset: usize, size: usize) -> Result<Self, Error> {
        let aligned_offset = trunc_page(offset);
        let adjustment_for_alignment = offset - aligned_offset;
        let addr = Self::map(obj, aligned_offset, adjustment_for_alignment + size)?;

        Ok(Self {
            offset,
            buffer: slice::from_raw_parts_mut(addr.add(adjustment_for_alignment), size),
        })
    }

    fn as_slice(&self) -> &[u8] {
        self.buffer
    }

    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer
    }
}

/// View the reference count at the start of a region header.
///
/// The header is the first bytes of a page-aligned mapping, so the cast is aligned.
/// The count lives as long as the mapping, which the returned borrow cannot outlive.
#[allow(clippy::cast_ptr_alignment)] // The header starts at offset 0 of a page-aligned mapping.
fn header_ref_count(header: &MappedRegion) -> &AtomicU32 {
    unsafe { &*header.as_slice().as_ptr().cast::<AtomicU32>() }
}

/// Truncate a size to the nearest page boundary.
#[inline]
fn trunc_page(size: usize) -> usize {
    size & !(page_mask())
}
