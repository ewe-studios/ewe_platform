/// Windows IPC transport — named pipes with IOCP, handle duplication for object passing.
///
/// Uses named pipes (message mode, overlapped I/O) for communication,
/// `CreateFileMapping` and `MapViewOfFile` for shared memory, and `DuplicateHandle`
/// for passing kernel objects across processes.
use std::{
    io, mem, ptr,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender, TryRecvError},
        Arc, OnceLock,
    },
    time::Duration,
};

use foundation_context::Ctx;

use crate::ipc::{
    decode, util::Align4, version::version, version::Version, EndpointID, Error, Label, LabelOp,
    MemoryRegion, Message, MessageBox, Selector,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, DuplicateHandle, GetLastError, DUPLICATE_SAME_ACCESS, ERROR_ACCESS_DENIED,
        ERROR_ALREADY_EXISTS, ERROR_BROKEN_PIPE, ERROR_FILE_NOT_FOUND, ERROR_INVALID_PARAMETER,
        ERROR_PIPE_BUSY, ERROR_PIPE_NOT_CONNECTED, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    },
    Storage::FileSystem::{
        CreateFileW, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        FILE_GENERIC_WRITE, OPEN_EXISTING, PIPE_ACCESS_INBOUND, SYNCHRONIZE,
    },
    System::{
        Memory::{
            CreateFileMappingW, MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
            MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READWRITE,
        },
        Pipes::{
            CreateNamedPipeW, GetNamedPipeServerProcessId, PeekNamedPipe, PIPE_READMODE_MESSAGE,
            PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
        },
        SystemInformation::GetSystemInfo,
        Threading::{
            GetCurrentProcess, OpenProcess, WaitForSingleObject, PROCESS_DUP_HANDLE,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    },
};

// On non-Windows, provide stub types so the module parses but is never compiled.
#[cfg(not(target_os = "windows"))]
type HANDLE = isize;

/// Opaque handle wrapper with ownership semantics.
#[derive(Debug)]
pub struct Handle(isize);

impl Handle {
    /// Duplicate this handle into the current process with `DuplicateHandle`.
    ///
    /// The result is an independent handle with the same access. It is not a `Clone`,
    /// because the duplication can fail.
    ///
    /// # Errors
    ///
    /// Returns the operating system error when the handle cannot be duplicated.
    /// Returns [`io::ErrorKind::Unsupported`] on targets other than Windows.
    pub fn duplicate(&self) -> io::Result<Self> {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut dup: HANDLE = ptr::null_mut();
            let r = DuplicateHandle(
                GetCurrentProcess(),
                self.0 as HANDLE,
                GetCurrentProcess(),
                &raw mut dup,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            );
            if r == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(dup as isize))
        }
        #[cfg(not(target_os = "windows"))]
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }

    /// Take ownership of a raw handle.
    ///
    /// # Safety
    ///
    /// `raw` must be a valid handle that no other owner closes.
    #[must_use]
    pub unsafe fn from_raw(raw: isize) -> Self {
        Self(raw)
    }

    /// Give up ownership and return the raw handle. The caller must close it.
    #[must_use]
    pub fn into_raw(self) -> isize {
        let raw = self.0;
        mem::forget(self);
        raw
    }

    /// The raw handle. This `Handle` keeps the ownership.
    #[must_use]
    pub fn as_raw(&self) -> isize {
        self.0
    }
}

#[cfg(target_os = "windows")]
impl Drop for Handle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != -1 {
            unsafe {
                CloseHandle(self.0 as HANDLE);
            }
        }
    }
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

pub type Object = Handle;

/// The write end of a named pipe, with an optional handle to the peer process.
#[derive(Debug)]
pub struct Remote {
    pipe: Handle,
    process: Option<Handle>,
}

impl PartialEq for Remote {
    fn eq(&self, other: &Self) -> bool {
        self.pipe.0 == other.pipe.0
    }
}

impl Remote {
    /// Wrap a pipe handle as a remote end, with no peer process handle.
    #[must_use]
    pub fn new(pipe: Handle) -> Self {
        Self {
            pipe,
            process: None,
        }
    }

