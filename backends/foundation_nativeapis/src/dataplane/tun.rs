//! Kernel TUN device data plane (spec-55, feature 00).
//!
//! WHY: When an application must use the OS's own sockets transparently (native, with
//! `CAP_NET_ADMIN`), the kernel *is* the TCP/IP stack. We read raw IP packets off a TUN
//! fd (outbound, to encrypt) and write decapsulated packets back (inbound). This is the
//! opt-in, privileged, native-only counterpart to the smoltcp [`netstack`](super::netstack).
//!
//! WHAT: [`TunDevice`] opens `/dev/net/tun` (Linux) or a `utun` control socket (Darwin)
//! and does raw-IP `read`/`write`. [`TunDataPlane`] adapts it to [`DataPlane`]: inbound
//! decapsulated packets are written to the fd, `drain_outbound_ip` reads packets off the
//! fd for `Tunn::encapsulate`, and the overlay socket API returns
//! [`io::ErrorKind::Unsupported`] because apps use kernel sockets in this mode.
//!
//! HOW: The fd is non-blocking and registers with the existing poll reactor exactly like
//! [`UdpSocket`](crate::native::net::UdpSocket). Reference (not a dependency):
//! `boringtun/src/device/tun_linux.rs` and `tun_darwin.rs`.

use std::io;
use std::net::SocketAddr;
use std::time::Instant;

use super::netstack::{OverlayListener, OverlayStream, OverlayUdp};
use super::DataPlane;

/// Configuration for opening a kernel TUN device.
#[derive(Debug, Clone)]
pub struct TunConfig {
    /// Desired interface name (e.g. `"wg-ewe0"`). On Darwin the kernel assigns a
    /// `utunN` name; the requested name is advisory.
    pub name: String,
    /// Interface MTU. [`TunDevice::open`] applies it to the kernel interface, and
    /// [`TunDataPlane`] uses the same value as the size of one read buffer.
    pub mtu: u32,
}

impl TunConfig {
    /// WHY: Callers name their tunnel interface; the MTU defaults to a WireGuard-safe
    /// value but no name default is invented (spec: no silent defaults).
    ///
    /// WHAT: Build a config for interface `name` with a default MTU of 1420.
    ///
    /// HOW: Stores the values verbatim.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mtu: 1420,
        }
    }
}

// ---------------------------------------------------------------------------
// Platform TUN device
// ---------------------------------------------------------------------------

#[cfg(all(
    not(target_family = "wasm"),
    any(target_os = "linux", target_os = "macos")
))]
mod imp {
    use super::TunConfig;
    use std::io;
    use std::os::unix::io::{AsRawFd, RawFd};

    use crate::native::poll::{Interest, Registry, SourceFd, Token};

