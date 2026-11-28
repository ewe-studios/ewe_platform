//! The Firecracker guest network (spec 92, feature 02, task T11).
//!
//! WHY: A Firecracker guest sends Ethernet frames through a virtio-net device, and a
//! TAP device carries those frames on the host. A TAP device has one holder, so the
//! Firecracker process cannot share it with a user-space stack. The guest network
//! therefore has two parts. The `agent-fc-net` helper creates a private user and
//! network namespace, makes the TAP device there, and bridges its frames to this
//! process over a Unix datagram pair with an `AF_PACKET` socket. This process runs the
//! smoltcp stack from `foundation_nativeapis` in Ethernet mode and relays the
//! configured ports to host services. No host route, firewall rule or Network Address
//! Translation (NAT) rule is needed.
//!
//! WHAT: [`GuestNetworkConfig`] holds every value of the link. [`NetworkRuntime`]
//! runs the stack and the relays in the control plane. [`run_helper`] is the entry
//! point of the helper program.
//!
//! HOW: The control plane spawns the helper with the Firecracker program after `--`.
//! The helper calls `unshare(2)` for a user namespace and a network namespace, makes
//! the TAP device, opens an `AF_PACKET` socket on it, starts Firecracker, and copies
//! frames between the packet socket and descriptor 3, which is one end of the Unix
//! datagram pair. It runs no `ip` command. The control plane copies frames between its
//! end and the stack. The namespace ends when its last holder ends: the helper, which
//! Firecracker's parent-death signal ends with it. The control plane stops the runtime
//! by its own thread handle and kills the helper by its saved process handle; it never
//! runs a command such as `pkill`.

use std::io::{Read as _, Write as _};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::os::fd::{AsRawFd as _, FromRawFd as _};
use std::os::unix::net::UnixDatagram;
use std::os::unix::process::CommandExt as _;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::Duration;

use foundation_context::Ctx;
use foundation_nativeapis::dataplane::netstack::{
    NetStackConfig, OverlayListener, OverlayStream, OverlayUdp, ETHERNET_HEADER_LEN,
};
use foundation_nativeapis::dataplane::tap::{FrameIo, TapDataPlane, DEFAULT_TAP_MTU};
use foundation_nativeapis::dataplane::{DataPlane, WakeFn};

use crate::shared::error::{SandboxError, SandboxResult};

/// The default guest interface name. `net.ifnames=0` makes the kernel use it.
pub const DEFAULT_INTERFACE: &str = "eth0";

/// The default TAP interface name. It lives in the private network namespace, so
/// two sandboxes can use the same name.
pub const DEFAULT_TAP_NAME: &str = "fc-tap0";

/// The default MAC address of the stack on the TAP side. The first byte has the
/// locally administered bit, so it never collides with a vendor address.
pub const DEFAULT_HOST_MAC: [u8; 6] = [0x06, 0x00, 0xac, 0x10, 0x00, 0x01];

/// The default MAC address of the guest interface. The first byte has the locally
/// administered bit, so it never collides with a vendor address.
pub const DEFAULT_GUEST_MAC: [u8; 6] = [0x06, 0x00, 0xac, 0x10, 0x00, 0x02];

/// The default address of the stack on the TAP link. It is a link-local address
/// (169.254.0.0/16), so it never collides with a host route.
pub const DEFAULT_HOST_IP: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 1);

/// The default address of the guest interface. It is in the same /30 as
/// [`DEFAULT_HOST_IP`].
pub const DEFAULT_GUEST_IP: Ipv4Addr = Ipv4Addr::new(169, 254, 169, 2);

/// The default prefix length. A /30 holds exactly the two link addresses.
pub const DEFAULT_PREFIX_LEN: u8 = 30;

/// The type of an `ioctl` request number. The C libraries disagree: the GNU C library
/// takes an unsigned long, and musl takes an int. Only a musl build shows the
/// difference.
#[cfg(target_env = "musl")]
type IoctlRequest = libc::c_int;
#[cfg(not(target_env = "musl"))]
type IoctlRequest = libc::c_ulong;

/// The descriptor number that carries frames between the helper and this process.
pub const FRAME_DESCRIPTOR: i32 = 3;

/// The largest Ethernet frame that the frame channel can carry.
///
/// A guest that negotiates segmentation offload sends one large frame instead of many
/// small ones. The largest IPv4 packet (65 535 bytes) plus its link header is 65 549
/// bytes, so this buffer never truncates a frame. This is a protocol bound, not a
/// tuning value.
const MAX_FRAME_BYTES: usize = 65_535 + 14;

/// One guest TCP port that the host answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TcpForward {
    /// The port on the guest-facing stack address.
    pub guest_port: u16,
    /// The host service that receives the connection.
    pub host: SocketAddr,
}

/// One guest UDP port that the host answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UdpForward {
    /// The port on the guest-facing stack address.
    pub guest_port: u16,
    /// The host service that receives the datagrams.
    pub host: SocketAddr,
}