    /// Check if the peer is gone, without writing to the pipe.
    ///
    /// A write of any size can put a message into a message-mode pipe, so this probe
    /// never writes. It runs these checks in order:
    ///
    /// 1. `PeekNamedPipe` with no buffer. It fails with `ERROR_BROKEN_PIPE` when the
    ///    other end of the pipe is closed. The call needs read access to the handle.
    ///    On a write-only handle it fails with `ERROR_ACCESS_DENIED`, and the probe
    ///    goes on to the next check.
    /// 2. `WaitForSingleObject` with a zero timeout on the peer process handle, when
    ///    this remote holds one. It reports a process that has exited.
    /// 3. The process id of the pipe peer from `GetNamedPipeServerProcessId`, opened
    ///    with `OpenProcess` and checked with a zero timeout wait. A process id that
    ///    no process owns means the peer is gone.
    ///
    /// A check that cannot decide, for example because of an access error, counts as
    /// alive. A later `send` finds a dead pipe and removes the endpoint.
    #[must_use]
    pub fn is_dead(&self) -> bool {
        #[cfg(target_os = "windows")]
        unsafe {
            let pipe = self.pipe.0 as HANDLE;

            // 1. A closed peer end of a readable pipe handle.
            if PeekNamedPipe(
                pipe,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            ) == 0
                && matches!(GetLastError(), ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED)
            {
                return true;
            }

            // 2. The peer process handle that this remote holds.
            if let Some(process) = &self.process {
                if WaitForSingleObject(process.0 as HANDLE, 0) == WAIT_OBJECT_0 {
                    return true;
                }
            }

            // 3. The process that owns the other end of the pipe.
            let mut peer_pid = 0u32;
            if GetNamedPipeServerProcessId(pipe, &raw mut peer_pid) == 0 {
                return matches!(GetLastError(), ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED);
            }
            let peer = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, peer_pid);
            if peer.is_null() {
                // No process has this id any more.
                return GetLastError() == ERROR_INVALID_PARAMETER;
            }
            let exited = WaitForSingleObject(peer, 0) == WAIT_OBJECT_0;
            CloseHandle(peer);
            exited
        }
        #[cfg(not(target_os = "windows"))]
        false
    }
}

/// The read end of a named pipe.
pub struct Local {
    pipe: Handle,
}

/// The size of the version and object count fields at the start of a pipe message.
const HEADER_LEN: usize = 8;
/// The size of one object slot in a pipe message.
const OBJECT_SLOT_LEN: usize = 8;

/// Read a native-endian `u32` at `offset`, or fail with a decode error when the message is short.
fn read_u32_at(buf: &[u8], offset: usize) -> Result<u32, Error> {
    offset
        .checked_add(4)
        .and_then(|end| buf.get(offset..end))
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map(u32::from_ne_bytes)
        .ok_or(Error::Decode(bincode::error::DecodeError::UnexpectedEnd {
            additional: 4,
        }))
}

/// Read a native-endian `u64` at `offset`, or fail with a decode error when the message is short.
fn read_u64_at(buf: &[u8], offset: usize) -> Result<u64, Error> {
    offset
        .checked_add(8)
        .and_then(|end| buf.get(offset..end))
        .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
        .map(u64::from_ne_bytes)
        .ok_or(Error::Decode(bincode::error::DecodeError::UnexpectedEnd {
            additional: 8,
        }))
}

