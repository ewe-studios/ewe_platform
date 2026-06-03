/// Windows IPC transport — Named pipes (stub).
///
/// Full implementation would use `CreateNamedPipe`, `ConnectNamedPipe`,
/// `CreateFileMapping`/`MapViewOfFile` for shared memory, and IOCP
/// for IO multiplexing.

use std::io;
use std::os::fd::BorrowedFd;

use crate::ipc::errors::IpcError;

/// Platform-native kernel object. On Windows, this is a HANDLE.
pub struct Object {
    handle: usize,
}

impl Object {
    pub unsafe fn from_raw(raw: usize) -> Self {
        Self { handle: raw }
    }

    pub fn into_raw(self) -> usize {
        self.handle
    }

    pub fn as_raw(&self) -> usize {
        self.handle
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
            "IPC not implemented on Windows",
        )))
    }

    pub fn recv(_sock_fd: BorrowedFd<'_>) -> Result<(Vec<u8>, Vec<std::os::fd::OwnedFd>), IpcError> {
        Err(IpcError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on Windows",
        )))
    }

    pub fn decode(_bytes: &[u8]) -> Result<(u32, &[u8], &[u8]), IpcError> {
        Err(IpcError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not implemented on Windows",
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
            "IPC not implemented on Windows",
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
