/// macOS IPC transport — Mach ports with mach_msg, kqueue IO multiplexing.
///
/// Uses mach ports for communication and bootstrap_register/bootstrap_look_up for bus discovery.

use std::{
    ffi::CString, io, mem, os::unix::prelude::{AsRawFd, FromRawFd, OwnedFd},
    ptr, slice, sync::{mpsc, Arc, Mutex, Once, mpsc::{Receiver, Sender, TryRecvError}},
    time::{Duration, Instant},
};
use type_uuid::TypeUuid;

use crate::ipc::{
    decode, version::version, version::Version,
    EndpointID, Error, Label, LabelOp, Message, MessageBox,
    Selector, SelectorMode, MemoryRegion, util::Align4,
};

// MACH_PORT_TYPE_DEAD_NAME = 1 << (4 + 16)
const MACH_PORT_TYPE_DEAD_NAME: mach_sys::mach_port_type_t = 1 << (4 + 16);

mod mach_sys {
    // Stub mach port bindings — full implementation requires mach_sys crate or manual bindings
    pub type mach_port_t = u32;
    pub type mach_port_type_t = u32;
    pub type kern_return_t = i32;
    pub type vm_prot_t = u32;
    pub type vm_inherit_t = u32;
    pub type vm_address_t = usize;
    pub type vm_size_t = usize;
    pub type vm_offset_t = usize;
    pub type mach_msg_type_name_t = u32;
    pub type mach_msg_bits_t = u32;
    pub type mach_msg_size_t = u32;
    pub type mach_msg_id_t = i32;

    pub const KERN_SUCCESS: kern_return_t = 0;
    pub const MACH_PORT_RIGHT_RECEIVE: u32 = 1;
    pub const MACH_PORT_RIGHT_SEND: u32 = 0;
    pub const MACH_PORT_QLIMIT_MAX: u32 = 1024;
    pub const MACH_PORT_LIMITS_INFO: u32 = 1;
    pub const MACH_MSG_TYPE_COPY_SEND: mach_msg_type_name_t = 19;
    pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x80000000;
    pub const MACH_PORT_NULL: mach_port_t = 0;
    pub const MACH_MSG_PORT_DESCRIPTOR: u32 = 0;
    pub const MACH_RCV_MSG: u32 = 2;
    pub const MACH_RCV_LARGE: u32 = 16;
    pub const MACH_RCV_TIMEOUT: u32 = 256;
    pub const MACH_RCV_TOO_LARGE: kern_return_t = 0x10000004;
    pub const MACH_MSG_SUCCESS: kern_return_t = 0;
    pub const MACH_RCV_TIMED_OUT: kern_return_t = 0x10000003;
    pub const MACH_RCV_PORT_DIED: kern_return_t = 0x10000010;
    pub const MACH_SEND_NO_BUFFER: kern_return_t = 0x10000009;
    pub const BOOTSTRAP_SUCCESS: kern_return_t = 0;
    pub const BOOTSTRAP_UNKNOWN_SERVICE: kern_return_t = 1;
    pub const BOOTSTRAP_NOT_PRIVILEGED: kern_return_t = 2;
    pub const BOOTSTRAP_NAME_IN_USE: kern_return_t = 3;

    #[repr(C)]
    pub struct mach_msg_header_t {
        pub msgh_bits: mach_msg_bits_t,
        pub msgh_size: mach_msg_size_t,
        pub msgh_remote_port: mach_port_t,
        pub msgh_local_port: mach_port_t,
        pub msgh_reserved: mach_msg_size_t,
        pub msgh_id: mach_msg_id_t,
    }

    #[repr(C)]
    pub struct mach_msg_body_t {
        pub msgh_descriptor_count: mach_msg_size_t,
    }

    #[repr(C)]
    pub struct mach_msg_port_descriptor_t {
        pub name: mach_port_t,
        pub pad1: u32,
        pub pad2: u16,
        pub disposition: mach_msg_type_name_t,
        pub type_: u32,
    }