/// Write a native-endian `u32` at `offset`. The caller sized the buffer for it.
fn write_u32_at(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

/// Write a native-endian `u64` at `offset`. The caller sized the buffer for it.
#[cfg(target_os = "windows")]
fn write_u64_at(buf: &mut [u8], offset: usize, value: u64) {
    buf[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
}

/// Convert a length to the `u32` of the wire format.
fn wire_len(len: usize) -> u32 {
    u32::try_from(len).expect("a pipe message field is at most u32::MAX bytes long")
}

pub(crate) struct EncodedMessage {
    pub selector: Selector,
    pub payload_data: &'static [u8],
    pipe_msg: Vec<u8>,
    msg_size: usize,
    pub objects: Vec<Handle>,
    pub memory_regions: Vec<MemoryRegion>,
}

impl EncodedMessage {
    pub fn extract_remote(&mut self) -> Option<Remote> {
        if self.objects.len() < 2 {
            return None;
        }
        let pipe = self.objects.pop()?;
        let process = self.objects.pop()?;
        Some(Remote {
            pipe,
            process: Some(process),
        })
    }

    pub fn send(&mut self, remote: &Remote) -> Result<(), Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            for r in &self.memory_regions {
                let _ = r.ref_count_inner(1);
            }

            // Duplicate handles into remote process
            if let Some(ref process) = remote.process {
                let all_objects: Vec<&Handle> = self
                    .objects
                    .iter()
                    .chain(self.memory_regions.iter().map(MemoryRegion::object))
                    .collect();

                for (i, obj) in all_objects.iter().enumerate() {
                    let mut dup: HANDLE = ptr::null_mut();
                    let r = DuplicateHandle(
                        GetCurrentProcess(),
                        obj.as_raw() as HANDLE,
                        process.as_raw() as HANDLE,
                        &raw mut dup,
                        0,
                        0,
                        DUPLICATE_SAME_ACCESS,
                    );
                    if r == 0 {
                        for r in &self.memory_regions {
                            let _ = r.ref_count_inner(-1);
                        }
                        return Err(Error::Disconnect);
                    }
                    write_u64_at(
                        &mut self.pipe_msg,
                        HEADER_LEN + i * OBJECT_SLOT_LEN,
                        dup as usize as u64,
                    );
                }
            }

            let mut written = 0u32;
            let r = WriteFile(
                remote.pipe.0 as HANDLE,
                self.pipe_msg.as_ptr().cast(),
                wire_len(self.msg_size),
                &raw mut written,
                ptr::null_mut(),
            );
            if r == 0 {
                for r in &self.memory_regions {
                    let _ = r.ref_count_inner(-1);
                }
                return Err(Error::Disconnect);
            }
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::IoError(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows only",
        )))
    }

    pub fn from_local(pipe: &mut Local) -> Result<Self, Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            /// The size of the receive buffer: one message must fit in it.
            const READ_BUFFER_LEN: u32 = 64 << 10;
            let mut buf = vec![0u8; READ_BUFFER_LEN as usize];
            let mut bytes_read = 0u32;
            let r = ReadFile(
                pipe.pipe.0 as HANDLE,
                buf.as_mut_ptr().cast(),
                READ_BUFFER_LEN,
                &raw mut bytes_read,
                ptr::null_mut(),
            );
            if r == 0 {
                return Err(Error::Disconnect);
            }
            let msg_size = bytes_read as usize;
            buf.truncate(msg_size);
            Self::new(buf, msg_size)
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::IoError(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows only",
        )))
    }

    fn new(pipe_msg: Vec<u8>, msg_size: usize) -> Result<Self, Error> {
        let short = || {
            Error::Decode(bincode::error::DecodeError::UnexpectedEnd {
                additional: HEADER_LEN,
            })
        };
        if msg_size < HEADER_LEN {
            return Err(short());
        }

        // Version
        let version_val = read_u32_at(&pipe_msg, 0)?;
        let [magic, major, minor, patch] = u32::to_ne_bytes(version_val);
        if magic != 0xFF {
            return Err(Error::VersionMismatch(Version::new(), None));
        }
        let remote_version = Version::from_parts(major, minor, patch);
        if !version().compatible(remote_version) {
            return Err(Error::VersionMismatch(remote_version, None));
        }

        // Object count
        let object_count = read_u32_at(&pipe_msg, 4)? as usize;
        let objects_end = object_count
            .checked_mul(OBJECT_SLOT_LEN)
            .and_then(|len| len.checked_add(HEADER_LEN))
            .filter(|end| *end <= pipe_msg.len())
            .ok_or_else(short)?;

        let mut objects: Vec<Handle> = Vec::with_capacity(object_count);
        for i in 0..object_count {
            let val = read_u64_at(&pipe_msg, HEADER_LEN + i * OBJECT_SLOT_LEN)?;
            let raw = isize::try_from(val.cast_signed()).map_err(|_| short())?;
            // The sender duplicated this handle into this process, so this process owns it.
            objects.push(unsafe { Handle::from_raw(raw) });
        }

        // Selector
        let selector_size = read_u32_at(&pipe_msg, objects_end)? as usize;
        let selector_start = objects_end + 4;
        let selector_data = selector_start
            .checked_add(selector_size)
            .and_then(|end| pipe_msg.get(selector_start..end))
            .ok_or_else(short)?;
        let selector: Selector = decode(selector_data)?;

        // Split memory regions from objects
        let mr_count = usize::from(selector.memory_region_count);
        let mr_start = objects.len().checked_sub(mr_count).ok_or_else(short)?;
        let memory_regions: Vec<_> = objects
            .drain(mr_start..)
            .map(|obj| {
                let r = MemoryRegion::from_object(obj);
                let _ = r.ref_count_inner(-1);
                r
            })
            .collect();

        // Payload
        let payload_offset = selector_start + selector_size.align4();
        let payload_size = read_u32_at(&pipe_msg, payload_offset)? as usize;
        let payload_start = payload_offset + 4;
        let payload_slice = payload_start
            .checked_add(payload_size)
            .and_then(|end| pipe_msg.get(payload_start..end))
            .ok_or_else(short)?;
        // The bytes live in the heap buffer of `pipe_msg`, which this struct owns and never
        // resizes, so the slice stays valid as long as the struct.
        let payload_data: &'static [u8] =
            unsafe { std::slice::from_raw_parts(payload_slice.as_ptr(), payload_size) };

        Ok(Self {
            selector,
            payload_data,
            pipe_msg,
            msg_size,
            objects,
            memory_regions,
        })
    }
}

