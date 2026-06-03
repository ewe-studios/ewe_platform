/// Message encoding and wire format for the IPC bus.

use std::io::{self, IoSlice};
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd, FromRawFd, IntoRawFd};
use std::ptr;

use bincode::config::standard;
use bincode::Encode;

use crate::ipc::errors::IpcError;
use crate::ipc::version::Version;
use crate::ipc::util::Align4;

use super::Remote;

/// Maximum number of fds that can be passed in a single message.
const MAX_FDS: usize = 64;

/// An encoded message ready for transmission over the socket.
pub struct EncodedMessage {
    /// Raw bytes of the encoded message (version + selector + payload).
    pub data: Vec<u8>,
    /// File descriptors to pass via `SCM_RIGHTS`.
    pub fds: Vec<OwnedFd>,
}

impl EncodedMessage {
    /// Create a new encoded message from raw bytes.
    pub fn new(data: Vec<u8>, fds: Vec<OwnedFd>) -> Self {
        Self { data, fds }
    }

    /// Encode a typed payload message.
    pub fn encode<T: Encode>(
        version: Version,
        selector_bytes: &[u8],
        payload: &T,
        fds: Vec<OwnedFd>,
    ) -> std::result::Result<Self, IpcError> {
        let payload_bytes = bincode::encode_to_vec(payload, standard())
            .map_err(IpcError::Encode)?;

        let sel_len = selector_bytes.len();
        let sel_aligned = sel_len.align4();
        let pay_len = payload_bytes.len();
        let pay_aligned = pay_len.align4();

        let total = 4 + 4 + sel_aligned + 4 + pay_aligned;
        let mut buf = Vec::with_capacity(total);

        buf.extend_from_slice(&version.to_u32().to_le_bytes());
        buf.extend_from_slice(&(sel_len as u32).to_le_bytes());
        buf.extend_from_slice(selector_bytes);
        buf.resize(buf.len() + (sel_aligned - sel_len), 0);
        buf.extend_from_slice(&(pay_len as u32).to_le_bytes());
        buf.extend_from_slice(&payload_bytes);
        buf.resize(buf.len() + (pay_aligned - pay_len), 0);

        Ok(Self {
            data: buf,
            fds,
        })
    }

    /// Send this message over a Unix domain socket.
    pub fn send(&self, remote: &Remote) -> std::result::Result<(), IpcError> {
        let fd_guard = remote.lock();
        let sock_fd = fd_guard.as_raw_fd();

        if self.fds.is_empty() {
            let mut offset = 0;
            while offset < self.data.len() {
                let written = unsafe {
                    libc::send(
                        sock_fd,
                        self.data.as_ptr().add(offset) as *const _,
                        self.data.len() - offset,
                        libc::MSG_NOSIGNAL,
                    )
                };
                if written < 0 {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::WouldBlock
                        || err.kind() == io::ErrorKind::Interrupted
                    {
                        continue;
                    }
                    return Err(IpcError::Io(err));
                }
                offset += written as usize;
            }
        } else {
            // sendmsg with SCM_RIGHTS
            let iov = libc::iovec {
                iov_base: self.data.as_ptr() as *mut _,
                iov_len: self.data.len(),
            };

            let fd_count = self.fds.len().min(MAX_FDS);
            let cmsg_space = unsafe { libc::CMSG_SPACE((fd_count * std::mem::size_of::<i32>()) as _) as usize };
            let mut cmsg_buf = vec![0u8; cmsg_space];

            let msg = libc::msghdr {
                msg_name: ptr::null_mut(),
                msg_namelen: 0,
                msg_iov: &iov as *const _ as *mut _,
                msg_iovlen: 1,
                msg_control: cmsg_buf.as_mut_ptr() as *mut _,
                msg_controllen: cmsg_space as _,
                #[cfg(target_os = "linux")]
                msg_flags: 0,
            };

            let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
            if !cmsg.is_null() {
                unsafe {
                    (*cmsg).cmsg_level = libc::SOL_SOCKET;
                    (*cmsg).cmsg_type = libc::SCM_RIGHTS;
                    (*cmsg).cmsg_len =
                        libc::CMSG_LEN((fd_count * std::mem::size_of::<i32>()) as _) as _;

                    let data_ptr = libc::CMSG_DATA(cmsg) as *mut i32;
                    for (i, fd) in self.fds.iter().take(fd_count).enumerate() {
                        data_ptr.add(i).write(fd.as_raw_fd());
                    }
                }
            }

            let sent = unsafe { libc::sendmsg(sock_fd, &msg, libc::MSG_NOSIGNAL) };
            if sent < 0 {
                return Err(IpcError::Io(io::Error::last_os_error()));
            }
        }

        Ok(())
    }

