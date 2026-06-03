/// macOS IPC transport — Mach ports (stub).
///
/// Full implementation would use `mach_msg`, `mach_port` operations,
/// `mach_make_memory_entry_64` for shared memory, and `EVFILT_MACHPORT`
/// for IO multiplexing.

use std::io;
use std::os::fd::BorrowedFd;

use crate::ipc::errors::IpcError;

/// Platform-native kernel object. On macOS, this is a mach_port_t.
pub struct Object {
    port: u32,
}

impl Object {
    pub unsafe fn from_raw(raw: u32) -> Self {
        Self { port: raw }
    }

    pub fn into_raw(self) -> u32 {
        self.port
    }

    pub fn as_raw(&self) -> u32 {
        self.port
    }
}

pub struct Fd {
    _private: (),
}

pub struct Remote {
    _private: (),
}

pub struct Local {
    _private: (),
}

pub struct EncodedMessage {
    _private: (),
}

impl EncodedMessage {
    pub fn send(&self, _remote: &Remote) -> Result<(), IpcError> {
        Err(IpcError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on macOS",
        )))
    }

    pub fn recv(_sock_fd: BorrowedFd<'_>) -> Result<(Vec<u8>, Vec<std::os::fd::OwnedFd>), IpcError> {
        Err(IpcError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on macOS",
        )))
    }

    pub fn decode(_bytes: &[u8]) -> Result<(u32, &[u8], &[u8]), IpcError> {
        Err(IpcError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on macOS",
        )))
    }
}

pub struct IoMultiplexing {
    _private: (),
}

impl IoMultiplexing {
    pub fn new() -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on macOS",
        ))
    }
}

pub struct MemoryRegion {
    _private: (),
}

impl MemoryRegion {
    pub fn new(_size: usize) -> Option<Self> {
        None
    }

    pub fn map(&mut self, _range: impl std::ops::RangeBounds<usize>) -> &mut [u8] {
        &mut []
    }

    pub fn buffer_size(&self) -> u64 {
        0
    }

    pub fn ref_count(&self) -> u32 {
        0
    }

    pub fn inc_ref(&self) {}

    pub fn dec_ref(&self) -> u32 {
        0
    }
}
