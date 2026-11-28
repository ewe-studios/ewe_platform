//! Ethernet TAP data plane (spec 92, feature 02, task T11).
//!
//! WHY: A Firecracker guest sends Ethernet frames through a virtio-net device. The
//! host-side TAP device carries those Layer 2 (L2) frames. The existing netstack speaks
//! Layer 3 (L3) only, so a micro virtual machine (microVM) needs an Ethernet-mode stack
//! with a raw TAP descriptor as its wire.
//!
//! WHAT: [`TapDevice`] opens `/dev/net/tun` in TAP mode and reads and writes one
//! Ethernet frame at a time. [`TapDataPlane`] composes a [`NetStack`] in Ethernet mode
//! with any [`FrameIo`]: it feeds inbound frames into the stack, polls it, and writes
//! the outbound frames back. The stack exposes the same overlay Transmission Control
//! Protocol (TCP) and User Datagram Protocol (UDP) sockets as the `WireGuard` data
//! plane, so an in-process server can serve the guest.
//!
//! HOW: The device descriptor is non-blocking. A driver polls it for readability and
//! calls [`TapDataPlane::service`] on each wake. The descriptor belongs to a private
//! network namespace that the creator makes, so the guest sees only this stack and not
//! the host interfaces.
//!
//! The Linux-only device has a stub on every other target. The Ethernet path of
//! [`NetStack`] and the [`TapDataPlane`] logic build and run everywhere, which is why
//! the unit test needs no TAP descriptor.

use std::io;
use std::time::Instant;

use foundation_context::Ctx;

use super::netstack::{NetStack, NetStackConfig, ETHERNET_HEADER_LEN};
use super::DataPlane;

/// Default largest payload of one TAP link, in bytes. 1500 bytes is the Ethernet
/// payload size that the Linux kernel and nginx use.
pub const DEFAULT_TAP_MTU: usize = 1500;

/// Default number of frames that one [`TapDataPlane::service`] call moves in each
/// direction. 64 frames keeps one wake bounded while it lets a burst drain.
pub const DEFAULT_MAX_FRAMES_PER_SERVICE: usize = 64;

/// Configuration of a TAP device and its service loop.
#[derive(Debug, Clone)]
pub struct TapConfig {
    /// The kernel interface name, such as `"fc-tap0"`. The name lives in the private
    /// network namespace, so two sandboxes can use the same name.
    pub name: String,
    /// The link MTU in bytes, without the Ethernet header. The device advertises
    /// `mtu + 14` to the stack.
    pub mtu: usize,
    /// The most frames that one service call moves in each direction.
    pub max_frames_per_service: usize,
}

impl TapConfig {
    /// A configuration for the interface `name`.
    ///
    /// The defaults are an MTU of [`DEFAULT_TAP_MTU`] and
    /// [`DEFAULT_MAX_FRAMES_PER_SERVICE`] frames per service call.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mtu: DEFAULT_TAP_MTU,
            max_frames_per_service: DEFAULT_MAX_FRAMES_PER_SERVICE,
        }
    }

    /// Override the link MTU.
    #[must_use]
    pub fn with_mtu(mut self, mtu: usize) -> Self {
        self.mtu = mtu;
        self
    }

    /// Override the most frames that one service call moves in each direction.
    #[must_use]
    pub fn with_max_frames_per_service(mut self, frames: usize) -> Self {
        self.max_frames_per_service = frames.max(1);
        self
    }
}

/// One bidirectional source of Ethernet frames.
///
/// [`TapDevice`] is the real source. A test supplies an in-memory queue, so the
/// Ethernet path runs on every operating system.
pub trait FrameIo: Send {
    /// Read one inbound frame into `buf`, returning its length. Returns
    /// [`io::ErrorKind::WouldBlock`] when no frame is ready.
    ///
    /// # Errors
    /// [`io::Error`] from the device.
    fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<usize>;

    /// Write one outbound frame.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when the device queue is full; other kinds on a
    /// device error.
    fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize>;

