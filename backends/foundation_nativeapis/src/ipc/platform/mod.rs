/// Platform-specific IPC transports.
///
/// | Platform | Transport | Mechanism |
/// |----------|-----------|-----------|
/// | Linux | Unix domain sockets (`SOCK_SEQPACKET`) | Abstract socket addresses, `SCM_RIGHTS` for FD passing |
/// | macOS | Mach ports | `mach_msg`, `mach_port` operations |
/// | Windows | Named pipes | `CreateNamedPipe`, `ConnectNamedPipe` |

use std::{
    io, mem,
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
pub(crate) use self::linux::{look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote, BusController};
#[cfg(target_os = "macos")]
pub(crate) use self::macos::{look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote};
#[cfg(target_os = "windows")]
pub(crate) use self::windows::{look_up, page_mask, register, EncodedMessage, IoHub, IoMultiplexing, Remote};

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
        self.ref_count_inner(-1);
    }
}

impl MemoryRegion {
    const HEADER_REFERENCE_COUNT: usize = 4;
    const HEADER_BUFFER_SIZE: usize = 8;

    const fn header_length() -> usize {
        Self::HEADER_REFERENCE_COUNT + Self::HEADER_BUFFER_SIZE
    }

    /// Create a new shared memory region of `size` bytes.
    pub fn new(size: usize) -> Option<Self> {
        let real_size = Self::header_length() + size;
        let obj = Self::obj_new(real_size)?;

        unsafe {
            let mut header = MappedRegion::from_object(&obj, 0, Self::header_length())
                .expect("MappedRegion::from_object");

            let rc: &AtomicU32 = mem::transmute(header.as_slice().as_ptr());
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

    /// Reconstruct a MemoryRegion from an Object (fd/handle) passed via IPC.
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
        mr.ref_count_inner(1);
        mr
    }

    /// Get the underlying object (fd/handle).
    pub fn object(&self) -> &Object {
        &self.obj
    }

    /// Clone this MemoryRegion (dups the underlying fd/handle, increments ref count).
    pub fn clone(&self) -> io::Result<Self> {
        self.object().clone().map(Self::from_object)
    }

    /// Map a range of the user data area (after the header).
    pub fn map(&mut self, range: impl RangeBounds<usize>) -> Result<&mut [u8], Error> {
        let (offset, size) = super::util::range_to_offset_size(range);
        let size = size.unwrap_or_else(|| {
            usize::try_from(self.buffer_size - offset as u64).unwrap_or(usize::MAX)
        });
        let real_offset = Self::header_length() + offset;

        let need_remap = self
            .buffer
            .as_ref()
            .map(|buffer| real_offset != buffer.offset || size != buffer.as_slice().len())
            .unwrap_or(true);

        if need_remap {
            let buffer = unsafe { MappedRegion::from_object(&self.obj, real_offset, size)? };
            self.buffer = Some(buffer);
        }

        Ok(self.buffer.as_mut().unwrap().as_mut())
    }

    /// Get the atomic ref count.
    pub fn ref_count_inner(&self, val: i32) -> u32 {
        let rc: &AtomicU32 = unsafe { mem::transmute(self.header.as_slice().as_ptr()) };

        if val == 0 {
            rc.load(Ordering::SeqCst)
        } else if val > 0 {
            rc.fetch_add(val as u32, Ordering::SeqCst)
        } else {
            rc.fetch_sub(-val as u32, Ordering::SeqCst)
        }
    }

    /// Get the ref count (alias for `ref_count_inner(0)`).
    pub fn ref_count(&self) -> u32 {
        self.ref_count_inner(0)
    }

    /// Total size of the user-accessible buffer (excluding header).
    pub fn buffer_size(&self) -> u64 {
        self.buffer_size
    }
}

/// A mapped portion of a shared memory object.
///
/// Holds a `&'static mut [u8]` slice obtained from platform-specific mmap/vm_map.
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
    unsafe fn from_object(obj: &Object, offset: usize, size: usize) -> Result<Self, Error> {
        let aligned_offset = trunc_page(offset);
        let adjustment_for_alignment = offset - aligned_offset;
        let addr = Self::map(obj, aligned_offset, adjustment_for_alignment + size)?;

        Ok(Self {
            offset,
            buffer: slice::from_raw_parts_mut(
                addr.add(adjustment_for_alignment),
                size,
            ),
        })
    }

    fn as_slice(&self) -> &[u8] {
        self.buffer
    }

    fn as_mut(&mut self) -> &mut [u8] {
        self.buffer
    }
}

/// Truncate a size to the nearest page boundary.
#[inline]
fn trunc_page(size: usize) -> usize {
    size & !(page_mask())
}
