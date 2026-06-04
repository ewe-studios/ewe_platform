/// Windows IPC transport — Named pipes with IOCP, handle duplication for object passing.
///
/// Uses named pipes (message mode, overlapped I/O) for communication,
/// CreateFileMapping/MapViewOfFile for shared memory, and DuplicateHandle
/// for passing kernel objects across processes.

use std::{
    io, mem, ptr, sync::{mpsc, Arc, Once, mpsc::{Receiver, Sender, TryRecvError}},
    time::{Duration, Instant},
};

use foundation_core::type_uuid::TypeUuid;

use crate::ipc::{
    decode, version::version, version::Version,
    EndpointID, Error, Label, LabelOp, Message, MessageBox,
    Selector, SelectorMode, MemoryRegion, util::Align4,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::{
    Foundation::{
        self, CloseHandle, DuplicateHandle, GetLastError, DUPLICATE_SAME_ACCESS,
        ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND,
        ERROR_IO_PENDING, ERROR_MORE_DATA, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED,
        ERROR_PIPE_LISTENING, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
    },
    Security::{self, SECURITY_ATTRIBUTES},
    Storage::FileSystem::{
        self, CreateFileW, ReadFile, WriteFile, FILE_FLAG_OVERLAPPED,
        FILE_GENERIC_WRITE, OPEN_EXISTING, PIPE_ACCESS_INBOUND,
    },
    System::{
        IO::{
            CreateIoCompletionPort, GetQueuedCompletionStatus, PostQueuedCompletionStatus,
            OVERLAPPED,
        },
        Memory::{
            CreateFileMappingW, MapViewOfFile, UnmapViewOfFile,
            FILE_MAP_READ, FILE_MAP_WRITE, PAGE_READWRITE,
        },
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
            GetNamedPipeServerProcessId, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE,
            PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
        },
        Threading::{
            GetCurrentProcess, GetCurrentProcessId, OpenProcess, PROCESS_DUP_HANDLE,
        },
        SystemInformation::GetSystemInfo,
    },
};

// On non-Windows, provide stub types so the module parses but is never compiled.
#[cfg(not(target_os = "windows"))]
type HANDLE = isize;

/// Opaque handle wrapper with ownership semantics.
#[derive(Debug)]
pub struct Handle(isize);

impl Handle {
    pub fn clone(&self) -> io::Result<Self> {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut dup: HANDLE = 0;
            let r = DuplicateHandle(
                GetCurrentProcess(), self.0, GetCurrentProcess(), &mut dup,
                0, 0, DUPLICATE_SAME_ACCESS,
            );
            if r == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self(dup))
        }
        #[cfg(not(target_os = "windows"))]
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }

    pub unsafe fn from_raw(raw: isize) -> Self {
        Self(raw)
    }

    pub fn into_raw(self) -> isize {
        let raw = self.0;
        mem::forget(self);
        raw
    }

    pub fn as_raw(&self) -> isize {
        self.0
    }
}

#[cfg(target_os = "windows")]
impl Drop for Handle {
    fn drop(&mut self) {
        if self.0 != 0 && self.0 != -1 {
            unsafe { CloseHandle(self.0); }
        }
    }
}

unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

pub type Object = Handle;

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
    pub fn new(pipe: Handle) -> Self {
        Self { pipe, process: None }
    }

    pub fn is_dead(&self) -> bool {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut written = 0u32;
            let r = WriteFile(self.pipe.0, ptr::null(), 0, &mut written, ptr::null_mut());
            r == 0
        }
        #[cfg(not(target_os = "windows"))]
        false
    }
}