/// Every value of the Firecracker guest link.
///
/// Build a value with [`GuestNetworkConfig::default`] and change what matters
/// with the `with_` methods. The driver enables the guest network only when the
/// caller passes this value to
/// [`FirecrackerDriver::with_network`](super::firecracker::driver::FirecrackerDriver::with_network).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuestNetworkConfig {
    /// The guest interface name. Default [`DEFAULT_INTERFACE`].
    pub interface: String,
    /// The TAP interface name in the private namespace. Default [`DEFAULT_TAP_NAME`].
    pub tap: String,
    /// The MAC address of the stack on the TAP side. Default [`DEFAULT_HOST_MAC`].
    pub host_mac: [u8; 6],
    /// The MAC address of the guest interface. Default [`DEFAULT_GUEST_MAC`].
    pub guest_mac: [u8; 6],
    /// The stack address on the link. Default [`DEFAULT_HOST_IP`].
    pub host_ip: Ipv4Addr,
    /// The guest address on the link. Default [`DEFAULT_GUEST_IP`].
    pub guest_ip: Ipv4Addr,
    /// The link prefix length. Default [`DEFAULT_PREFIX_LEN`].
    pub prefix_len: u8,
    /// The link MTU in bytes, without the Ethernet header. Default [`DEFAULT_TAP_MTU`].
    pub mtu: usize,
    /// The most frames that one service call moves in each direction. Default 64.
    pub max_frames_per_service: usize,
    /// The DNS servers. The first one becomes a UDP forward of guest port 53 to its
    /// port 53; the guest resolver names [`Self::host_ip`]. Further servers are not
    /// forwarded, because one guest port has one forward. Default empty.
    pub dns: Vec<Ipv4Addr>,
    /// The guest TCP ports that the host answers. Default empty.
    pub tcp_forwards: Vec<TcpForward>,
    /// The guest UDP ports that the host answers. Default empty.
    pub udp_forwards: Vec<UdpForward>,
    /// The longest wait between two services of the link, and between two reads of a
    /// relay. It is also the wait of the helper between two frame checks. Default
    /// 10 ms.
    pub poll_interval: Duration,
    /// The connect deadline to a host service. Default 2 s.
    pub connect_timeout: Duration,
    /// The most relay threads that one sandbox holds at the same time. Default 64.
    pub max_connections: usize,
    /// The bytes that one relay reads at a time. Default 8 KiB, one pipe page.
    pub relay_buffer_bytes: usize,
}

impl Default for GuestNetworkConfig {
    fn default() -> Self {
        Self {
            interface: DEFAULT_INTERFACE.to_string(),
            tap: DEFAULT_TAP_NAME.to_string(),
            host_mac: DEFAULT_HOST_MAC,
            guest_mac: DEFAULT_GUEST_MAC,
            host_ip: DEFAULT_HOST_IP,
            guest_ip: DEFAULT_GUEST_IP,
            prefix_len: DEFAULT_PREFIX_LEN,
            mtu: DEFAULT_TAP_MTU,
            max_frames_per_service: 64,
            dns: Vec::new(),
            tcp_forwards: Vec::new(),
            udp_forwards: Vec::new(),
            poll_interval: Duration::from_millis(10),
            connect_timeout: Duration::from_secs(2),
            max_connections: 64,
            relay_buffer_bytes: 8 * 1024,
        }
    }
}

impl GuestNetworkConfig {
    /// Override the guest interface name.
    #[must_use]
    pub fn with_interface(mut self, interface: impl Into<String>) -> Self {
        self.interface = interface.into();
        self
    }

    /// Override the TAP interface name.
    #[must_use]
    pub fn with_tap(mut self, tap: impl Into<String>) -> Self {
        self.tap = tap.into();
        self
    }

    /// Override the stack and guest MAC addresses.
    #[must_use]
    pub fn with_macs(mut self, host_mac: [u8; 6], guest_mac: [u8; 6]) -> Self {
        self.host_mac = host_mac;
        self.guest_mac = guest_mac;
        self
    }

    /// Override the stack address, the guest address and the prefix length.
    #[must_use]
    pub fn with_addresses(mut self, host_ip: Ipv4Addr, guest_ip: Ipv4Addr, prefix_len: u8) -> Self {
        self.host_ip = host_ip;
        self.guest_ip = guest_ip;
        self.prefix_len = prefix_len;
        self
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

    /// Add one DNS server.
    #[must_use]
    pub fn with_dns(mut self, server: Ipv4Addr) -> Self {
        self.dns.push(server);
        self
    }

    /// Add one guest TCP port that the host answers.
    #[must_use]
    pub fn with_tcp_forward(mut self, guest_port: u16, host: SocketAddr) -> Self {
        self.tcp_forwards.push(TcpForward { guest_port, host });
        self
    }

    /// Add one guest UDP port that the host answers.
    #[must_use]
    pub fn with_udp_forward(mut self, guest_port: u16, host: SocketAddr) -> Self {
        self.udp_forwards.push(UdpForward { guest_port, host });
        self
    }

    /// Override the longest wait between two services.
    #[must_use]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval.max(Duration::from_millis(1));
        self
    }