    #[repr(C)]
    pub struct mach_msg_trailer_t {
        pub message_type: u32,
        pub pad: [u8; 12],
    }

    // Stub functions — full implementation requires linking against libSystem
    extern "C" {
        pub fn mach_task_self() -> mach_port_t;
        pub fn mach_port_allocate(task: mach_port_t, right: u32, name: *mut mach_port_t) -> kern_return_t;
        pub fn mach_port_insert_right(task: mach_port_t, port: mach_port_t, name: mach_port_t, disposition: mach_msg_type_name_t) -> kern_return_t;
        pub fn mach_port_set_attributes(task: mach_port_t, port: mach_port_t, flavor: u32, info: *mut mach_port_limits_t, count: u32) -> kern_return_t;
        pub fn mach_port_mod_refs(task: mach_port_t, name: mach_port_t, right: u32, delta: i32) -> kern_return_t;
        pub fn mach_port_deallocate(task: mach_port_t, name: mach_port_t) -> kern_return_t;
        pub fn mach_port_type(task: mach_port_t, name: mach_port_t, type_: *mut mach_port_type_t) -> kern_return_t;
        pub fn mach_msg(msg: *mut mach_msg_header_t, option: u32, send_size: mach_msg_size_t, rcv_size: mach_msg_size_t, rcv_name: mach_port_t, timeout: u32, notify: mach_port_t) -> kern_return_t;
        pub fn mach_make_memory_entry_64(task: mach_port_t, size: *mut u64, offset: u64, protection: vm_prot_t, object_handle: *mut mach_port_t, parent_handle: mach_port_t) -> kern_return_t;
        pub fn vm_map(task: mach_port_t, address: *mut vm_address_t, size: vm_size_t, mask: vm_address_t, flags: u32, object: mach_port_t, offset: vm_offset_t, copy: u32, cur_protection: vm_prot_t, max_protection: vm_prot_t, inheritance: vm_inherit_t) -> kern_return_t;
        pub fn vm_deallocate(task: mach_port_t, address: vm_address_t, size: vm_size_t) -> kern_return_t;
        pub fn bootstrap_look_up(bs: mach_port_t, service_name: *const i8, service_port: *mut mach_port_t) -> kern_return_t;
        pub fn bootstrap_register(bs: mach_port_t, service_name: *const i8, service_port: mach_port_t) -> kern_return_t;
        pub fn task_get_special_port(task: mach_port_t, which: u32, port: *mut mach_port_t) -> kern_return_t;
        pub fn vm_page_mask: usize;
    }

    #[repr(C)]
    pub struct mach_port_limits_t {
        pub mpl_qlimit: u32,
    }

    pub const TASK_BOOTSTRAP_PORT: u32 = 4;
    pub const MACH_MSGH_BITS_COMPLEX: u32 = 0x80000000;
    pub const VM_FLAGS_ANYWHERE: u32 = 0x0001;
    pub const VM_PROT_READ: vm_prot_t = 0x01;
    pub const VM_PROT_WRITE: vm_prot_t = 0x02;
    pub const VM_PROT_DEFAULT: vm_prot_t = VM_PROT_READ | VM_PROT_WRITE;
    pub const MAP_MEM_NAMED_CREATE: vm_prot_t = 0x020000;
    pub const VM_INHERIT_NONE: vm_inherit_t = 2;

    pub static mut vm_page_mask: usize = 0;
}

/// Mach port wrapper.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct MachPort {
    port: mach_sys::mach_port_t,
    receive_right: bool,
}

impl MachPort {
    pub fn clone(&self) -> io::Result<Self> {
        unsafe {
            let r = mach_sys::mach_port_mod_refs(
                mach_sys::mach_task_self(),
                self.as_raw(),
                mach_sys::MACH_PORT_RIGHT_SEND,
                1,
            );
            if r != mach_sys::KERN_SUCCESS {
                return Err(io::Error::other("mach_port_mod_refs"));
            }
            Ok(Self::from_raw(self.as_raw()))
        }
    }