pub struct Local {
    pipe: Handle,
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
        if self.objects.len() < 2 { return None; }
        let pipe = self.objects.pop()?;
        let process = self.objects.pop()?;
        Some(Remote { pipe, process: Some(process) })
    }

    pub fn send(&mut self, remote: &Remote) -> Result<(), Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            for r in &self.memory_regions {
                r.ref_count_inner(1);
            }

            // Duplicate handles into remote process
            if let Some(ref process) = remote.process {
                let obj_offset = 8; // version(4) + object_count(4)
                let obj_ptr = self.pipe_msg.as_mut_ptr().add(obj_offset) as *mut u64;
                let all_objects: Vec<&Handle> = self.objects.iter()
                    .chain(self.memory_regions.iter().map(|mr| mr.object()))
                    .collect();

                for (i, obj) in all_objects.iter().enumerate() {
                    let mut dup: HANDLE = 0;
                    let r = DuplicateHandle(
                        GetCurrentProcess(), obj.as_raw(), process.as_raw(), &mut dup,
                        0, 0, DUPLICATE_SAME_ACCESS,
                    );
                    if r == 0 {
                        for r in &self.memory_regions { r.ref_count_inner(-1); }
                        return Err(Error::Disconnect);
                    }
                    *obj_ptr.add(i) = dup as u64;
                }
            }

            let mut written = 0u32;
            let r = WriteFile(
                remote.pipe.0,
                self.pipe_msg.as_ptr().cast(),
                self.msg_size as u32,
                &mut written,
                ptr::null_mut(),
            );
            if r == 0 {
                for r in &self.memory_regions { r.ref_count_inner(-1); }
                return Err(Error::Disconnect);
            }
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::IoError(io::Error::new(io::ErrorKind::Unsupported, "Windows only")))
    }

    pub fn from_local(pipe: &mut Local) -> Result<Self, Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            let mut buf = vec![0u8; 64 << 10]; // 64KB initial
            let mut bytes_read = 0u32;
            let r = ReadFile(
                pipe.pipe.0, buf.as_mut_ptr().cast(), buf.len() as u32,
                &mut bytes_read, ptr::null_mut(),
            );
            if r == 0 {
                return Err(Error::Disconnect);
            }
            let msg_size = bytes_read as usize;
            buf.truncate(msg_size);
            Self::new(buf, msg_size)
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::IoError(io::Error::new(io::ErrorKind::Unsupported, "Windows only")))
    }

    fn new(pipe_msg: Vec<u8>, msg_size: usize) -> Result<Self, Error> {
        if msg_size < 8 { return Err(Error::Decode(bincode::error::DecodeError::UnexpectedEnd { additional: 8 })); }

        unsafe {
            let ptr = pipe_msg.as_ptr();

            // Version
            let version_val = *(ptr as *const u32);
            let [magic, major, minor, patch] = u32::to_ne_bytes(version_val);
            if magic != 0xFF {
                return Err(Error::VersionMismatch(Version::new(), None));
            }
            let remote_version = Version::from_parts(major, minor, patch);
            if !version().compatible(remote_version) {
                return Err(Error::VersionMismatch(remote_version, None));
            }

            // Object count
            let object_count = *(ptr.add(4) as *const u32) as usize;
            let objects_start = 8;
            let objects_end = objects_start + object_count * 8;

            let mut objects: Vec<Handle> = (0..object_count)
                .map(|i| {
                    let val = *(ptr.add(objects_start + i * 8) as *const u64);
                    Handle::from_raw(val as isize)
                })
                .collect();

            // Selector
            let selector_size_ptr = ptr.add(objects_end) as *const u32;
            let selector_size = *selector_size_ptr as usize;
            let selector_data = std::slice::from_raw_parts(
                selector_size_ptr.add(1) as *const u8, selector_size,
            );
            let selector: Selector = decode(selector_data)?;

            // Split memory regions from objects
            let mr_count = selector.memory_region_count as usize;
            let memory_regions: Vec<_> = objects
                .drain((objects.len() - mr_count)..)
                .map(|obj| { let r = MemoryRegion::from_object(obj); r.ref_count_inner(-1); r })
                .collect();

            // Payload
            let payload_offset = objects_end + 4 + selector_size.align4();
            let payload_size = *(ptr.add(payload_offset) as *const u32) as usize;
            let payload_data: &'static [u8] = std::slice::from_raw_parts(
                ptr.add(payload_offset + 4), payload_size,
            );

            Ok(Self { selector, payload_data, pipe_msg, msg_size, objects, memory_regions })
        }
    }
}

impl<T: MessageBox> Message<T> {
    fn encode_inner_win(&self) -> (Vec<u8>, usize) {
        let selector_data = bincode::serde::encode_to_vec(&self.selector, bincode::config::standard())
            .expect("selector encode");
        let payload_data = self.payload.encode().expect("payload encode");

        let object_count = self.objects.len() + self.memory_regions.len();
        let msg_size =
            4 /* version */ + 4 /* object_count */ + object_count * 8 /* object slots */
            + 4 /* selector_size */ + selector_data.len().align4()
            + 4 /* payload_size */ + payload_data.len().align4();

        let mut buf = vec![0u8; msg_size];
        unsafe {
            let ptr = buf.as_mut_ptr();

            // Version
            let v = version();
            *(ptr as *mut u32) = u32::from_ne_bytes([0xFF, v.major(), v.minor(), v.patch()]);

            // Object count
            *(ptr.add(4) as *mut u32) = object_count as u32;

            // Object slots (filled by send with DuplicateHandle values)
            // Left as zeros — send() will write the duplicated handles

            let data_start = 8 + object_count * 8;

            // Selector
            *(ptr.add(data_start) as *mut u32) = selector_data.len() as u32;
            ptr::copy_nonoverlapping(
                selector_data.as_ptr(), ptr.add(data_start + 4), selector_data.len(),
            );

            // Payload
            let payload_offset = data_start + 4 + selector_data.len().align4();
            *(ptr.add(payload_offset) as *mut u32) = payload_data.len() as u32;
            ptr::copy_nonoverlapping(
                payload_data.as_ptr(), ptr.add(payload_offset + 4), payload_data.len(),
            );
        }

        (buf, msg_size)
    }