    /// Override the connect deadline to a host service.
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Override the most relay threads of one sandbox.
    #[must_use]
    pub fn with_max_connections(mut self, connections: usize) -> Self {
        self.max_connections = connections.max(1);
        self
    }

    /// Override the bytes that one relay reads at a time.
    #[must_use]
    pub fn with_relay_buffer_bytes(mut self, bytes: usize) -> Self {
        self.relay_buffer_bytes = bytes.max(1024);
        self
    }

    /// The guest MAC address in the form that the Firecracker API takes.
    #[must_use]
    pub fn guest_mac_address(&self) -> String {
        format!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.guest_mac[0],
            self.guest_mac[1],
            self.guest_mac[2],
            self.guest_mac[3],
            self.guest_mac[4],
            self.guest_mac[5]
        )
    }

    /// The kernel arguments that configure the guest interface.
    ///
    /// `net.ifnames=0` keeps the kernel names simple (`eth0`). `ewe.iface` names the
    /// interface and `ewe.net` names the guest address, the prefix and the gateway; the
    /// slave applies both at boot. The guest resolver must name [`Self::host_ip`]; the
    /// configured DNS server is a UDP forward of guest port 53 on it.
    #[must_use]
    pub fn boot_args(&self) -> String {
        format!(
            "net.ifnames=0 ewe.iface={} ewe.net={}/{},{}",
            self.interface, self.guest_ip, self.prefix_len, self.host_ip
        )
    }

    /// The arguments of the helper, without the Firecracker program and its arguments.
    #[must_use]
    pub(crate) fn helper_args(&self) -> Vec<String> {
        vec![
            "--tap".to_string(),
            self.tap.clone(),
            "--mtu".to_string(),
            self.mtu.to_string(),
            "--poll".to_string(),
            self.poll_interval.as_millis().to_string(),
            "--".to_string(),
        ]
    }
}

// ---------------------------------------------------------------------------
// The control-plane side
// ---------------------------------------------------------------------------

/// One running guest link: the stack, the relays and their thread.
pub struct NetworkRuntime {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl std::fmt::Debug for NetworkRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NetworkRuntime")
            .field("stopped", &self.stop.load(Ordering::SeqCst))
            .finish_non_exhaustive()
    }
}