    /// Convert a byte count returned by `read` or `write` to `usize`.
    fn count_to_usize(n: libc::ssize_t) -> io::Result<usize> {
        usize::try_from(n).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "kernel returned a negative byte count",
            )
        })
    }

    /// Build an error for a value that does not fit its target field.
    fn out_of_range(what: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, what)
    }

    /// Copy `name` into the fixed-size kernel field, rejecting an over-long name.
    fn kernel_name(name: &str) -> io::Result<[libc::c_char; libc::IFNAMSIZ]> {
        let bytes = name.as_bytes();
        if bytes.len() >= libc::IFNAMSIZ {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tun interface name too long",
            ));
        }
        let mut out = [0; libc::IFNAMSIZ];
        for (dst, &b) in out.iter_mut().zip(bytes) {
            // C `char` keeps the byte's bits; reinterpret rather than convert.
            *dst = libc::c_char::from_ne_bytes([b]);
        }
        Ok(out)
    }

    /// Read the interface name the kernel wrote back into `ifr_name`.
    #[cfg(target_os = "linux")]
    fn name_from_kernel(raw: &[libc::c_char]) -> io::Result<String> {
        let bytes: Vec<u8> = raw
            .iter()
            .map(|c| u8::from_ne_bytes(c.to_ne_bytes()))
            .take_while(|&b| b != 0)
            .collect();
        String::from_utf8(bytes)
            .map_err(|_| out_of_range("the kernel returned a non-UTF-8 interface name"))
    }

    /// Open a close-on-exec datagram socket for interface ioctls.
    fn control_socket() -> io::Result<RawFd> {
        let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if sock < 0 {
            return Err(io::Error::last_os_error());
        }
        let flags = unsafe { libc::fcntl(sock, libc::F_GETFD) };
        if flags < 0 || unsafe { libc::fcntl(sock, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(sock) };
            return Err(err);
        }
        Ok(sock)
    }

    /// Set the MTU of interface `name`.
    ///
    /// WHY: The kernel drops packets larger than the interface MTU, and
    /// [`TunDataPlane`] sizes its read buffer from the configured value. A mismatch
    /// truncates packets.
    ///
    /// WHAT: Issues `SIOCSIFMTU` with `ifr_mtu = mtu` through a control socket.
    fn set_interface_mtu(name: &str, mtu: u32) -> io::Result<()> {
        let mtu =
            libc::c_int::try_from(mtu).map_err(|_| out_of_range("MTU does not fit in a C int"))?;
        let ifr_name = kernel_name(name)?;
        let sock = control_socket()?;
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = ifr_name;
        ifr.ifr_ifru.ifru_mtu = mtu;
        let rc = unsafe {
            libc::ioctl(
                sock,
                libc::SIOCSIFMTU,
                std::ptr::addr_of_mut!(ifr).cast::<libc::c_void>(),
            )
        };
        let err = if rc < 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        unsafe { libc::close(sock) };
        err.map_or(Ok(()), Err)
    }

    /// Read the MTU of interface `name` back from the kernel.
    fn read_interface_mtu(name: &str) -> io::Result<u32> {
        let ifr_name = kernel_name(name)?;
        let sock = control_socket()?;
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = ifr_name;
        let rc = unsafe {
            libc::ioctl(
                sock,
                libc::SIOCGIFMTU,
                std::ptr::addr_of_mut!(ifr).cast::<libc::c_void>(),
            )
        };
        if rc < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(sock) };
            return Err(err);
        }
        let mtu = unsafe { ifr.ifr_ifru.ifru_mtu };
        unsafe { libc::close(sock) };
        u32::try_from(mtu).map_err(|_| out_of_range("the kernel returned a negative MTU"))
    }

    /// Read the `utunN` name that the kernel assigned to the control socket.
    #[cfg(target_os = "macos")]
    fn utun_name(fd: RawFd) -> io::Result<String> {
        // <sys/kern_control.h>: the second option of the utun control is its name.
        const UTUN_OPT_IFNAME: libc::c_int = 2;
        let mut buf = [0u8; libc::IFNAMSIZ];
        let mut len = libc::socklen_t::try_from(buf.len())
            .map_err(|_| out_of_range("IFNAMSIZ does not fit in socklen_t"))?;
        let rc = unsafe {
            libc::getsockopt(
                fd,
                libc::SYSPROTO_CONTROL,
                UTUN_OPT_IFNAME,
                buf.as_mut_ptr().cast(),
                std::ptr::addr_of_mut!(len),
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8(buf[..end].to_vec())
            .map_err(|_| out_of_range("the kernel returned a non-UTF-8 interface name"))
    }

    /// An open kernel TUN device carrying raw IP packets.
    pub struct TunDevice {
        fd: RawFd,
        name: String,
        /// Darwin `utun` prepends a 4-byte address-family header on every packet.
        af_header: bool,
        /// Reused read buffer. It holds the caller's packet plus the Darwin header,
        /// so a steady stream of packets allocates nothing after the first read.
        scratch: Vec<u8>,
    }

    impl std::fmt::Debug for TunDevice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TunDevice")
                .field("fd", &self.fd)
                .field("name", &self.name)
                .field("af_header", &self.af_header)
                .field("scratch_bytes", &self.scratch.len())
                .finish()
        }
    }

    impl TunDevice {
        /// WHY: Establish the kernel data-plane interface for a `WireGuard` tunnel.
        ///
        /// WHAT: Open the TUN device described by `config` in non-blocking raw-IP mode,
        /// and set the interface MTU to `config.mtu`.
        ///
        /// HOW: Linux opens `/dev/net/tun` and issues `TUNSETIFF` with `IFF_TUN |
        /// IFF_NO_PI`; Darwin opens a `SYSPROTO_CONTROL` socket to
        /// `com.apple.net.utun_control`. Both then issue `SIOCSIFMTU` for the name the
        /// kernel assigned.
        ///
        /// # Errors
        /// Returns [`io::Error`] if the device cannot be opened (commonly `EPERM`
        /// without `CAP_NET_ADMIN`/root), or if the kernel rejects the MTU request.
        pub fn open(config: &TunConfig) -> io::Result<Self> {
            #[cfg(target_os = "linux")]
            {
                Self::open_linux(config)
            }
            #[cfg(target_os = "macos")]
            {
                Self::open_darwin(config)
            }
        }

        #[cfg(target_os = "linux")]
        fn open_linux(config: &TunConfig) -> io::Result<Self> {
            // TUNSETIFF = _IOW('T', 202, int) — see <linux/if_tun.h>.
            const TUNSETIFF: libc::c_ulong = 0x4004_54ca;

            let fd = unsafe {
                libc::open(
                    c"/dev/net/tun".as_ptr(),
                    libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
            ifr.ifr_name = match kernel_name(&config.name) {
                Ok(name) => name,
                Err(err) => {
                    unsafe { libc::close(fd) };
                    return Err(err);
                }
            };
            let Ok(flags) = libc::c_short::try_from(libc::IFF_TUN | libc::IFF_NO_PI) else {
                unsafe { libc::close(fd) };
                return Err(out_of_range("TUN flags do not fit in a C short"));
            };
            ifr.ifr_ifru.ifru_flags = flags;

            let rc = unsafe { libc::ioctl(fd, TUNSETIFF, &mut ifr) };
            if rc < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            // `TUNSETIFF` may rename the interface, so use the name the kernel
            // reports, not the requested one.
            let name = match name_from_kernel(&ifr.ifr_name) {
                Ok(name) => name,
                Err(err) => {
                    unsafe { libc::close(fd) };
                    return Err(err);
                }
            };
            if let Err(err) = set_interface_mtu(&name, config.mtu) {
                unsafe { libc::close(fd) };
                return Err(err);
            }

            Ok(Self {
                fd,
                name,
                af_header: false,
                scratch: Vec::new(),
            })
        }

        #[cfg(target_os = "macos")]
        fn open_darwin(config: &TunConfig) -> io::Result<Self> {
            const UTUN_CONTROL_NAME: &[u8] = b"com.apple.net.utun_control\0";

            let fd =
                unsafe { libc::socket(libc::PF_SYSTEM, libc::SOCK_DGRAM, libc::SYSPROTO_CONTROL) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let mut info: libc::ctl_info = unsafe { std::mem::zeroed() };
            for (dst, &b) in info.ctl_name.iter_mut().zip(UTUN_CONTROL_NAME) {
                // C `char` keeps the byte's bits; reinterpret rather than convert.
                *dst = libc::c_char::from_ne_bytes([b]);
            }
            if unsafe { libc::ioctl(fd, libc::CTLIOCGINFO, &mut info) } < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            // Request a specific utun unit if the name is `utunN`; otherwise let the
            // kernel pick (sc_unit = 0).
            let unit: u32 = config
                .name
                .strip_prefix("utun")
                .and_then(|n| n.parse::<u32>().ok())
                .map_or(0, |n| n + 1);

            let sc_len = u8::try_from(std::mem::size_of::<libc::sockaddr_ctl>())
                .map_err(|_| out_of_range("sockaddr_ctl is too large for sc_len"))?;
            let sc_family = u8::try_from(libc::AF_SYSTEM)
                .map_err(|_| out_of_range("AF_SYSTEM does not fit in sc_family"))?;
            let ss_sysaddr = u16::try_from(libc::AF_SYS_CONTROL)
                .map_err(|_| out_of_range("AF_SYS_CONTROL does not fit in ss_sysaddr"))?;
            let addr_len = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_ctl>())
                .map_err(|_| out_of_range("sockaddr_ctl is too large for socklen_t"))?;
            let addr = libc::sockaddr_ctl {
                sc_len,
                sc_family,
                ss_sysaddr,
                sc_id: info.ctl_id,
                sc_unit: unit,
                sc_reserved: [0; 5],
            };
            let rc = unsafe {
                libc::connect(
                    fd,
                    std::ptr::addr_of!(addr).cast::<libc::sockaddr>(),
                    addr_len,
                )
            };
            if rc < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            // Switch to non-blocking mode.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
            if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            let name = match utun_name(fd) {
                Ok(name) => name,
                Err(err) => {
                    unsafe { libc::close(fd) };
                    return Err(err);
                }
            };
            if let Err(err) = set_interface_mtu(&name, config.mtu) {
                unsafe { libc::close(fd) };
                return Err(err);
            }

            Ok(Self {
                fd,
                name,
                af_header: true,
                scratch: Vec::new(),
            })
        }

        /// Read one raw IP packet into `buf`. `Ok(0)` on `WouldBlock` is mapped to an
        /// error of kind [`io::ErrorKind::WouldBlock`].
        ///
        /// # Errors
        /// Returns [`io::Error`]; kind `WouldBlock` when no packet is available.
        pub fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            // Darwin prepends a four-byte address-family header that the caller does
            // not expect, so read into a scratch buffer with room for it. The size
            // follows the caller's packet buffer; it has no fixed limit.
            let header_len = if self.af_header { 4 } else { 0 };
            let want = buf.len().saturating_add(header_len);
            if self.scratch.len() < want {
                self.scratch.resize(want, 0);
            }
            let raw = &mut self.scratch[..want];
            let n = unsafe { libc::read(self.fd, raw.as_mut_ptr().cast(), raw.len()) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            let n = count_to_usize(n)?;
            let offset = if self.af_header { 4.min(n) } else { 0 };
            let payload = &raw[offset..n];
            let len = payload.len().min(buf.len());
            buf[..len].copy_from_slice(&payload[..len]);
            Ok(len)
        }

        /// Write one raw IP packet.
        ///
        /// # Errors
        /// Returns [`io::Error`]; kind `WouldBlock` when the device queue is full.
        pub fn write(&self, packet: &[u8]) -> io::Result<usize> {
            if self.af_header {
                // Prepend the 4-byte address family header Darwin utun expects.
                let af: u32 = if !packet.is_empty() && (packet[0] >> 4) == 6 {
                    libc::AF_INET6 as u32
                } else {
                    libc::AF_INET as u32
                };
                let mut framed = Vec::with_capacity(packet.len() + 4);
                framed.extend_from_slice(&af.to_be_bytes());
                framed.extend_from_slice(packet);
                let n = unsafe { libc::write(self.fd, framed.as_ptr().cast(), framed.len()) };
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(count_to_usize(n)?.saturating_sub(4))
            } else {
                let n = unsafe { libc::write(self.fd, packet.as_ptr().cast(), packet.len()) };
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(count_to_usize(n)?)
            }
        }

        /// Register the TUN fd with the poll reactor for readability (outbound packets).
        ///
        /// # Errors
        /// Returns [`io::Error`] if registration fails.
        pub fn register(&self, registry: &Registry, token: Token) -> io::Result<()> {
            registry.register(&mut SourceFd(self.fd), token, Interest::READABLE)
        }

        /// The kernel-facing interface name.
        #[must_use]
        pub fn name(&self) -> &str {
            &self.name
        }

        /// Read the interface MTU back from the kernel.
        ///
        /// # Errors
        /// Returns [`io::Error`] if the kernel query fails.
        pub fn mtu(&self) -> io::Result<u32> {
            read_interface_mtu(&self.name)
        }
    }

    impl AsRawFd for TunDevice {
        fn as_raw_fd(&self) -> RawFd {
            self.fd
        }
    }

    impl Drop for TunDevice {
        fn drop(&mut self) {
            unsafe { libc::close(self.fd) };
        }
    }
}

// wasm / unsupported-OS stub: compiles but errors if selected.
#[cfg(not(all(
    not(target_family = "wasm"),
    any(target_os = "linux", target_os = "macos")
)))]
mod imp {
    use super::TunConfig;
    use std::io;

