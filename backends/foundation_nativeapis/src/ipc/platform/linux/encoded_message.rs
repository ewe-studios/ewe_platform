/// Linux IPC encoded message — wire format with SCM_RIGHTS FD passing.
///
/// Message layout:
/// | version (magic__major__minor__patch) 4 bytes
/// | selector_size (u32)                   4 bytes
/// | selector                              variable
/// | selector_padding                      0-3 bytes
/// | payload_size (u32)                    4 bytes
/// | payload                               variable
/// | payload_padding                       0-3 bytes
///
/// Ancillary data: SCM_RIGHTS with array of fds (objects + memory region fds)

use std::{io, mem, os::fd::RawFd, ptr, slice};

use type_uuid::TypeUuid;

use super::fd::{Local, Remote};
use crate::ipc::{
    decode, util::Align4, version::version, version::Version,
    Error, MemoryRegion, Selector,
};
use super::Fd as Object;

/// Maximum buffer size for socket send/recv (64KB).
static MAXIMUM_BUF_SIZE: i32 = 64 << 10;

pub(crate) struct EncodedMessage {
    pub selector: Selector,
    pub payload_data: &'static [u8],
    pub iov_data: Vec<u8>,
    pub control_data: Vec<u8>,
    pub objects: Vec<Object>,
    pub memory_regions: Vec<MemoryRegion>,
}

impl EncodedMessage {
    /// Extract the reply remote from the objects list (used during handshake).
    /// The last object in a ConnectMessage is the socketpair write fd.
    pub fn extract_remote(&mut self) -> Option<Remote> {
        debug_assert_eq!(
            self.selector.uuid,
            <crate::ipc::ConnectMessage as TypeUuid>::UUID
        );

        let reply = self.objects.pop()?;
        Some(Remote::new(reply))
    }

    /// Receive an encoded message from a local socket.
    /// Uses MSG_PEEK | MSG_TRUNC to discover message size before receiving.
    pub fn from_local(local: &mut Local) -> Result<Self, Error> {
        unsafe {
            // Peek meta to get message size
            let mut iov = libc::iovec {
                iov_base: ptr::null_mut(),
                iov_len: 0,
            };

            let mut hdr: libc::msghdr = mem::zeroed();
            hdr.msg_iov = &mut iov;
            hdr.msg_iovlen = 1;

            let mut r = libc::recvmsg(
                local.0.as_raw(),
                &mut hdr,
                libc::MSG_PEEK | libc::MSG_TRUNC | libc::MSG_CTRUNC,
            );
            // When remote disconnected, recvmsg returns 0
            if r <= 0 {
                return Err(Error::Disconnect);
            }

            let meta = Meta {
                iov_len: r as u32,
                control_len: 32, // TODO: How to peek this?
            };

            // Receive payload
            let mut iov_data: Vec<u8> = alloc_buffer::<u32>(meta.iov_len as _);
            iov.iov_base = iov_data.as_mut_ptr() as _;
            iov.iov_len = iov_data.len();

            let mut control_data: Vec<u8> = alloc_buffer::<usize>(meta.control_len as _);
            hdr.msg_control = control_data.as_mut_ptr() as _;
            hdr.msg_controllen = control_data.len() as _;

            hdr.msg_flags = 0;

            r = libc::recvmsg(local.0.as_raw(), &mut hdr, 0);
            // When remote disconnected, recvmsg returns 0
            if r <= 0
                || (hdr.msg_flags & libc::MSG_TRUNC == libc::MSG_TRUNC)
                || (hdr.msg_flags & libc::MSG_CTRUNC == libc::MSG_CTRUNC)
            {
                return Err(Error::Disconnect);
            }

            // Parse ancillary data (objects/fds)
            let mut objects = vec![];

            if hdr.msg_controllen > 0 {
                let control_ptr = control_data.as_ptr() as *const libc::cmsghdr;
                let mut control_data_ptr = libc::CMSG_DATA(control_ptr) as *const RawFd;

                let control_count = ((*control_ptr).cmsg_len as usize
                    - (control_data_ptr as usize - control_ptr as usize))
                    / mem::size_of::<RawFd>();

                for _ in 0..control_count {
                    objects.push(Object::from_raw(ptr::read(control_data_ptr)));
                    control_data_ptr = control_data_ptr.offset(1);
                }
            }

            // Parse version
            let version_ptr = iov_data.as_ptr() as *const u32;
            let [magic, major, minor, patch]: [u8; 4] = u32::to_ne_bytes(*version_ptr);
            if magic != 0xFF {
                return Err(Error::VersionMismatch(Version::new(), None));
            }
            let remote_version = Version::from_parts(major, minor, patch);

            if !version().compatible(remote_version) {
                return Err(Error::VersionMismatch(remote_version, None));
            }

            // Parse selector
            let selector_size_ptr = version_ptr.offset(1);
            let selector_size = *selector_size_ptr;
            let selector_ptr = selector_size_ptr.offset(1) as *const u8;

            let selector: Selector =
                decode(slice::from_raw_parts(selector_ptr, selector_size as _))?;

            // Extract memory regions from trailing objects
            let memory_regions: Vec<_> = objects
                .drain((objects.len() - selector.memory_region_count as usize)..)
                .map(|obj| {
                    let r = MemoryRegion::from_object(obj);
                    r.ref_count_inner(-1);
                    r
                })
                .collect();

            // Parse payload pointer
            let payload_size_ptr = selector_ptr.offset(selector_size.align4() as _) as *const u32;

            Ok(Self {
                selector,
                payload_data: slice::from_raw_parts(
                    payload_size_ptr.offset(1) as *const u8,
                    *payload_size_ptr as _,
                ),
                iov_data,
                control_data,
                objects,
                memory_regions,
            })
        }
    }

    /// Send this message to a remote endpoint.
    pub fn send(&mut self, remote: &Remote) -> Result<(), Error> {
        let mut iov = libc::iovec {
            iov_base: ptr::null_mut(),
            iov_len: 0,
        };
        let mut hdr: libc::msghdr = unsafe { mem::zeroed() };
        hdr.msg_iov = &mut iov;
        hdr.msg_iovlen = 1;

        // Set up payload
        iov.iov_base = self.iov_data.as_mut_ptr() as _;
        iov.iov_len = self.iov_data.len();

        hdr.msg_control = self.control_data.as_mut_ptr() as _;
        hdr.msg_controllen = self.control_data.len() as _;

        // Increment ref count for memory regions
        for r in self.memory_regions.iter() {
            r.ref_count_inner(1);
        }

        let remote_guard = remote.lock();
        let r = unsafe { libc::sendmsg(remote_guard.as_raw(), &hdr, 0) };
        if r == -1 {
            for r in self.memory_regions.iter() {
                r.ref_count_inner(-1);
            }
            Err(Error::Disconnect)
        } else {
            Ok(())
        }
    }
}

struct Meta {
    iov_len: u32,
    control_len: u32,
}

/// Allocate a buffer with alignment for type T.
pub fn alloc_buffer<T>(size: usize) -> Vec<u8> {
    let mut buffer_t_size = size / mem::size_of::<T>();
    if size % mem::size_of::<T>() != 0 {
        buffer_t_size += 1;
    }
    let mut buf_t = Vec::<T>::with_capacity(buffer_t_size);

    let buffer_size = buffer_t_size * mem::size_of::<T>();
    let buf = unsafe { Vec::from_raw_parts(buf_t.as_mut_ptr().cast::<u8>(), buffer_size, buffer_size) };

    mem::forget(buf_t);

    buf
}