    pub unsafe fn from_raw(port: mach_sys::mach_port_t) -> Self {
        Self { port, receive_right: false }
    }

    pub unsafe fn into_raw(self) -> mach_sys::mach_port_t {
        let raw = self.as_raw();
        mem::forget(self);
        raw
    }

    pub fn as_raw(&self) -> mach_sys::mach_port_t {
        self.port
    }
}

impl Drop for MachPort {
    fn drop(&mut self) {
        unsafe {
            if self.receive_right {
                mach_sys::mach_port_mod_refs(
                    mach_sys::mach_task_self(),
                    self.as_raw(),
                    mach_sys::MACH_PORT_RIGHT_RECEIVE,
                    -1,
                );
            }
            mach_sys::mach_port_deallocate(mach_sys::mach_task_self(), self.as_raw());
        }
    }
}

pub type Object = MachPort;

#[derive(Debug, PartialEq)]
pub struct Remote {
    port: MachPort,
}

impl Remote {
    pub fn new(fd: MachPort) -> Self {
        Self { port: fd }
    }

    pub fn lock(&self) -> RemoteLock<'_> {
        RemoteLock { port: &self.port }
    }

    pub fn is_dead(&self) -> bool {
        let mut ty = 0;
        unsafe {
            let r = mach_sys::mach_port_type(mach_sys::mach_task_self(), self.port.as_raw(), &mut ty);
            if r != mach_sys::KERN_SUCCESS { return true; }
        }
        ty & MACH_PORT_TYPE_DEAD_NAME != 0
    }
}

pub struct RemoteLock<'a> {
    port: &'a MachPort,
}

impl RemoteLock<'_> {
    pub fn as_raw(&self) -> mach_sys::mach_port_t {
        self.port.as_raw()
    }
}

/// The local (read) end — the mach port we listen on.
pub struct Local(pub(crate) MachPort);

/// Bootstrap port cache.
static mut BOOTSTRAP_PORT_ROOT: mach_sys::mach_port_t = 0;
static INIT: Once = Once::new();

unsafe fn init() {
    INIT.call_once(|| {
        let mut up = 0;
        let mut r = mach_sys::task_get_special_port(
            mach_sys::mach_task_self(),
            mach_sys::TASK_BOOTSTRAP_PORT,
            &mut BOOTSTRAP_PORT_ROOT,
        );
        assert_eq!(r, mach_sys::BOOTSTRAP_SUCCESS);

        loop {
            r = mach_sys::bootstrap_parent(BOOTSTRAP_PORT_ROOT, &mut up);
            if r != mach_sys::BOOTSTRAP_SUCCESS { break; }
            if BOOTSTRAP_PORT_ROOT == up { break; }
            BOOTSTRAP_PORT_ROOT = up;
        }
    });
}

fn get_bootstrap_port_root() -> mach_sys::mach_port_t {
    unsafe { init(); BOOTSTRAP_PORT_ROOT }
}

pub(crate) fn look_up(
    identifier: &str,
    label: Label,
    token: String,
    im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Remote, EndpointID), Error> {
    let identifier = CString::new(identifier).unwrap();
    let mut remote: mach_sys::mach_port_t = 0;

    unsafe {
        let r = mach_sys::bootstrap_look_up(
            get_bootstrap_port_root(),
            identifier.as_ptr(),
            &mut remote,
        );

        match r {
            mach_sys::BOOTSTRAP_SUCCESS if remote > 0 => {
                let remote = Remote { port: MachPort::from_raw(remote) };
                let local = MachPort::with_receive_right();

                let mut msg = Message::new(
                    Selector::unicast(LabelOp::True),
                    crate::ipc::ConnectMessage {
                        version: version(),
                        token,
                        label,
                    },
                );
                msg.objects.push(local.clone().map_err(|_| Error::Unknown)?);
                let mut encoded_msg = msg.into_encoded();
                encoded_msg.send(&remote)?;

                let mut io_hub: IoHub = IoHub::for_endpoint(local, im);
                let encoded_msg = io_hub.recv(Some(Duration::from_secs(2)), Some(&remote))?;
                let ack = crate::ipc::ConnectMessageAck::decode(
                    encoded_msg.selector.uuid, encoded_msg.payload_data,
                )?;

                match ack {
                    crate::ipc::ConnectMessageAck::Ok(endpoint_id) => Ok((io_hub, remote, endpoint_id)),
                    crate::ipc::ConnectMessageAck::ErrVersion(v) => Err(Error::VersionMismatch(v, None)),
                    crate::ipc::ConnectMessageAck::ErrToken => Err(Error::TokenMismatch),
                }
            }
            mach_sys::BOOTSTRAP_UNKNOWN_SERVICE => Err(Error::IdentifierNotInUse),
            mach_sys::BOOTSTRAP_NOT_PRIVILEGED => Err(Error::PermissionDenied),
            _ => Err(Error::Unknown),
        }
    }
}

