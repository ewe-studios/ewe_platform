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

/// Maximum IP packet we will read from the TUN device in one call.
const MAX_PACKET: usize = 65_535;

/// Configuration for opening a kernel TUN device.
#[derive(Debug, Clone)]
pub struct TunConfig {
    /// Desired interface name (e.g. `"wg-ewe0"`). On Darwin the kernel assigns a
    /// `utunN` name; the requested name is advisory.
    pub name: String,
    /// Interface MTU to request.
    pub mtu: u32,
    /// Whether to enable IPv6 on the interface.
    pub ipv6: bool,
}

impl TunConfig {
    /// WHY: Callers name their tunnel interface; the MTU defaults to a WireGuard-safe
    /// value but no name default is invented (spec: no silent defaults).
    ///
    /// WHAT: Build a config for interface `name` with a default MTU of 1420 and IPv6 on.
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
            ipv6: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Platform TUN device
// ---------------------------------------------------------------------------

#[cfg(all(not(target_family = "wasm"), any(target_os = "linux", target_os = "macos")))]
mod imp {
    use super::{TunConfig, MAX_PACKET};
    use std::io;
    use std::os::unix::io::{AsRawFd, RawFd};

    use crate::native::poll::{Interest, Registry, SourceFd, Token};

    /// An open kernel TUN device carrying raw IP packets.
    pub struct TunDevice {
        fd: RawFd,
        name: String,
        /// Darwin `utun` prepends a 4-byte address-family header on every packet.
        af_header: bool,
    }

    impl std::fmt::Debug for TunDevice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TunDevice")
                .field("fd", &self.fd)
                .field("name", &self.name)
                .finish()
        }
    }

    impl TunDevice {
        /// WHY: Establish the kernel data-plane interface for a WireGuard tunnel.
        ///
        /// WHAT: Open the TUN device described by `config` in non-blocking raw-IP mode.
        ///
        /// HOW: Linux opens `/dev/net/tun` and issues `TUNSETIFF` with `IFF_TUN |
        /// IFF_NO_PI`; Darwin opens a `SYSPROTO_CONTROL` socket to
        /// `com.apple.net.utun_control`.
        ///
        /// # Errors
        /// Returns [`io::Error`] if the device cannot be opened (commonly `EPERM`
        /// without `CAP_NET_ADMIN`/root).
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
            let name_bytes = config.name.as_bytes();
            if name_bytes.len() >= libc::IFNAMSIZ {
                unsafe { libc::close(fd) };
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "tun interface name too long",
                ));
            }
            for (dst, &b) in ifr.ifr_name.iter_mut().zip(name_bytes) {
                *dst = b as libc::c_char;
            }
            ifr.ifr_ifru.ifru_flags = (libc::IFF_TUN | libc::IFF_NO_PI) as libc::c_short;

            let rc = unsafe { libc::ioctl(fd, TUNSETIFF, &mut ifr) };
            if rc < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            Ok(Self {
                fd,
                name: config.name.clone(),
                af_header: false,
            })
        }

        #[cfg(target_os = "macos")]
        fn open_darwin(config: &TunConfig) -> io::Result<Self> {
            const UTUN_CONTROL_NAME: &[u8] = b"com.apple.net.utun_control\0";

            let fd = unsafe {
                libc::socket(libc::PF_SYSTEM, libc::SOCK_DGRAM, libc::SYSPROTO_CONTROL)
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }

            let mut info: libc::ctl_info = unsafe { std::mem::zeroed() };
            for (dst, &b) in info.ctl_name.iter_mut().zip(UTUN_CONTROL_NAME) {
                *dst = b as libc::c_char;
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

            let addr = libc::sockaddr_ctl {
                sc_len: std::mem::size_of::<libc::sockaddr_ctl>() as u8,
                sc_family: libc::AF_SYSTEM as u8,
                ss_sysaddr: libc::AF_SYS_CONTROL as u16,
                sc_id: info.ctl_id,
                sc_unit: unit,
                sc_reserved: [0; 5],
            };
            let rc = unsafe {
                libc::connect(
                    fd,
                    std::ptr::addr_of!(addr).cast::<libc::sockaddr>(),
                    std::mem::size_of::<libc::sockaddr_ctl>() as libc::socklen_t,
                )
            };
            if rc < 0 {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            // Switch to non-blocking mode.
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFL, 0) };
            if flags < 0
                || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
            {
                let err = io::Error::last_os_error();
                unsafe { libc::close(fd) };
                return Err(err);
            }

            Ok(Self {
                fd,
                name: config.name.clone(),
                af_header: true,
            })
        }

        /// Read one raw IP packet into `buf`. `Ok(0)` on `WouldBlock` is mapped to an
        /// error of kind [`io::ErrorKind::WouldBlock`].
        ///
        /// # Errors
        /// Returns [`io::Error`]; kind `WouldBlock` when no packet is available.
        pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
            let mut raw = [0u8; MAX_PACKET + 4];
            let want = buf.len().min(MAX_PACKET) + if self.af_header { 4 } else { 0 };
            let n = unsafe {
                libc::read(self.fd, raw.as_mut_ptr().cast(), want.min(raw.len()))
            };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            let n = n as usize;
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
                let n = unsafe {
                    libc::write(self.fd, framed.as_ptr().cast(), framed.len())
                };
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok((n as usize).saturating_sub(4))
            } else {
                let n = unsafe { libc::write(self.fd, packet.as_ptr().cast(), packet.len()) };
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(n as usize)
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
#[cfg(not(all(not(target_family = "wasm"), any(target_os = "linux", target_os = "macos"))))]
mod imp {
    use super::{TunConfig, MAX_PACKET};
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
            let _ = MAX_PACKET;
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
#[derive(Debug)]
pub struct TunDataPlane {
    device: TunDevice,
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
        Ok(Self {
            device: TunDevice::open(config)?,
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
        let mut buf = [0u8; MAX_PACKET];
        loop {
            match self.device.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => f(&buf[..n]),
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