impl<T: MessageBox> Message<T> {
    /// Encode this message. Returns the buffer and the position and length of the payload.
    fn encode_inner_win(&self) -> (Vec<u8>, usize, usize) {
        let selector_data =
            bincode::serde::encode_to_vec(&self.selector, bincode::config::standard())
                .expect("selector encode");
        let payload_data = self.payload.encode().expect("payload encode");

        let object_count = self.objects.len() + self.memory_regions.len();
        let msg_size = HEADER_LEN
            + object_count * OBJECT_SLOT_LEN
            + 4 /* selector_size */ + selector_data.len().align4()
            + 4 /* payload_size */ + payload_data.len().align4();

        let mut buf = vec![0u8; msg_size];

        // Version
        let v = version();
        write_u32_at(
            &mut buf,
            0,
            u32::from_ne_bytes([0xFF, v.major(), v.minor(), v.patch()]),
        );

        // Object count
        write_u32_at(&mut buf, 4, wire_len(object_count));

        // Object slots (filled by send with DuplicateHandle values)
        // Left as zeros — send() will write the duplicated handles

        let data_start = HEADER_LEN + object_count * OBJECT_SLOT_LEN;

        // Selector
        write_u32_at(&mut buf, data_start, wire_len(selector_data.len()));
        buf[data_start + 4..data_start + 4 + selector_data.len()].copy_from_slice(&selector_data);

        // Payload
        let payload_offset = data_start + 4 + selector_data.len().align4();
        write_u32_at(&mut buf, payload_offset, wire_len(payload_data.len()));
        let payload_start = payload_offset + 4;
        buf[payload_start..payload_start + payload_data.len()].copy_from_slice(&payload_data);

        (buf, payload_start, payload_data.len())
    }

    pub(crate) fn into_encoded(mut self) -> EncodedMessage {
        self.selector.memory_region_count = u16::try_from(self.memory_regions.len())
            .expect("a message carries at most 65535 memory regions");
        let (pipe_msg, payload_start, payload_len) = self.encode_inner_win();
        let msg_size = pipe_msg.len();
        // The bytes live in the heap buffer of `pipe_msg`, which the message owns and never
        // resizes, so the slice stays valid as long as the message.
        let payload_data: &'static [u8] = unsafe {
            std::slice::from_raw_parts(pipe_msg.as_ptr().add(payload_start), payload_len)
        };
        EncodedMessage {
            selector: self.selector,
            payload_data,
            pipe_msg,
            msg_size,
            objects: self.objects,
            memory_regions: self.memory_regions,
        }
    }
}

pub(crate) struct IoHub {
    bus_receiver: Option<Receiver<EncodedMessage>>,
    local: Option<Local>,
    im: Arc<IoMultiplexing>,
}

impl IoHub {
    fn for_bus_controller(
        _listener: Handle,
        bus_receiver: Receiver<EncodedMessage>,
        im: Arc<IoMultiplexing>,
    ) -> Self {
        Self {
            bus_receiver: Some(bus_receiver),
            local: None,
            im,
        }
    }

    fn for_endpoint(local: Local, im: Arc<IoMultiplexing>) -> Self {
        Self {
            bus_receiver: None,
            local: Some(local),
            im,
        }
    }