    /// Stub TUN device for platforms without a kernel TUN interface (e.g. wasm). Every
    /// operation returns [`io::ErrorKind::Unsupported`].
    #[derive(Debug)]
    pub struct TunDevice {
        _private: (),
    }

    impl TunDevice {
        /// Always fails on unsupported platforms.
        ///
        /// # Errors
        /// Always returns [`io::ErrorKind::Unsupported`].
        pub fn open(_config: &TunConfig) -> io::Result<Self> {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "kernel TUN device is not available on this platform",
            ))
        }

        /// Always fails on unsupported platforms.
        ///
        /// # Errors
        /// Always returns [`io::ErrorKind::Unsupported`].
        pub fn read(&self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "no TUN device"))
        }

        /// Always fails on unsupported platforms.
        ///
        /// # Errors
        /// Always returns [`io::ErrorKind::Unsupported`].
        pub fn write(&self, _packet: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::Unsupported, "no TUN device"))
        }

        /// The kernel-facing interface name (empty on stub platforms).
        #[must_use]
        pub fn name(&self) -> &str {
            ""
        }
    }
}

pub use imp::TunDevice;

// ---------------------------------------------------------------------------
// TunDataPlane
// ---------------------------------------------------------------------------

/// A [`DataPlane`] backed by a kernel TUN device.
///
/// In this mode the kernel owns the TCP/IP stack, so the overlay socket API is
/// unavailable — applications use ordinary OS sockets. `inject_inbound_ip` writes
/// decapsulated packets to the fd; `drain_outbound_ip` reads raw IP packets off the fd
/// to be encrypted.
pub struct TunDataPlane {
    device: TunDevice,
    /// One interface MTU, reused for every read so a steady stream of outbound
    /// packets does not allocate per packet.
    buffer: Vec<u8>,
}