    pub(crate) fn into_encoded(mut self) -> EncodedMessage {
        self.selector.memory_region_count = self.memory_regions.len() as u16;
        let (pipe_msg, msg_size) = self.encode_inner_win();
        EncodedMessage {
            selector: self.selector,
            payload_data: unsafe {
                let ptr = pipe_msg.as_ptr();
                let object_count = *(ptr.add(4) as *const u32) as usize;
                let data_start = 8 + object_count * 8;
                let selector_size = *(ptr.add(data_start) as *const u32) as usize;
                let payload_offset = data_start + 4 + selector_size.align4();
                let payload_size = *(ptr.add(payload_offset) as *const u32) as usize;
                std::slice::from_raw_parts(ptr.add(payload_offset + 4), payload_size)
            },
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
        Self { bus_receiver: Some(bus_receiver), local: None, im }
    }

    fn for_endpoint(local: Local, im: Arc<IoMultiplexing>) -> Self {
        Self { bus_receiver: None, local: Some(local), im }
    }

    pub fn recv(
        &mut self,
        timeout: Option<Duration>,
        _remote: Option<&Remote>,
    ) -> Result<EncodedMessage, Error> {
        let end = timeout.map(|t| Instant::now() + t);

        loop {
            if let Some(rx) = &self.bus_receiver {
                match rx.try_recv() {
                    Ok(msg) => return Ok(msg),
                    Err(TryRecvError::Disconnected) => { self.bus_receiver = None; }
                    Err(TryRecvError::Empty) => {}
                }
            }

            if let Some(local) = &mut self.local {
                match EncodedMessage::from_local(local) {
                    Ok(msg) => return Ok(msg),
                    Err(Error::Timeout) => {}
                    Err(e) => {
                        self.local = None;
                        if self.bus_receiver.is_none() { return Err(e); }
                    }
                }
            }

            if self.bus_receiver.is_none() && self.local.is_none() {
                return Err(Error::Disconnect);
            }

            if let Some(end) = end {
                let remaining = end.saturating_duration_since(Instant::now());
                if remaining.is_zero() { return Err(Error::Timeout); }
                if let Some(rx) = &self.bus_receiver {
                    match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
                        Ok(msg) => return Ok(msg),
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                        Err(_) => { self.bus_receiver = None; continue; }
                    }
                }
            } else if let Some(rx) = &self.bus_receiver {
                match rx.recv_timeout(Duration::from_millis(200)) {
                    Ok(msg) => return Ok(msg),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(_) => { self.bus_receiver = None; continue; }
                }
            }
        }
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

fn pipe_name(identifier: &str) -> Vec<u16> {
    let name = format!("\\\\.\\pipe\\{identifier}");
    name.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(crate) fn look_up(
    identifier: &str,
    label: Label,
    token: String,
    im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Remote, EndpointID), Error> {
    #[cfg(target_os = "windows")]
    unsafe {
        let name = pipe_name(identifier);

        let pipe = CreateFileW(
            name.as_ptr(), FILE_GENERIC_WRITE, 0, ptr::null(), OPEN_EXISTING, 0, 0,
        );
        if pipe == INVALID_HANDLE_VALUE {
            return match GetLastError() {
                ERROR_FILE_NOT_FOUND | ERROR_PIPE_BUSY => Err(Error::IdentifierNotInUse),
                ERROR_ACCESS_DENIED => Err(Error::PermissionDenied),
                _ => Err(Error::Unknown),
            };
        }
        let remote_pipe = Handle(pipe);

        let mut server_pid = 0u32;
        GetNamedPipeServerProcessId(pipe, &mut server_pid);

        let process = OpenProcess(PROCESS_DUP_HANDLE, 0, server_pid);
        let remote_process = if process == 0 {
            None
        } else {
            Some(Handle(process))
        };

        let remote = Remote { pipe: remote_pipe, process: remote_process };

        // Create reply pipe pair for handshake
        let reply_name = format!("\\\\.\\pipe\\{identifier}.reply.{}", uuid::Uuid::new_v4());
        let reply_name_w: Vec<u16> = reply_name.encode_utf16().chain(std::iter::once(0)).collect();

        let read_pipe = CreateNamedPipeW(
            reply_name_w.as_ptr(),
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1, 4 << 20, 4 << 20, 0, ptr::null(),
        );
        if read_pipe == INVALID_HANDLE_VALUE {
            return Err(Error::Unknown);
        }
        let read_handle = Handle(read_pipe);

        let write_pipe = CreateFileW(
            reply_name_w.as_ptr(), FILE_GENERIC_WRITE, 0, ptr::null(), OPEN_EXISTING, 0, 0,
        );
        if write_pipe == INVALID_HANDLE_VALUE {
            return Err(Error::Unknown);
        }
        let write_handle = Handle(write_pipe);

        // Send ConnectMessage with process handle + write pipe handle
        let mut msg = Message::new(
            Selector::unicast(LabelOp::True),
            crate::ipc::ConnectMessage { version: version(), token, label },
        );
        let cur_process = Handle(GetCurrentProcess());
        msg.objects.push(Handle(cur_process.into_raw()));
        msg.objects.push(write_handle);

        let mut encoded = msg.into_encoded();
        encoded.send(&remote)?;

        let local = Local { pipe: read_handle };
        let mut io_hub = IoHub::for_endpoint(local, im);
        let ack_msg = io_hub.recv(Some(Duration::from_secs(2)), Some(&remote))?;
        let ack = crate::ipc::ConnectMessageAck::decode(ack_msg.selector.uuid, ack_msg.payload_data)?;

        match ack {
            crate::ipc::ConnectMessageAck::Ok(endpoint_id) => Ok((io_hub, remote, endpoint_id)),
            crate::ipc::ConnectMessageAck::ErrVersion(v) => Err(Error::VersionMismatch(v, None)),
            crate::ipc::ConnectMessageAck::ErrToken => Err(Error::TokenMismatch),
        }
    }
    #[cfg(not(target_os = "windows"))]
    Err(Error::IoError(io::Error::new(io::ErrorKind::Unsupported, "Windows only")))
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
            PIPE_ACCESS_INBOUND | FILE_FLAG_OVERLAPPED | 0x00080000, // FILE_FLAG_FIRST_PIPE_INSTANCE
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES, 4 << 20, 4 << 20, 0, ptr::null(),
        );
        if pipe == INVALID_HANDLE_VALUE {
            return match GetLastError() {
                ERROR_ALREADY_EXISTS => Err(Error::IdentifierInUse),
                ERROR_ACCESS_DENIED => Err(Error::PermissionDenied),
                _ => Err(Error::Unknown),
            };
        }
        let handle = Handle(pipe);
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
            let handle = CreateFileMappingW(
                INVALID_HANDLE_VALUE, ptr::null(), PAGE_READWRITE,
                (size >> 32) as u32, size as u32, ptr::null(),
            );
            if handle == 0 { return None; }
            Some(Handle(handle))
        }
        #[cfg(not(target_os = "windows"))]
        None
    }
}