    pub fn recv(
        &mut self,
        timeout: Option<Duration>,
        _remote: Option<&Remote>,
        ctx: &Ctx,
    ) -> Result<EncodedMessage, Error> {
        let end = timeout.map(|t| ctx.clock().now() + t);

        loop {
            if let Some(rx) = &self.bus_receiver {
                match rx.try_recv() {
                    Ok(msg) => return Ok(msg),
                    Err(TryRecvError::Disconnected) => {
                        self.bus_receiver = None;
                    }
                    Err(TryRecvError::Empty) => {}
                }
            }

            if let Some(local) = &mut self.local {
                match EncodedMessage::from_local(local) {
                    Ok(msg) => return Ok(msg),
                    Err(Error::Timeout) => {}
                    Err(e) => {
                        self.local = None;
                        if self.bus_receiver.is_none() {
                            return Err(e);
                        }
                    }
                }
            }

            if self.bus_receiver.is_none() && self.local.is_none() {
                return Err(Error::Disconnect);
            }

            let poll = Duration::from_millis(200);
            let wait = if let Some(end) = end {
                let remaining = end.saturating_duration_since(ctx.clock().now());
                if remaining.is_zero() {
                    return Err(Error::Timeout);
                }
                remaining.min(poll)
            } else {
                poll
            };
            if let Some(rx) = &self.bus_receiver {
                match rx.recv_timeout(wait) {
                    Ok(msg) => return Ok(msg),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => self.bus_receiver = None,
                }
            }
        }
    }

    /// The IO multiplexing instance that this hub belongs to.
    #[must_use]
    pub fn io_multiplexing(&self) -> Arc<IoMultiplexing> {
        self.im.clone()
    }
}

pub(crate) struct IoMultiplexing {
    _private: (),
}

impl IoMultiplexing {
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }

    // Named pipe reads block in their own call, so there is nothing to wake. The method keeps
    // the same shape as the other platforms.
    #[allow(clippy::unused_self)]
    pub fn wake(&self) {}
}

impl Default for IoMultiplexing {
    fn default() -> Self {
        Self::new()
    }
}