pub(crate) fn register(
    identifier: &str,
    im: Arc<IoMultiplexing>,
) -> Result<(IoHub, Sender<EncodedMessage>, EndpointID), Error> {
    let identifier = CString::new(identifier).unwrap();
    let local = MachPort::with_receive_right();
    unsafe {
        let r = mach_sys::bootstrap_register(
            get_bootstrap_port_root(),
            identifier.as_ptr(),
            local.as_raw(),
        );

        match r {
            mach_sys::BOOTSTRAP_SUCCESS => {
                let (bus_sender, bus_receiver) = mpsc::channel();
                Ok((
                    IoHub::for_bus_controller(local, bus_receiver, im),
                    bus_sender,
                    EndpointID::new(),
                ))
            }
            mach_sys::BOOTSTRAP_NAME_IN_USE => Err(Error::IdentifierInUse),
            mach_sys::BOOTSTRAP_NOT_PRIVILEGED => Err(Error::PermissionDenied),
            _ => Err(Error::Unknown),
        }
    }
}

impl MachPort {
    fn with_receive_right() -> Self {
        let mut local = 0;
        unsafe {
            let mut r = mach_sys::mach_port_allocate(
                mach_sys::mach_task_self(),
                mach_sys::MACH_PORT_RIGHT_RECEIVE,
                &mut local,
            );
            assert_eq!(r, mach_sys::KERN_SUCCESS);

            r = mach_sys::mach_port_insert_right(
                mach_sys::mach_task_self(),
                local,
                local,
                mach_sys::MACH_MSG_TYPE_COPY_SEND,
            );
            assert_eq!(r, mach_sys::KERN_SUCCESS);

            let mut limits = mach_sys::mach_port_limits_t {
                mpl_qlimit: mach_sys::MACH_PORT_QLIMIT_MAX,
            };
            r = mach_sys::mach_port_set_attributes(
                mach_sys::mach_task_self(),
                local,
                mach_sys::MACH_PORT_LIMITS_INFO,
                &mut limits as *mut _ as _,
                1,
            );
            assert_eq!(r, mach_sys::KERN_SUCCESS);

            Self { port: local, receive_right: true }
        }
    }
}

// Pipe state machine for mach port reading
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum PipeStatus { Readable, Pending, Offline }

struct Pipe {
    port: MachPort,
    status: PipeStatus,
}

impl Pipe {
    fn new(port: MachPort) -> Self {
        Self { port, status: PipeStatus::Readable }
    }