impl std::fmt::Debug for TunDataPlane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunDataPlane")
            .field("device", &self.device)
            .field("buffer_bytes", &self.buffer.len())
            .finish_non_exhaustive()
    }
}

impl TunDataPlane {
    /// WHY: Wrap an opened TUN device as a data plane the mesh orchestrator can drive.
    ///
    /// WHAT: Open the device from `config` and adapt it to [`DataPlane`].
    ///
    /// HOW: Delegates to [`TunDevice::open`].
    ///
    /// # Errors
    /// Returns [`io::Error`] if the device cannot be opened (see [`TunDevice::open`]).
    pub fn open(config: &TunConfig) -> io::Result<Self> {
        // The interface MTU bounds the largest packet the kernel delivers, so one MTU
        // is the read buffer size. `u32` fits in `usize` on every supported target.
        let buffer_bytes = usize::try_from(config.mtu).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("MTU {} does not fit in usize", config.mtu),
            )
        })?;
        Ok(Self {
            device: TunDevice::open(config)?,
            buffer: vec![0u8; buffer_bytes],
        })
    }

    /// Access the underlying device (e.g. to register its fd with a reactor).
    #[must_use]
    pub fn device(&self) -> &TunDevice {
        &self.device
    }
}

impl DataPlane for TunDataPlane {
    fn inject_inbound_ip(&mut self, packet: &[u8]) {
        // Best-effort: a full device queue drops the packet (WireGuard is lossy anyway).
        if let Err(err) = self.device.write(packet) {
            if err.kind() != io::ErrorKind::WouldBlock {
                tracing::debug!(error = %err, "tun write failed");
            }
        }
    }

    fn poll(&mut self, _now: Instant) -> Option<Instant> {
        // The kernel drives all timers; nothing to advance in-process.
        None
    }

    fn drain_outbound_ip(&mut self, f: &mut dyn FnMut(&[u8])) {
        loop {
            match self.device.read(&mut self.buffer) {
                Ok(0) => break,
                Ok(n) => f(&self.buffer[..n]),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    tracing::debug!(error = %err, "tun read failed");
                    break;
                }
            }
        }
    }

    fn tcp_listen(&mut self, _addr: SocketAddr) -> io::Result<OverlayListener> {
        Err(unsupported())
    }

    fn tcp_connect(&mut self, _addr: SocketAddr) -> io::Result<OverlayStream> {
        Err(unsupported())
    }

    fn udp_bind(&mut self, _addr: SocketAddr) -> io::Result<OverlayUdp> {
        Err(unsupported())
    }
}

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "kernel TUN mode uses OS sockets; overlay sockets are unavailable",
    )
}