    /// The link MTU in bytes, without the Ethernet header.
    fn mtu(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Linux TAP device
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod imp {
    use std::ffi::CString;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

    use super::{FrameIo, TapConfig};

    /// The `ifreq` flags of a TAP device: Layer 2 frames and no 4-byte prefix.
    const IFF_TAP: libc::c_short = 0x0002;
    /// Do not prepend the packet information header.
    const IFF_NO_PI: libc::c_short = 0x1000;
    /// The `ioctl` that creates or attaches a TAP device.
    const TUNSETIFF: libc::c_ulong = 0x4004_54ca;
    /// The `ioctl` that sets the interface flags.
    const SIOCSIFFLAGS: libc::c_ulong = 0x8914;
    /// The `ioctl` that sets the interface MTU.
    const SIOCSIFMTU: libc::c_ulong = 0x8922;
    /// The flag that makes an interface active.
    const IFF_UP: libc::c_short = 0x1;

    /// An open kernel TAP device carrying Ethernet frames.
    pub struct TapDevice {
        fd: OwnedFd,
        name: String,
        mtu: usize,
    }

    impl std::fmt::Debug for TapDevice {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("TapDevice")
                .field("fd", &self.fd.as_raw_fd())
                .field("name", &self.name)
                .field("mtu", &self.mtu)
                .finish()
        }
    }

    impl TapDevice {
        /// Create and open a TAP device in the current network namespace.
        ///
        /// WHY: A control plane starts a guest in a private namespace, so the TAP is
        /// made in that namespace and the Firecracker process joins it.
        ///
        /// WHAT: Open `/dev/net/tun`, attach the interface `config.name` in TAP mode
        /// with no packet header, set its MTU and set it up.
        ///
        /// HOW: `TUNSETIFF` with `IFF_TAP | IFF_NO_PI`; `SIOCSIFMTU` and
        /// `SIOCSIFFLAGS` through a control socket. The descriptor is non-blocking and
        /// close-on-exec.
        ///
        /// # Errors
        /// [`io::Error`] when the device cannot be opened (commonly `EPERM` without
        /// `CAP_NET_ADMIN`), when the name is too long, or when the kernel rejects the
        /// MTU or the up flag.
        pub fn open(config: &TapConfig) -> io::Result<Self> {
            let path = CString::new("/dev/net/tun").map_err(|_| invalid("the device path"))?;
            let fd = unsafe {
                libc::open(
                    path.as_ptr(),
                    libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the call returned a new descriptor that this process owns.
            let fd = unsafe { OwnedFd::from_raw_fd(fd) };
            let name = attach(fd.as_raw_fd(), &config.name)?;
            let mtu = u32::try_from(config.mtu)
                .map_err(|_| invalid("the MTU does not fit in 32 bits"))?;
            let mtu = set_up(&name, mtu)?;
            let mtu = usize::try_from(mtu).map_err(|_| invalid("the MTU does not fit in usize"))?;
            Ok(Self { fd, name, mtu })
        }

        /// Adopt an open TAP descriptor, such as one received from a helper process.
        ///
        /// The descriptor must be non-blocking and close-on-exec, and the interface must
        /// already be up with `mtu` set.
        ///
        /// # Panics
        /// Never panics.
        #[must_use]
        pub fn from_owned_fd(fd: OwnedFd, name: impl Into<String>, mtu: usize) -> Self {
            Self {
                fd,
                name: name.into(),
                mtu,
            }
        }

        /// The kernel-facing interface name.
        #[must_use]
        pub fn name(&self) -> &str {
            &self.name
        }

        /// The link MTU in bytes, without the Ethernet header.
        #[must_use]
        pub fn mtu(&self) -> usize {
            self.mtu
        }
    }

    impl AsRawFd for TapDevice {
        fn as_raw_fd(&self) -> RawFd {
            self.fd.as_raw_fd()
        }
    }

    impl FrameIo for TapDevice {
        fn read_frame(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = unsafe { libc::read(self.fd.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            usize::try_from(n).map_err(|_| invalid("the kernel returned a negative length"))
        }

        fn write_frame(&mut self, frame: &[u8]) -> io::Result<usize> {
            let n = unsafe { libc::write(self.fd.as_raw_fd(), frame.as_ptr().cast(), frame.len()) };
            if n < 0 {
                return Err(io::Error::last_os_error());
            }
            usize::try_from(n).map_err(|_| invalid("the kernel returned a negative length"))
        }

        fn mtu(&self) -> usize {
            self.mtu
        }
    }

    /// Copy `name` into the kernel field of `ifreq`, rejecting an over-long name.
    fn kernel_name(name: &str) -> io::Result<[libc::c_char; libc::IFNAMSIZ]> {
        let bytes = name.as_bytes();
        if bytes.len() >= libc::IFNAMSIZ {
            return Err(invalid("the TAP interface name is too long"));
        }
        let mut out = [0; libc::IFNAMSIZ];
        for (dst, &b) in out.iter_mut().zip(bytes) {
            *dst = libc::c_char::from_ne_bytes([b]);
        }
        Ok(out)
    }

    /// Attach the interface `name` to `fd` in TAP mode and return the kernel name.
    fn attach(fd: RawFd, name: &str) -> io::Result<String> {
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = kernel_name(name)?;
        ifr.ifr_ifru.ifru_flags = IFF_TAP | IFF_NO_PI;
        let rc = unsafe { libc::ioctl(fd, TUNSETIFF, &mut ifr) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        let bytes: Vec<u8> = ifr
            .ifr_name
            .iter()
            .map(|c| u8::from_ne_bytes(c.to_ne_bytes()))
            .take_while(|&b| b != 0)
            .collect();
        String::from_utf8(bytes).map_err(|_| invalid("the kernel returned a non-UTF-8 name"))
    }

    /// A close-on-exec datagram socket for the interface `ioctl` calls.
    fn control_socket() -> io::Result<OwnedFd> {
        let sock = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if sock < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the call returned a new descriptor that this process owns.
        Ok(unsafe { OwnedFd::from_raw_fd(sock) })
    }

    /// Set the MTU and the up flag, and read the MTU back from the kernel.
    fn set_up(name: &str, mtu: u32) -> io::Result<u32> {
        let mtu = libc::c_int::try_from(mtu).map_err(|_| invalid("the MTU is too large"))?;
        // All three `ifreq` requests go to an `AF_INET` socket: the TAP descriptor
        // handles only the TUN-specific requests.
        let control = control_socket()?;
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = kernel_name(name)?;
        ifr.ifr_ifru.ifru_mtu = mtu;
        let rc = unsafe { libc::ioctl(control.as_raw_fd(), SIOCSIFMTU, &mut ifr) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = kernel_name(name)?;
        let rc = unsafe { libc::ioctl(control.as_raw_fd(), libc::SIOCGIFFLAGS, &mut ifr) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `SIOCGIFFLAGS` filled the flags member of the union.
        unsafe { ifr.ifr_ifru.ifru_flags |= IFF_UP };
        let rc = unsafe { libc::ioctl(control.as_raw_fd(), SIOCSIFFLAGS, &mut ifr) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        u32::try_from(mtu).map_err(|_| invalid("the MTU is negative"))
    }

    /// An error of one rejected value.
    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message)
    }
}

// Stub for every other target: compiles, and reports that a TAP needs Linux.
#[cfg(not(target_os = "linux"))]
mod imp {
    use std::io;
    #[cfg(unix)]
    use std::os::fd::OwnedFd;

    use super::{FrameIo, TapConfig};

    /// A stub TAP device on a target without `/dev/net/tun`.
    #[derive(Debug)]
    pub struct TapDevice {
        _private: (),
    }

    impl TapDevice {
        /// Always fails on this target.
        ///
        /// # Errors
        /// Always returns [`io::ErrorKind::Unsupported`].
        pub fn open(_config: &TapConfig) -> io::Result<Self> {
            Err(unsupported())
        }

        /// Adopt an open TAP descriptor.
        ///
        /// A descriptor exists only on a Unix target, so a Windows build has no such
        /// method.
        ///
        /// # Panics
        /// Never panics.
        #[cfg(unix)]
        #[must_use]
        pub fn from_owned_fd(_fd: OwnedFd, _name: impl Into<String>, _mtu: usize) -> Self {
            Self { _private: () }
        }

        /// The kernel-facing interface name (empty on a stub target).
        #[must_use]
        pub fn name(&self) -> &str {
            ""
        }

        /// The link MTU (zero on a stub target).
        #[must_use]
        pub fn mtu(&self) -> usize {
            0
        }
    }

    impl FrameIo for TapDevice {
        fn read_frame(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(unsupported())
        }

        fn write_frame(&mut self, _frame: &[u8]) -> io::Result<usize> {
            Err(unsupported())
        }

        fn mtu(&self) -> usize {
            0
        }
    }

    /// An unsupported-target error.
    fn unsupported() -> io::Error {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "a TAP device is available on Linux only",
        )
    }
}

pub use imp::TapDevice;

// ---------------------------------------------------------------------------
// TapDataPlane
// ---------------------------------------------------------------------------

/// A [`NetStack`] in Ethernet mode with a frame source as its wire.
///
/// The generic parameter is the wire. [`TapDevice`] is the real one; a test uses an
/// in-memory queue, so the full frame path runs without privileges and on every
/// operating system.
pub struct TapDataPlane<D: FrameIo = TapDevice> {
    io: D,
    stack: NetStack,
    /// One frame buffer, reused for every read so a steady stream allocates nothing.
    buffer: Vec<u8>,
    max_frames_per_service: usize,
}

impl<D: FrameIo> std::fmt::Debug for TapDataPlane<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapDataPlane")
            .field("mtu", &self.io.mtu())
            .field("max_frames_per_service", &self.max_frames_per_service)
            .finish_non_exhaustive()
    }
}

impl TapDataPlane<TapDevice> {
    /// Open the TAP device described by `config` and put an Ethernet stack on it.
    ///
    /// # Errors
    /// [`io::Error`] from [`TapDevice::open`].
    pub fn open(config: &TapConfig, stack_config: NetStackConfig, ctx: &Ctx) -> io::Result<Self> {
        Ok(Self::new(
            TapDevice::open(config)?,
            stack_config,
            config.max_frames_per_service,
            ctx,
        ))
    }
}

impl<D: FrameIo> TapDataPlane<D> {
    /// Put an Ethernet stack on `io` with the given stack configuration.
    ///
    /// The stack must use [`LinkMedium::Ethernet`](super::netstack::LinkMedium); its
    /// media access control (MAC) address is the one that the peer resolves.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(
        io: D,
        stack_config: NetStackConfig,
        max_frames_per_service: usize,
        ctx: &Ctx,
    ) -> Self {
        let buffer = vec![0u8; io.mtu() + ETHERNET_HEADER_LEN];
        Self {
            io,
            stack: NetStack::new(stack_config, ctx),
            buffer,
            max_frames_per_service: max_frames_per_service.max(1),
        }
    }

    /// Move every ready inbound frame into the stack, poll it, and write every outbound
    /// frame to the wire.
    ///
    /// Returns the next instant at which the stack wants a poll, or `None` when it is
    /// idle.
    ///
    /// # Errors
    /// [`io::Error`] from the wire, except [`io::ErrorKind::WouldBlock`], which means
    /// that the queue is empty or full and is not an error.
    pub fn service(&mut self, now: Instant) -> io::Result<Option<Instant>> {
        for _ in 0..self.max_frames_per_service {
            match self.io.read_frame(&mut self.buffer) {
                Ok(0) => break,
                Ok(n) => self.stack.inject_inbound_frame(&self.buffer[..n]),
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => return Err(err),
            }
        }

        let deadline = self.stack.poll(now);

        let mut failure = None;
        let mut stack = self.stack.clone();
        stack.drain_outbound_frame(&mut |frame| {
            if let Err(err) = self.io.write_frame(frame) {
                if err.kind() != io::ErrorKind::WouldBlock {
                    failure = Some(err);
                }
            }
        });
        if let Some(err) = failure {
            return Err(err);
        }
        Ok(deadline)
    }

    /// The stack on this plane, for its overlay sockets.
    #[must_use]
    pub fn stack(&self) -> &NetStack {
        &self.stack
    }

    /// The frame wire.
    #[must_use]
    pub fn io(&self) -> &D {
        &self.io
    }

    /// The frame wire, for a test to inspect.
    #[must_use]
    pub fn io_mut(&mut self) -> &mut D {
        &mut self.io
    }
}