impl NetworkRuntime {
    /// Put the Ethernet stack on the frame channel and start the service thread.
    ///
    /// `frames` is one end of the Unix datagram pair. The helper holds the other end
    /// and moves TAP frames across it.
    ///
    /// # Errors
    /// [`SandboxError::Io`] when a listener cannot bind on the stack address or the
    /// channel cannot become non-blocking.
    pub fn start(
        config: &GuestNetworkConfig,
        frames: UnixDatagram,
        ctx: &Ctx,
    ) -> SandboxResult<Self> {
        frames
            .set_nonblocking(true)
            .map_err(|error| SandboxError::Io {
                op: "set the frame channel non-blocking".to_string(),
                message: error.to_string(),
            })?;
        let stack_config = NetStackConfig::new(IpAddr::V4(config.host_ip), config.prefix_len)
            .with_ethernet(config.host_mac)
            .with_mtu(config.mtu);
        let plane = TapDataPlane::new(
            SocketFrames {
                socket: frames,
                // The stack sends frames of its own MTU; the receive buffer holds the
                // largest frame that the guest can send.
                mtu: MAX_FRAME_BYTES.saturating_sub(ETHERNET_HEADER_LEN),
            },
            stack_config,
            config.max_frames_per_service,
            ctx,
        );
        let mut stack = plane.stack().clone();

        let mut listeners: Vec<(OverlayListener, SocketAddr)> = Vec::new();
        for forward in &config.tcp_forwards {
            let address = SocketAddr::new(IpAddr::V4(config.host_ip), forward.guest_port);
            let listener = stack
                .tcp_listen(address)
                .map_err(|error| SandboxError::Io {
                    op: format!("listen on the guest address {address}"),
                    message: error.to_string(),
                })?;
            listeners.push((listener, forward.host));
        }

        // One guest port has one forward. An explicit forward wins over the DNS
        // default, and the first DNS server is the resolver.
        let mut udp_targets: std::collections::BTreeMap<u16, SocketAddr> =
            std::collections::BTreeMap::new();
        for forward in &config.udp_forwards {
            udp_targets
                .entry(forward.guest_port)
                .or_insert(forward.host);
        }
        if let Some(server) = config.dns.first() {
            udp_targets
                .entry(53)
                .or_insert_with(|| SocketAddr::new(IpAddr::V4(*server), 53));
        }
        let mut datagrams: Vec<UdpRelay> = Vec::new();
        for (guest_port, host) in udp_targets {
            datagrams.push(UdpRelay::bind(&mut stack, config, guest_port, host)?);
        }

        let stop = Arc::new(AtomicBool::new(false));
        let live = Arc::new(AtomicUsize::new(0));
        let thread_stop = Arc::clone(&stop);
        let thread_ctx = ctx.clone();
        let thread_config = config.clone();
        let thread = std::thread::Builder::new()
            .name("ewe-fc-network".to_string())
            .spawn(move || {
                service_loop(
                    plane,
                    listeners,
                    datagrams,
                    thread_config,
                    thread_ctx,
                    thread_stop,
                    live,
                );
            })
            .map_err(|error| SandboxError::Io {
                op: "start the guest network thread".to_string(),
                message: error.to_string(),
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for NetworkRuntime {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            // The thread wakes from `poll` within one poll interval and ends.
            let _ = thread.join();
        }
    }
}

/// The control-plane end of the frame channel.
struct SocketFrames {
    socket: UnixDatagram,
    mtu: usize,
}

impl FrameIo for SocketFrames {
    fn read_frame(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.socket.recv(buf) {
            // A datagram socket reports the closed peer as a zero-length read.
            Ok(0) => Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "the guest network helper closed the frame channel",
            )),
            other => other,
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> std::io::Result<usize> {
        self.socket.send(frame)
    }

    fn mtu(&self) -> usize {
        self.mtu
    }
}

/// One UDP forward: a stack socket and the host socket that answers it.
struct UdpRelay {
    overlay: OverlayUdp,
    host: UdpSocket,
    /// The host service that receives the guest datagrams.
    target: SocketAddr,
    /// The most recent guest source of this forward, so a reply can go back.
    guest: Option<SocketAddr>,
}

impl UdpRelay {
    /// Bind one stack UDP port and one host UDP socket.
    fn bind(
        stack: &mut foundation_nativeapis::dataplane::NetStack,
        config: &GuestNetworkConfig,
        guest_port: u16,
        host: SocketAddr,
    ) -> SandboxResult<Self> {
        let address = SocketAddr::new(IpAddr::V4(config.host_ip), guest_port);
        let overlay = stack.udp_bind(address).map_err(|error| SandboxError::Io {
            op: format!("bind the guest UDP address {address}"),
            message: error.to_string(),
        })?;
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|error| SandboxError::Io {
            op: "bind the host UDP relay".to_string(),
            message: error.to_string(),
        })?;
        socket
            .set_nonblocking(true)
            .map_err(|error| SandboxError::Io {
                op: "set the host UDP relay non-blocking".to_string(),
                message: error.to_string(),
            })?;
        Ok(Self {
            overlay,
            host: socket,
            target: host,
            guest: None,
        })
    }

    /// Copy every ready datagram in each direction.
    fn relay(&mut self, buffer: &mut [u8]) {
        while let Ok((len, from)) = self.overlay.recv_from(buffer) {
            self.guest = Some(from);
            let _ = self.host.send_to(&buffer[..len], self.target);
        }
        if let Some(guest) = self.guest {
            while let Ok((len, _from)) = self.host.recv_from(buffer) {
                let _ = self.overlay.send_to(&buffer[..len], guest);
            }
        }
    }
}

/// Service the frame channel and every relay until `stop` is set.
fn service_loop(
    mut plane: TapDataPlane<SocketFrames>,
    mut listeners: Vec<(OverlayListener, SocketAddr)>,
    mut datagrams: Vec<UdpRelay>,
    config: GuestNetworkConfig,
    ctx: Ctx,
    stop: Arc<AtomicBool>,
    live: Arc<AtomicUsize>,
) {
    let channel_fd = plane.io().socket.as_raw_fd();
    let mut buffer = vec![0u8; config.relay_buffer_bytes];
    while !stop.load(Ordering::SeqCst) {
        // Answer the guest on every configured port.
        for (listener, host) in &mut listeners {
            loop {
                let Ok(stream) = listener.accept() else {
                    break;
                };
                if live.load(Ordering::SeqCst) >= config.max_connections {
                    drop(stream);
                    continue;
                }
                spawn_relay(stream, *host, &config, Arc::clone(&stop), Arc::clone(&live));
            }
        }
        for relay in &mut datagrams {
            relay.relay(&mut buffer);
        }

        let now = ctx.clock().now();
        if let Err(error) = plane.service(now) {
            tracing::debug!(error = %error, "the guest network stopped");
            break;
        }

        let poll_ms = i32::try_from(config.poll_interval.as_millis()).unwrap_or(i32::MAX);
        let mut descriptor = libc::pollfd {
            fd: channel_fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid descriptor and a bounded timeout.
        let _ = unsafe { libc::poll(std::ptr::addr_of_mut!(descriptor), 1, poll_ms) };
    }
}

/// Start the relay threads of one guest connection.
fn spawn_relay(
    stream: OverlayStream,
    host: SocketAddr,
    config: &GuestNetworkConfig,
    stop: Arc<AtomicBool>,
    live: Arc<AtomicUsize>,
) {
    let config = config.clone();
    live.fetch_add(1, Ordering::SeqCst);
    let guard = LiveGuard {
        live: Arc::clone(&live),
    };
    let _ = std::thread::Builder::new()
        .name("ewe-fc-relay".to_string())
        .spawn(move || {
            let _guard = guard;
            let Ok(tcp) = TcpStream::connect_timeout(&host, config.connect_timeout) else {
                return;
            };
            let _ = tcp.set_nodelay(true);
            let Ok(peer) = tcp.try_clone() else {
                return;
            };
            let upload = std::thread::Builder::new()
                .name("ewe-fc-relay-up".to_string())
                .spawn({
                    let stream = stream.clone();
                    let config = config.clone();
                    let stop = Arc::clone(&stop);
                    move || relay_guest_to_host(stream, tcp, &config, &stop)
                });
            relay_host_to_guest(peer, stream, &config, &stop);
            if let Ok(upload) = upload {
                let _ = upload.join();
            }
        });
}

/// Count one relay and release its slot when the relay ends.
struct LiveGuard {
    live: Arc<AtomicUsize>,
}

impl Drop for LiveGuard {
    fn drop(&mut self) {
        self.live.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Copy the guest bytes to the host service.
fn relay_guest_to_host(
    stream: OverlayStream,
    mut host: TcpStream,
    config: &GuestNetworkConfig,
    stop: &AtomicBool,
) {
    let mut buffer = vec![0u8; config.relay_buffer_bytes];
    while !stop.load(Ordering::SeqCst) {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(len) => {
                if host.write_all(&buffer[..len]).is_err() {
                    break;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                let flag = Arc::new(Flag::default());
                stream.set_read_waker(flag.waker());
                let _ = flag.wait(config.poll_interval);
            }
            Err(_) => break,
        }
    }
    let _ = host.shutdown(std::net::Shutdown::Write);
}

/// Copy the host bytes to the guest.
fn relay_host_to_guest(
    mut host: TcpStream,
    stream: OverlayStream,
    config: &GuestNetworkConfig,
    stop: &AtomicBool,
) {
    let _ = host.set_read_timeout(Some(config.poll_interval));
    let mut buffer = vec![0u8; config.relay_buffer_bytes];
    while !stop.load(Ordering::SeqCst) {
        match host.read(&mut buffer) {
            Ok(0) => {
                stream.close();
                break;
            }
            Ok(len) => {
                let mut offset = 0;
                while offset < len && !stop.load(Ordering::SeqCst) {
                    match stream.write(&buffer[offset..len]) {
                        Ok(0) => return,
                        Ok(written) => offset += written,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            let flag = Arc::new(Flag::default());
                            stream.set_write_waker(flag.waker());
                            let _ = flag.wait(config.poll_interval);
                        }
                        Err(_) => return,
                    }
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }
}

/// A one-shot wake flag for a relay thread.
#[derive(Default)]
struct Flag {
    value: Mutex<bool>,
    waiter: Condvar,
}

impl Flag {
    /// A wake callback that sets the flag.
    fn waker(self: &Arc<Self>) -> WakeFn {
        let flag = Arc::clone(self);
        Arc::new(move || {
            *flag.value.lock().unwrap_or_else(PoisonError::into_inner) = true;
            flag.waiter.notify_all();
        })
    }

    /// Wait until the flag is set or `timeout` passes; true when the flag was set.
    fn wait(&self, timeout: Duration) -> bool {
        let value = self.value.lock().unwrap_or_else(PoisonError::into_inner);
        if *value {
            return true;
        }
        let (value, _) = self
            .waiter
            .wait_timeout(value, timeout)
            .unwrap_or_else(PoisonError::into_inner);
        *value
    }
}

// ---------------------------------------------------------------------------
// The helper program
// ---------------------------------------------------------------------------

/// Run the helper program and return its exit code.
///
/// This function is the whole body of the `agent-fc-net` program. It is not async:
/// the program has one job and one thread.
#[must_use]
pub fn run_helper() -> i32 {
    match helper_main() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("agent-fc-net: {error}");
            70
        }
    }
}

/// The arguments of the helper.
struct HelperArgs {
    tap: String,
    mtu: u32,
    /// The longest wait between two frame checks, in milliseconds.
    poll_ms: i32,
    program: String,
    program_args: Vec<String>,
}

impl HelperArgs {
    /// Parse `--tap <name> --mtu <n> --poll <ms> -- <program> <args...>`.
    fn parse(mut args: Vec<String>) -> std::io::Result<Self> {
        let mut tap = None;
        let mut mtu = None;
        let mut poll_ms = None;
        loop {
            let Some(argument) = args.first().cloned() else {
                return Err(invalid("the helper needs --tap, --mtu and --poll"));
            };
            args.remove(0);
            match argument.as_str() {
                "--tap" => {
                    let value = args
                        .first()
                        .cloned()
                        .ok_or_else(|| invalid("--tap needs a name"))?;
                    args.remove(0);
                    tap = Some(value);
                }
                "--mtu" => {
                    let value = args
                        .first()
                        .cloned()
                        .ok_or_else(|| invalid("--mtu needs a number"))?;
                    args.remove(0);
                    mtu = Some(
                        value
                            .parse::<u32>()
                            .map_err(|_| invalid("--mtu is not a number"))?,
                    );
                }
                "--poll" => {
                    let value = args
                        .first()
                        .cloned()
                        .ok_or_else(|| invalid("--poll needs a number"))?;
                    args.remove(0);
                    let millis = value
                        .parse::<u64>()
                        .map_err(|_| invalid("--poll is not a number"))?;
                    poll_ms =
                        Some(i32::try_from(millis).map_err(|_| invalid("--poll is too large"))?);
                }
                "--" => break,
                other => {
                    return Err(invalid_owned(format!("unknown helper argument {other}")));
                }
            }
        }
        let program = args
            .first()
            .cloned()
            .ok_or_else(|| invalid("the helper needs a program after --"))?;
        args.remove(0);
        Ok(Self {
            tap: tap.ok_or_else(|| invalid("the helper needs --tap"))?,
            mtu: mtu.ok_or_else(|| invalid("the helper needs --mtu"))?,
            poll_ms: poll_ms.ok_or_else(|| invalid("the helper needs --poll"))?,
            program,
            program_args: args,
        })
    }
}

/// The body of the helper.
fn helper_main() -> std::io::Result<i32> {
    let parsed = HelperArgs::parse(std::env::args().skip(1).collect())?;
    make_namespace()?;
    create_tap(&parsed.tap, parsed.mtu)?;
    let (packet, interface) = open_packet_socket(&parsed.tap)?;
    // SAFETY: descriptor 3 is the frame channel that the control plane passed.
    let frames = unsafe { UnixDatagram::from_raw_fd(FRAME_DESCRIPTOR) };
    frames.set_nonblocking(true)?;
    // The program after `--` must not inherit the channel.
    // SAFETY: the descriptor is open and owned by this process.
    if unsafe { libc::fcntl(frames.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut child = spawn_program(&parsed.program, &parsed.program_args)?;
    pump_frames(packet, interface, &frames, &mut child, parsed.poll_ms)
}

/// Make a private user and network namespace, and map this user into it.
fn make_namespace() -> std::io::Result<()> {
    // A user namespace gives this process `CAP_NET_ADMIN` over its new network
    // namespace, so no host privilege is needed. The helper has one thread here.
    // SAFETY: both flags are valid for `unshare`.
    if unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNET) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `geteuid` reads the process credential only.
    let uid = unsafe { libc::geteuid() };
    // SAFETY: `getegid` reads the process credential only.
    let gid = unsafe { libc::getegid() };
    write_control(c"/proc/self/setgroups", b"deny")?;
    write_control(c"/proc/self/uid_map", format!("0 {uid} 1\n").as_bytes())?;
    write_control(c"/proc/self/gid_map", format!("0 {gid} 1\n").as_bytes())
}

/// Create the TAP device `name`, set its MTU and set it up.
///
/// The device is persistent, because this process closes its descriptor before
/// Firecracker opens the device. A TAP device has one holder, so the guest frames
/// reach this helper through an `AF_PACKET` socket, not a second open.
fn create_tap(name: &str, mtu: u32) -> std::io::Result<()> {
    /// The `ifreq` flags of a TAP device: Layer 2 frames and no 4-byte prefix.
    const IFF_TAP: libc::c_short = 0x0002;
    /// Do not prepend the packet information header.
    const IFF_NO_PI: libc::c_short = 0x1000;
    /// The `ioctl` that creates or attaches a TAP device.
    const TUNSETIFF: IoctlRequest = 0x4004_54ca;
    /// The `ioctl` that keeps a TAP device after its last descriptor closes.
    const TUNSETPERSIST: IoctlRequest = 0x4004_54cb;

    let interface = kernel_name(name)?;
    // SAFETY: the path and flags are valid for `open`.
    let fd = unsafe {
        libc::open(
            c"/dev/net/tun".as_ptr(),
            libc::O_RDWR | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = interface;
        ifr.ifr_ifru.ifru_flags = IFF_TAP | IFF_NO_PI;
        // SAFETY: `ifr` has the kernel layout of `struct ifreq`.
        if unsafe { libc::ioctl(fd, TUNSETIFF, std::ptr::addr_of_mut!(ifr)) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        set_interface_up_and_mtu(interface, mtu)?;
        let persist: libc::c_int = 1;
        // SAFETY: the descriptor is a TAP device and the argument is an integer.
        if unsafe { libc::ioctl(fd, TUNSETPERSIST, &persist) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })();
    // SAFETY: the descriptor is open and owned by this process.
    unsafe { libc::close(fd) };
    result
}

/// Set `IFF_UP` and the MTU of `name` through a control socket.
///
/// A TAP descriptor handles only the TUN-specific requests, so the three `ifreq`
/// requests go to an `AF_INET` socket.
fn set_interface_up_and_mtu(name: [libc::c_char; libc::IFNAMSIZ], mtu: u32) -> std::io::Result<()> {
    /// The `ioctl` that reads the interface flags.
    const SIOCGIFFLAGS: IoctlRequest = 0x8913;
    /// The `ioctl` that sets the interface flags.
    const SIOCSIFFLAGS: IoctlRequest = 0x8914;
    /// The `ioctl` that sets the interface MTU.
    const SIOCSIFMTU: IoctlRequest = 0x8922;
    /// The flag that makes an interface active.
    const IFF_UP: libc::c_short = 0x1;

    let mtu =
        libc::c_int::try_from(mtu).map_err(|_| invalid("the MTU does not fit in a C integer"))?;
    // SAFETY: the arguments make a datagram socket.
    let control = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    if control < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = name;
        ifr.ifr_ifru.ifru_mtu = mtu;
        // SAFETY: `ifr` has the kernel layout of `struct ifreq`.
        if unsafe { libc::ioctl(control, SIOCSIFMTU, std::ptr::addr_of_mut!(ifr)) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut ifr: libc::ifreq = unsafe { std::mem::zeroed() };
        ifr.ifr_name = name;
        // SAFETY: the same valid buffer.
        if unsafe { libc::ioctl(control, SIOCGIFFLAGS, std::ptr::addr_of_mut!(ifr)) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `SIOCGIFFLAGS` filled the flags member of the union.
        unsafe { ifr.ifr_ifru.ifru_flags |= IFF_UP };
        // SAFETY: the same valid buffer.
        if unsafe { libc::ioctl(control, SIOCSIFFLAGS, std::ptr::addr_of_mut!(ifr)) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    })();
    // SAFETY: the descriptor is open and owned by this process.
    unsafe { libc::close(control) };
    result
}

/// Open an `AF_PACKET` socket bound to the TAP interface `name`.
///
/// This socket sees every frame that the kernel receives on the TAP device, which is
/// every frame the guest sends. A frame written to it reaches the TAP holder, which is
/// Firecracker. It does not hold the TAP device, so Firecracker can open the device.
fn open_packet_socket(name: &str) -> std::io::Result<(libc::c_int, i32)> {
    /// The Ethernet protocol number that matches every frame.
    const ETH_P_ALL: u16 = 0x0003;
    /// Suppress the loopback copy of the frames this socket sends.
    const PACKET_IGNORE_OUTGOING: libc::c_int = 23;

    let interface =
        std::ffi::CString::new(name).map_err(|_| invalid("the TAP name holds a zero byte"))?;
    // SAFETY: the path is a valid C string; the buffer is large enough.
    let index = unsafe { libc::if_nametoindex(interface.as_ptr()) };
    if index == 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the arguments make a packet socket.
    let fd = unsafe {
        libc::socket(
            libc::AF_PACKET,
            libc::SOCK_RAW | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            i32::from(ETH_P_ALL.to_be()),
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let one: libc::c_int = 1;
    let option_len = libc::socklen_t::try_from(std::mem::size_of::<libc::c_int>())
        .map_err(|_| invalid("a socket option length does not fit"))?;
    // SAFETY: the option takes one integer of four bytes.
    let ignored = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_PACKET,
            PACKET_IGNORE_OUTGOING,
            std::ptr::addr_of!(one).cast(),
            option_len,
        )
    };
    if ignored < 0 {
        let error = std::io::Error::last_os_error();
        // A kernel older than 4.20 has no such option. The packet-type check in
        // `pump_frames` skips the same frames.
        if error.raw_os_error() != Some(libc::ENOPROTOOPT) {
            // SAFETY: the descriptor is open and owned by this process.
            unsafe { libc::close(fd) };
            return Err(error);
        }
    }
    let mut address: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    address.sll_family = libc::c_ushort::try_from(libc::AF_PACKET)
        .map_err(|_| invalid("the packet family does not fit"))?;
    address.sll_protocol = ETH_P_ALL.to_be();
    address.sll_ifindex =
        i32::try_from(index).map_err(|_| invalid("the interface index does not fit"))?;
    let address_len = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_ll>())
        .map_err(|_| invalid("the address length does not fit"))?;
    // SAFETY: `address` has the kernel layout of `sockaddr_ll`.
    let bound = unsafe {
        libc::bind(
            fd,
            std::ptr::addr_of!(address).cast::<libc::sockaddr>(),
            address_len,
        )
    };
    if bound < 0 {
        let error = std::io::Error::last_os_error();
        // SAFETY: the descriptor is open and owned by this process.
        unsafe { libc::close(fd) };
        return Err(error);
    }
    Ok((fd, address.sll_ifindex))
}

/// Start the program after `--`, with a parent-death signal.
fn spawn_program(program: &str, arguments: &[String]) -> std::io::Result<std::process::Child> {
    let mut command = std::process::Command::new(program);
    command.args(arguments).env_clear();
    // SAFETY: `pre_exec` runs in the forked child before `exec`. It calls only
    // `prctl`, `getppid` and `_exit`.
    unsafe {
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() == 1 {
                libc::_exit(1);
            }
            Ok(())
        });
    }
    command.spawn()
}

/// Copy frames between the packet socket and the control plane until the program ends.
fn pump_frames(
    packet: libc::c_int,
    interface: i32,
    frames: &UnixDatagram,
    child: &mut std::process::Child,
    poll_ms: i32,
) -> std::io::Result<i32> {
    let mut buffer = vec![0u8; MAX_FRAME_BYTES];
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(exit_code(status));
        }
        let mut descriptors = [
            libc::pollfd {
                fd: packet,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: frames.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // SAFETY: two valid descriptors and a bounded timeout.
        let ready = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                libc::nfds_t::try_from(descriptors.len())
                    .map_err(|_| invalid("the descriptor count does not fit"))?,
                poll_ms,
            )
        };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            let mut source: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
            let mut length = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_ll>())
                .map_err(|_| invalid("the address length does not fit"))?;
            // SAFETY: the buffer and the address are valid.
            let len = unsafe {
                libc::recvfrom(
                    packet,
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    0,
                    std::ptr::addr_of_mut!(source).cast::<libc::sockaddr>(),
                    std::ptr::addr_of_mut!(length),
                )
            };
            if len > 0 {
                // The loopback copy of a frame this socket sent is not guest traffic.
                let outgoing = u8::try_from(libc::PACKET_OUTGOING)
                    .map_err(|_| invalid("the packet type does not fit"))?;
                if source.sll_pkttype != outgoing {
                    let len = usize::try_from(len)
                        .map_err(|_| invalid("a frame has a negative length"))?;
                    match frames.send(&buffer[..len]) {
                        Ok(_) => {}
                        // A full channel drops the frame; Ethernet is lossy and the
                        // protocol above it retransmits.
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(_) => {
                            // The control plane is gone: end the VM.
                            let _ = child.kill();
                            let _ = child.wait();
                            return Ok(0);
                        }
                    }
                }
            }
        }
        if descriptors[1].revents & libc::POLLIN != 0 {
            match frames.recv(&mut buffer) {
                Ok(0) => {
                    // The control plane dropped the channel: end the VM.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(0);
                }
                Ok(len) => send_frame(packet, &buffer[..len], interface)?,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
    }
}

/// Send one frame to the TAP holder through the packet socket.
fn send_frame(fd: libc::c_int, frame: &[u8], interface: i32) -> std::io::Result<()> {
    let mut address: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
    address.sll_family = libc::c_ushort::try_from(libc::AF_PACKET)
        .map_err(|_| invalid("the packet family does not fit"))?;
    address.sll_protocol = 0x0003_u16.to_be();
    address.sll_ifindex = interface;
    let address_len = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_ll>())
        .map_err(|_| invalid("the address length does not fit"))?;
    // SAFETY: the frame and the address are valid for the call.
    let sent = unsafe {
        libc::sendto(
            fd,
            frame.as_ptr().cast(),
            frame.len(),
            0,
            std::ptr::addr_of!(address).cast::<libc::sockaddr>(),
            address_len,
        )
    };
    if sent < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// The exit code of a finished child, with `128 + signal` for a signal.
fn exit_code(status: std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt as _;
    status
        .code()
        .or_else(|| status.signal().map(|signal| 128 + signal))
        .unwrap_or(1)
}

/// Copy `name` into the kernel field of `ifreq`, rejecting an over-long name.
fn kernel_name(name: &str) -> std::io::Result<[libc::c_char; libc::IFNAMSIZ]> {
    let bytes = name.as_bytes();
    if bytes.len() >= libc::IFNAMSIZ {
        return Err(invalid("the TAP interface name is too long"));
    }
    let mut out = [0; libc::IFNAMSIZ];
    for (dst, &byte) in out.iter_mut().zip(bytes) {
        *dst = libc::c_char::from_ne_bytes([byte]);
    }
    Ok(out)
}

/// Write `data` to a `/proc/self` control file.
fn write_control(path: &std::ffi::CStr, data: &[u8]) -> std::io::Result<()> {
    // SAFETY: the path is a valid C string and the flags are valid for `open`.
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut written = 0;
    let result = loop {
        if written >= data.len() {
            break Ok(());
        }
        // SAFETY: the slice and the descriptor are valid.
        let count =
            unsafe { libc::write(fd, data[written..].as_ptr().cast(), data.len() - written) };
        if count < 0 {
            break Err(std::io::Error::last_os_error());
        }
        match usize::try_from(count) {
            Ok(0) => break Err(invalid("a control file accepted no bytes")),
            Ok(count) => written += count,
            Err(_) => break Err(invalid("a control write has a negative length")),
        }
    };
    // SAFETY: the descriptor is open and owned by this process.
    unsafe { libc::close(fd) };
    result
}

/// An invalid-argument error.
fn invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

/// An invalid-argument error with generated text.
fn invalid_owned(message: String) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}