fn pipe_name(identifier: &str) -> Vec<u16> {
    let name = format!("\\\\.\\pipe\\{identifier}");
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn look_up(
    identifier: &str,
    label: Label,
    token: String,
    im: Arc<IoMultiplexing>,
    ctx: &Ctx,
) -> Result<(IoHub, Remote, EndpointID), Error> {
    #[cfg(target_os = "windows")]
    unsafe {
        let name = pipe_name(identifier);

        let pipe = CreateFileW(
            name.as_ptr(),
            FILE_GENERIC_WRITE,
            0,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if pipe == INVALID_HANDLE_VALUE {
            return match GetLastError() {
                ERROR_FILE_NOT_FOUND | ERROR_PIPE_BUSY => Err(Error::IdentifierNotInUse),
                ERROR_ACCESS_DENIED => Err(Error::PermissionDenied),
                _ => Err(Error::Unknown),
            };
        }
        let remote_pipe = Handle(pipe as isize);

        let mut server_pid = 0u32;
        GetNamedPipeServerProcessId(pipe, &raw mut server_pid);

        let process = OpenProcess(PROCESS_DUP_HANDLE, 0, server_pid);
        let remote_process = if process.is_null() {
            None
        } else {
            Some(Handle(process as isize))
        };

        let remote = Remote {
            pipe: remote_pipe,
            process: remote_process,
        };

        // Create reply pipe pair for handshake
        let reply_name = format!("\\\\.\\pipe\\{identifier}.reply.{}", uuid::Uuid::new_v4());
        let reply_name_w: Vec<u16> = reply_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let read_pipe = CreateNamedPipeW(
            reply_name_w.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            4 << 20,
            4 << 20,
            0,
            ptr::null(),
        );
        if read_pipe == INVALID_HANDLE_VALUE {
            return Err(Error::Unknown);
        }
        let read_handle = Handle(read_pipe as isize);

        let write_pipe = CreateFileW(
            reply_name_w.as_ptr(),
            FILE_GENERIC_WRITE,
            0,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if write_pipe == INVALID_HANDLE_VALUE {
            return Err(Error::Unknown);
        }
        let write_handle = Handle(write_pipe as isize);

        // Send ConnectMessage with process handle + write pipe handle
        let mut msg = Message::new(
            Selector::unicast(LabelOp::True),
            crate::ipc::ConnectMessage {
                version: version(),
                token,
                label,
            },
        );
        let cur_process = Handle(GetCurrentProcess() as isize);
        msg.objects.push(Handle(cur_process.into_raw()));
        msg.objects.push(write_handle);

        let mut encoded = msg.into_encoded();
        encoded.send(&remote)?;

        let local = Local { pipe: read_handle };
        let mut io_hub = IoHub::for_endpoint(local, im);
        let ack_msg = io_hub.recv(Some(Duration::from_secs(2)), Some(&remote), ctx)?;
        let ack =
            crate::ipc::ConnectMessageAck::decode(ack_msg.selector.uuid, ack_msg.payload_data)?;

        match ack {
            crate::ipc::ConnectMessageAck::Ok(endpoint_id) => Ok((io_hub, remote, endpoint_id)),
            crate::ipc::ConnectMessageAck::ErrVersion(v) => Err(Error::VersionMismatch(v, None)),
            crate::ipc::ConnectMessageAck::ErrToken => Err(Error::TokenMismatch),
        }
    }
    #[cfg(not(target_os = "windows"))]
    Err(Error::IoError(io::Error::new(
        io::ErrorKind::Unsupported,
        "Windows only",
    )))
}

pub(crate) fn register(
    identifier: &str,
    im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Sender<EncodedMessage>, EndpointID), Error> {
    #[cfg(target_os = "windows")]
    unsafe {
        let name = pipe_name(identifier);
        let pipe = CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            4 << 20,
            4 << 20,
            0,
            ptr::null(),
        );
        if pipe == INVALID_HANDLE_VALUE {
            return match GetLastError() {
                ERROR_ALREADY_EXISTS => Err(Error::IdentifierInUse),
                ERROR_ACCESS_DENIED => Err(Error::PermissionDenied),
                _ => Err(Error::Unknown),
            };
        }
        let handle = Handle(pipe as isize);
        let (bus_sender, bus_receiver) = mpsc::channel();
        Ok((
            IoHub::for_bus_controller(handle, bus_receiver, im),
            bus_sender,
            EndpointID::new(),
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let (bus_sender, bus_receiver) = mpsc::channel();
        Ok((
            IoHub::for_bus_controller(Handle(0), bus_receiver, im),
            bus_sender,
            EndpointID::new(),
        ))
    }
}

// Shared memory
impl MemoryRegion {
    pub(crate) fn obj_new(size: usize) -> Option<Object> {
        #[cfg(target_os = "windows")]
        unsafe {
            let size = size as u64;
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE,
                ptr::null(),
                PAGE_READWRITE,
                (size >> 32) as u32,
                (size & 0xFFFF_FFFF) as u32,
                ptr::null(),
            );
            if handle.is_null() {
                return None;
            }
            Some(Handle(handle as isize))
        }
        #[cfg(not(target_os = "windows"))]
        None
    }
}

impl crate::ipc::platform::MappedRegion {
    pub(crate) fn map(
        obj: &Object,
        aligned_offset: usize,
        aligned_size: usize,
    ) -> Result<*mut u8, Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            let offset = aligned_offset as u64;
            let ptr = MapViewOfFile(
                obj.as_raw() as HANDLE,
                FILE_MAP_READ | FILE_MAP_WRITE,
                (offset >> 32) as u32,
                (offset & 0xFFFF_FFFF) as u32,
                aligned_size,
            );
            if ptr.Value.is_null() {
                Err(Error::MemoryRegionMapping)
            } else {
                Ok(ptr.Value.cast::<u8>())
            }
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::MemoryRegionMapping)
    }

    pub(crate) fn unmap(addr: *mut u8, _len: usize) {
        #[cfg(target_os = "windows")]
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: addr.cast() });
        }
    }
}

static PAGE_MASK: OnceLock<usize> = OnceLock::new();

pub(crate) fn page_mask() -> usize {
    #[cfg(target_os = "windows")]
    {
        *PAGE_MASK.get_or_init(|| unsafe {
            let mut info = mem::zeroed();
            GetSystemInfo(&raw mut info);
            info.dwAllocationGranularity as usize - 1
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = &PAGE_MASK;
        0xFFF
    }
}