    fn read(&mut self) -> Option<Vec<u8>> {
        match self.status {
            PipeStatus::Readable => {}
            PipeStatus::Pending | PipeStatus::Offline => return None,
        }

        let mut mach_msg: Vec<u8> = Vec::with_capacity(
            mem::size_of::<BaseMessage>() + mem::size_of::<mach_sys::mach_msg_trailer_t>() + 64,
        );
        let option = mach_sys::MACH_RCV_MSG | mach_sys::MACH_RCV_LARGE | mach_sys::MACH_RCV_TIMEOUT;

        unsafe {
            loop {
                let header_ptr = mach_msg.as_mut_ptr() as *mut BaseMessage;
                (*header_ptr).header.msgh_local_port = 0;
                (*header_ptr).header.msgh_size = 0;
                (*header_ptr).header.msgh_bits = 0;
                (*header_ptr).header.msgh_remote_port = 0;
                (*header_ptr).header.msgh_reserved = 0;
                (*header_ptr).header.msgh_id = 0;

                let r = mach_sys::mach_msg(
                    header_ptr as *mut _,
                    option,
                    0,
                    mach_msg.capacity() as _,
                    self.port.as_raw(),
                    0,
                    mach_sys::MACH_PORT_NULL,
                );

                match r {
                    mach_sys::MACH_RCV_TOO_LARGE => {
                        mach_msg.reserve(
                            (*header_ptr).header.msgh_size as usize
                                + mem::size_of::<mach_sys::mach_msg_trailer_t>(),
                        );
                        continue;
                    }
                    mach_sys::MACH_MSG_SUCCESS => {
                        break Some(mach_msg);
                    }
                    mach_sys::MACH_RCV_TIMED_OUT => {
                        self.status = PipeStatus::Pending;
                        break None;
                    }
                    mach_sys::MACH_RCV_PORT_DIED => {
                        self.status = PipeStatus::Offline;
                        break None;
                    }
                    _ => {
                        self.status = PipeStatus::Offline;
                        break None;
                    }
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Default)]
struct BaseMessage {
    header: mach_sys::mach_msg_header_t,
    body: mach_sys::mach_msg_body_t,
}

pub(crate) struct EncodedMessage {
    pub selector: Selector,
    pub payload_data: &'static [u8],
    mach_msg: Vec<u8>,
    pub objects: Vec<MachPort>,
    pub memory_regions: Vec<MemoryRegion>,
}

impl EncodedMessage {
    pub fn extract_remote(&mut self) -> Option<Remote> {
        debug_assert_eq!(self.selector.uuid, <crate::ipc::ConnectMessage as TypeUuid>::UUID);
        let reply = self.objects.pop()?;
        Some(Remote { port: reply })
    }

    pub fn send(&mut self, remote: &Remote) -> Result<(), Error> {
        unsafe {
            let header_ptr = self.mach_msg.as_mut_ptr() as *mut BaseMessage;
            (*header_ptr).header.msgh_remote_port = remote.port.as_raw();

            for r in self.memory_regions.iter() {
                r.ref_count_inner(1);
            }

            loop {
                let r = mach_sys::mach_msg(header_ptr as *mut _, mach_sys::MACH_SEND_MSG | mach_sys::MACH_RCV_MSG,
                    (*header_ptr).header.msgh_size, 0, remote.port.as_raw(), 0, mach_sys::MACH_PORT_NULL);
                if r == mach_sys::MACH_MSG_SUCCESS {
                    break Ok(());
                } else if r == mach_sys::MACH_SEND_NO_BUFFER {
                    thread::sleep(Duration::from_millis(200));
                } else {
                    for r in self.memory_regions.iter() {
                        r.ref_count_inner(-1);
                    }
                    break if remote.is_dead() {
                        Err(Error::Disconnect)
                    } else {
                        Err(Error::Unknown)
                    };
                }
            }
        }
    }

    pub fn from_local(local: &mut Local) -> Result<Self, Error> {
        match local.0.read() {
            Some(mach_msg) => Self::new(mach_msg),
            None => Err(Error::Disconnect),
        }
    }

    fn new(mut mach_msg: Vec<u8>) -> Result<Self, Error> {
        unsafe {
            let base_ptr = mach_msg.as_mut_ptr() as *mut BaseMessage;
            let mut descriptor_ptr = base_ptr.offset(1) as *mut mach_sys::mach_msg_port_descriptor_t;
            let mut objects = Vec::with_capacity((*base_ptr).body.msgh_descriptor_count as _);

            for _ in 0..(*base_ptr).body.msgh_descriptor_count {
                objects.push(MachPort::from_raw((*descriptor_ptr).name));
                descriptor_ptr = descriptor_ptr.offset(1);
            }

            let version_ptr = descriptor_ptr as *mut u32;
            let [magic, major, minor, patch]: [u8; 4] = u32::to_ne_bytes(*version_ptr);
            let remote_version = Version((major, minor, patch));
            if magic != 0xFF {
                return Err(Error::VersionMismatch(Version((0, 0, 0)), None));
            }
            if !version().compatible(remote_version) {
                return Err(Error::VersionMismatch(remote_version, None));
            }

            let selector_size_ptr = version_ptr.offset(1);
            let selector_ptr = selector_size_ptr.offset(1) as *const u8;
            let selector: Selector = decode(slice::from_raw_parts(selector_ptr, *selector_size_ptr as _))?;

            let memory_regions: Vec<_> = objects
                .drain((objects.len() - selector.memory_region_count as usize)..)
                .map(|obj| { let r = MemoryRegion::from_object(obj); r.ref_count_inner(-1); r })
                .collect();

            let payload_size_ptr = selector_ptr.offset((*selector_size_ptr).align4() as _) as *const u32;
            Ok(Self {
                selector, payload_data: slice::from_raw_parts(payload_size_ptr.offset(1) as *const u8, *payload_size_ptr as _),
                mach_msg, objects, memory_regions,
            })
        }
    }
}

pub(crate) struct IoHub {
    local: Option<Pipe>,
    bus_receiver: Option<Receiver<EncodedMessage>>,
    waker: usize,
    im: Arc<IoMultiplexing>,
    in_buffer: Vec<libc::kevent>,
}

impl IoHub {
    fn for_bus_controller(local: MachPort, bus_receiver: Receiver<EncodedMessage>, im: Arc<IoMultiplexing>) -> Self {
        im.register_mach_port(&local);
        Self { local: Some(Pipe::new(local)), bus_receiver: Some(bus_receiver), waker: im.waker, im, in_buffer: Vec::with_capacity(2) }
    }

    fn for_endpoint(local: MachPort, im: Arc<IoMultiplexing>) -> Self {
        im.register_mach_port(&local);
        Self { local: Some(Pipe::new(local)), bus_receiver: None, waker: im.waker, im, in_buffer: Vec::with_capacity(2) }
    }

    pub fn recv(&mut self, timeout: Option<Duration>, remote: Option<&Remote>) -> Result<EncodedMessage, Error> {
        let end = timeout.map(|timeout| Instant::now() + timeout);
        loop {
            if let Some(bus_receiver) = &self.bus_receiver {
                match bus_receiver.try_recv() {
                    Ok(message) => break Ok(message),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => { self.bus_receiver = None; }
                }
            }

            if let Some(local) = &mut self.local {
                match local.read() {
                    Some(mach_msg) => break EncodedMessage::new(mach_msg),
                    None => match local.status {
                        PipeStatus::Pending => {
                            if let Some(remote) = remote { if remote.is_dead() { self.local = None; } }
                        }
                        PipeStatus::Offline => self.local = None,
                        PipeStatus::Readable => unreachable!(),
                    },
                }
            }

            if self.bus_receiver.is_none() && self.local.is_none() {
                break Err(Error::Disconnect);
            }

            self.im.wait(&mut self.in_buffer, Some(Duration::from_millis(200)));

            if self.in_buffer.is_empty() {
                if let Some(end) = end {
                    if Instant::now() > end { break Err(Error::Timeout); }
                }
                continue;
            }

            if let Some(local) = &mut self.local {
                if self.in_buffer.drain(..).any(|i| i.ident == local.port.as_raw() as _) {
                    local.status = PipeStatus::Readable;
                }
            }
        }
    }

    pub fn io_multiplexing(&self) -> Arc<IoMultiplexing> {
        self.im.clone()
    }
}

pub(crate) struct IoMultiplexing {
    fd: OwnedFd,
    pub(crate) waker: usize,
}

impl IoMultiplexing {
    pub fn new() -> Self {
        unsafe {
            let fd = libc::kqueue();
            assert_ne!(fd, -1);
            let fd = OwnedFd::from_raw_fd(fd);

            let event = libc::kevent {
                ident: 2887, filter: libc::EVFILT_USER, flags: libc::EV_ADD | libc::EV_CLEAR,
                fflags: 0, data: 0, udata: ptr::null_mut(),
            };
            let r = libc::kevent(fd.as_raw_fd(), &event, 1, ptr::null_mut(), 0, ptr::null());
            assert_ne!(r, -1);

            Self { fd, waker: event.ident }
        }
    }

    fn register_mach_port(&self, mach_port: &MachPort) {
        unsafe {
            let event = libc::kevent {
                ident: mach_port.as_raw() as _, filter: libc::EVFILT_MACHPORT,
                flags: libc::EV_ADD | libc::EV_RECEIPT, fflags: 0, data: 0, udata: ptr::null_mut(),
            };
            libc::kevent(self.fd.as_raw_fd(), &event, 1, ptr::null_mut(), 0, ptr::null());
        }
    }

    fn wait(&self, receiver: &mut Vec<libc::kevent>, timeout: Option<Duration>) {
        let mut tmout = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        if let Some(timeout) = timeout {
            tmout.tv_sec = timeout.as_secs() as _;
            tmout.tv_nsec = timeout.subsec_nanos() as _;
        }
        unsafe {
            let n = libc::kevent(
                self.fd.as_raw_fd(), ptr::null(), 0,
                receiver.as_mut_ptr() as _, receiver.capacity() as _,
                if timeout.is_some() { &tmout } else { ptr::null() },
            );
            if n < 0 { receiver.clear(); } else { receiver.set_len(n as _); }
        }
    }

    pub fn wake(&self) {
        unsafe {
            let event = libc::kevent {
                ident: self.waker, filter: libc::EVFILT_USER, flags: 0,
                fflags: libc::NOTE_TRIGGER, data: 0, udata: ptr::null_mut(),
            };
            libc::kevent(self.fd.as_raw_fd(), &event, 1, ptr::null_mut(), 0, ptr::null());
        }
    }
}

// MemoryRegion: macOS uses mach_make_memory_entry_64 + vm_map
impl MemoryRegion {
    pub(crate) fn obj_new(size: usize) -> Option<Object> {
        let mut port = 0;
        let mut alloc_size = size as u64;
        unsafe {
            let r = mach_sys::mach_make_memory_entry_64(
                mach_sys::mach_task_self(), &mut alloc_size, 0,
                mach_sys::MAP_MEM_NAMED_CREATE | mach_sys::VM_PROT_DEFAULT,
                &mut port, mach_sys::MACH_PORT_NULL,
            );
            if r != mach_sys::KERN_SUCCESS || alloc_size < size as u64 { return None; }
            Some(Object::from_raw(port))
        }
    }
}

impl crate::ipc::platform::MappedRegion {
    pub(crate) fn map(obj: &Object, aligned_offset: usize, aligned_size: usize) -> Result<*mut u8, Error> {
        let mut address = 0;
        let r = unsafe {
            mach_sys::vm_map(
                mach_sys::mach_task_self(), &mut address, aligned_size, 0,
                mach_sys::VM_FLAGS_ANYWHERE, obj.as_raw(), aligned_offset, 0,
                mach_sys::VM_PROT_DEFAULT, mach_sys::VM_PROT_DEFAULT, mach_sys::VM_INHERIT_NONE,
            )
        };
        if r != mach_sys::KERN_SUCCESS { Err(Error::MemoryRegionMapping) } else { Ok(address as *mut u8) }
    }

    pub(crate) fn unmap(addr: *mut u8, len: usize) {
        unsafe { mach_sys::vm_deallocate(mach_sys::mach_task_self(), addr as _, len); }
    }
}

pub(crate) fn page_mask() -> usize {
    unsafe { mach_sys::vm_page_mask }
}
