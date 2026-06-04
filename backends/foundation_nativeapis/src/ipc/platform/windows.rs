/// Windows IPC transport — Named pipes (stub — full implementation requires windows-sys bindings).
///
/// Uses named pipes for communication and CreateFileMapping/MapViewOfFile for shared memory.

use std::{
    io, mem, sync::{mpsc, Arc, mpsc::{Receiver, Sender, TryRecvError}},
    time::{Duration, Instant},
};

use crate::ipc::{
    version, EndpointID, Error, Label, MemoryRegion, Selector,
};

pub type Object = Handle;
pub type Handle = usize;

#[derive(Debug, PartialEq)]
pub struct Remote {
    pipe: Handle,
    process: Option<Handle>,
}

impl Remote {
    pub fn new(pipe: Handle) -> Self {
        Self { pipe, process: None }
    }

    pub fn lock(&self) -> RemoteLock<'_> {
        RemoteLock { handle: self.pipe }
    }

    pub fn is_dead(&self) -> bool {
        // Compatible with Windows 7: 0-byte write to broken pipe returns false
        // Stub: always alive for now
        false
    }
}

pub struct RemoteLock<'a> {
    handle: &'a Handle,
}

impl RemoteLock<'_> {
    pub fn as_raw(&self) -> Handle {
        *self.handle
    }
}

pub struct Local {
    _private: (),
}

pub(crate) struct EncodedMessage {
    pub selector: Selector,
    pub payload_data: Vec<u8>,
    pub objects: Vec<Handle>,
    pub memory_regions: Vec<MemoryRegion>,
}

impl EncodedMessage {
    pub fn extract_remote(&mut self) -> Option<Remote> {
        let reply = self.objects.pop()?;
        Some(Remote::new(reply))
    }

    pub fn send(&mut self, _remote: &Remote) -> Result<(), Error> {
        Err(Error::IoError(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not yet implemented on Windows",
        )))
    }

    pub fn from_local(_local: &mut Local) -> Result<Self, Error> {
        Err(Error::IoError(io::Error::new(
            io::ErrorKind::Unsupported,
            "IPC not yet implemented on Windows",
        )))
    }
}

pub(crate) struct IoHub {
    bus_receiver: Option<Receiver<EncodedMessage>>,
    im: Arc<IoMultiplexing>,
}

impl IoHub {
    fn for_bus_controller(_listener: Handle, bus_receiver: Receiver<EncodedMessage>, im: Arc<IoMultiplexing>) -> Self {
        Self { bus_receiver: Some(bus_receiver), im }
    }

    fn for_endpoint(_local: Local, im: Arc<IoMultiplexing>) -> Self {
        Self { bus_receiver: None, im }
    }

    pub fn recv(&mut self, timeout: Option<Duration>, _remote: Option<&Remote>) -> Result<EncodedMessage, Error> {
        if let Some(rx) = &self.bus_receiver {
            match rx.try_recv() {
                Ok(msg) => return Ok(msg),
                Err(TryRecvError::Disconnected) => { self.bus_receiver = None; }
                _ => {}
            }
        }

        if self.bus_receiver.is_none() {
            return Err(Error::Disconnect);
        }

        if let Some(timeout) = timeout {
            if let Some(rx) = &self.bus_receiver {
                match rx.recv_timeout(timeout) {
                    Ok(msg) => return Ok(msg),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Err(Error::Timeout),
                    Err(_) => return Err(Error::Disconnect),
                }
            }
        }

        Err(Error::Timeout)
    }

    pub fn io_multiplexing(&self) -> Arc<IoMultiplexing> {
        self.im.clone()
    }
}

pub(crate) struct IoMultiplexing {
    _private: (),
}

impl IoMultiplexing {
    pub fn new() -> Self {
        Self { _private: () }
    }

    pub fn wake(&self) {}
}

pub(crate) fn look_up(
    _identifier: &str,
    _label: Label,
    _token: String,
    _im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Remote, EndpointID), Error> {
    Err(Error::IoError(io::Error::new(
        io::ErrorKind::Unsupported,
        "IPC not yet implemented on Windows",
    )))
}

pub(crate) fn register(
    _identifier: &str,
    im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Sender<EncodedMessage>, EndpointID), Error> {
    let (bus_sender, bus_receiver) = mpsc::channel();
    // Stub: use a dummy handle
    Ok((
        IoHub::for_bus_controller(0, bus_receiver, im),
        bus_sender,
        EndpointID::new(),
    ))
}

impl MemoryRegion {
    pub(crate) fn obj_new(_size: usize) -> Option<Object> {
        None // Requires CreateFileMapping
    }
}

impl crate::ipc::platform::MappedRegion {
    pub(crate) fn map(_obj: &Object, _aligned_offset: usize, _aligned_size: usize) -> Result<*mut u8, Error> {
        Err(Error::MemoryRegionMapping)
    }

    pub(crate) fn unmap(_addr: *mut u8, _len: usize) {
        unreachable!()
    }
}

pub(crate) fn page_mask() -> usize {
    0xFFF // 4KB pages (typical)
}

impl Object {
    pub fn clone(&self) -> io::Result<Self> {
        // Requires DuplicateHandle
        Err(io::Error::new(io::ErrorKind::Unsupported, "Object::clone not implemented on Windows"))
    }

    pub unsafe fn from_raw(raw: Handle) -> Self {
        raw
    }

    pub fn as_raw(&self) -> Handle {
        *self
    }
}