    /// Receive a message from a socket.
    pub fn recv(sock_fd: BorrowedFd<'_>) -> std::result::Result<(Vec<u8>, Vec<OwnedFd>), IpcError> {
        let mut data = Vec::with_capacity(4096);
        let mut fds = Vec::new();

        let cmsg_space = unsafe { libc::CMSG_SPACE((MAX_FDS * std::mem::size_of::<i32>()) as _) as usize };
        let mut cmsg_buf = vec![0u8; cmsg_space];

        let mut buf = [0u8; 4096];
        let iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut _,
            iov_len: buf.len(),
        };

        let msg = libc::msghdr {
            msg_name: ptr::null_mut(),
            msg_namelen: 0,
            msg_iov: &iov as *const _ as *mut _,
            msg_iovlen: 1,
            msg_control: cmsg_buf.as_mut_ptr() as *mut _,
            msg_controllen: cmsg_space as _,
            #[cfg(target_os = "linux")]
            msg_flags: 0,
        };

        let received = unsafe { libc::recvmsg(sock_fd.as_raw_fd(), &msg as *const _ as *mut _, 0) };
        if received < 0 {
            return Err(IpcError::Io(io::Error::last_os_error()));
        }
        if received == 0 {
            return Err(IpcError::Disconnect);
        }

        data.extend_from_slice(&buf[..received as usize]);

        // Extract fds from ancillary data
        let cmsg = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        if !cmsg.is_null() {
            let cmsg_ptr = unsafe { &*cmsg };
            if cmsg_ptr.cmsg_level == libc::SOL_SOCKET && cmsg_ptr.cmsg_type == libc::SCM_RIGHTS {
                let data_ptr = unsafe { libc::CMSG_DATA(cmsg) as *const i32 };
                let header_len = unsafe { libc::CMSG_LEN(0) };
                let n_fds = (cmsg_ptr.cmsg_len - header_len as usize) / std::mem::size_of::<i32>();
                for i in 0..n_fds {
                    let raw_fd = unsafe { data_ptr.add(i).read() };
                    fds.push(unsafe { OwnedFd::from_raw_fd(raw_fd) });
                }
            }
        }

        Ok((data, fds))
    }

    /// Decode from raw bytes. Returns (version_u32, selector_bytes, payload_bytes).
    pub fn decode(bytes: &[u8]) -> std::result::Result<(u32, &[u8], &[u8]), IpcError> {
        if bytes.len() < 12 {
            return Err(IpcError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "message too short",
            )));
        }

        let version_u32 = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let selector_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let sel_aligned = selector_size.align4();

        let selector_start = 8;
        let selector_end = selector_start + selector_size;
        let payload_start = (8 + sel_aligned + 4) as usize;

        let payload_size = u32::from_le_bytes([
            bytes[payload_start - 4],
            bytes[payload_start - 3],
            bytes[payload_start - 2],
            bytes[payload_start - 1],
        ]) as usize;
        let payload_end = payload_start + payload_size;

        if payload_end > bytes.len() {
            return Err(IpcError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated message",
            )));
        }

        Ok((
            version_u32,
            &bytes[selector_start..selector_start + selector_size],
            &bytes[payload_start..payload_end],
        ))
    }
}