impl crate::ipc::platform::MappedRegion {
    pub(crate) fn map(obj: &Object, aligned_offset: usize, aligned_size: usize) -> Result<*mut u8, Error> {
        #[cfg(target_os = "windows")]
        unsafe {
            let ptr = MapViewOfFile(
                obj.as_raw(),
                FILE_MAP_READ | FILE_MAP_WRITE,
                (aligned_offset >> 32) as u32,
                aligned_offset as u32,
                aligned_size,
            );
            if ptr.Value.is_null() {
                Err(Error::MemoryRegionMapping)
            } else {
                Ok(ptr.Value as *mut u8)
            }
        }
        #[cfg(not(target_os = "windows"))]
        Err(Error::MemoryRegionMapping)
    }

    pub(crate) fn unmap(addr: *mut u8, _len: usize) {
        #[cfg(target_os = "windows")]
        unsafe {
            UnmapViewOfFile(windows_sys::Win32::System::Memory::MEMORYMAPPEDVIEW_HANDLE(addr as _));
        }
    }
}

static mut PAGE_MASK: usize = 0;
static PAGE_MASK_ONCE: Once = Once::new();

pub(crate) fn page_mask() -> usize {
    #[cfg(target_os = "windows")]
    unsafe {
        PAGE_MASK_ONCE.call_once(|| {
            let mut info = mem::zeroed();
            GetSystemInfo(&mut info);
            PAGE_MASK = info.dwAllocationGranularity as usize - 1;
        });
        PAGE_MASK
    }
    #[cfg(not(target_os = "windows"))]
    0xFFF
}
