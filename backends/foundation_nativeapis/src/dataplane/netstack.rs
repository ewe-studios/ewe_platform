//! smoltcp-backed userspace TCP/IP netstack (spec-55, feature 00).
//!
//! WHY: The default, portable, unprivileged data plane. Unlike a kernel TUN device it
//! needs no `CAP_NET_ADMIN`, no `/dev/net/tun`, and works on wasm/mobile — because the
//! TCP/IP protocol runs *in our process* on in-memory buffers rather than in the kernel.
//!
//! WHAT: [`NetStack`] wraps a smoltcp [`Interface`] at **medium-ip** (L3, no Ethernet)
//! plus a [`SocketSet`]. Its "wire" is not a NIC but the `Tunn` plaintext side: outbound
//! IP packets are queued for `Tunn::encapsulate`, inbound decapsulated packets are
//! injected back in. It implements [`DataPlane`] and hands out overlay TCP/UDP sockets.
//!
//! HOW: A custom smoltcp [`phy::Device`] ([`TunnDevice`]) whose RX queue is fed by
//! [`DataPlane::inject_inbound_ip`] and whose TX queue is drained by
//! [`DataPlane::drain_outbound_ip`]. All state lives behind an `Arc<Mutex<..>>` so the
//! single driver task and the overlay socket handles share it cheaply while staying
//! `Send` (required to run inside a valtron task); the stack is single-threaded by
//! construction (smoltcp is sans-I/O and brings no runtime), so the mutex is uncontended.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
use smoltcp::phy::{Device, DeviceCapabilities, Medium};
use smoltcp::socket::{tcp, udp};
use smoltcp::storage::PacketMetadata;
use smoltcp::time::Instant as SmolInstant;
use smoltcp::wire::{HardwareAddress, IpAddress, IpCidr, IpEndpoint, IpListenEndpoint};

use super::{DataPlane, WakeFn};

/// Default MTU for the overlay link. WireGuard's own overhead is applied at the outer
/// (UDP) boundary; this is the inner L3 MTU the netstack advertises to its sockets.
const DEFAULT_MTU: usize = 1420;

/// Per-socket send/receive buffer size (bytes) for overlay TCP sockets.
const TCP_BUFFER_BYTES: usize = 64 * 1024;

/// Per-socket receive/transmit buffer size (bytes) for overlay UDP sockets.
const UDP_BUFFER_BYTES: usize = 64 * 1024;

/// Maximum number of buffered datagrams per overlay UDP socket direction.
const UDP_PACKETS: usize = 32;

/// First ephemeral port handed out to outbound overlay connections.
const EPHEMERAL_PORT_START: u16 = 49152;

/// Default per-listener accept backlog: how many connections the stack completes on a
/// listener's behalf before the application calls [`OverlayListener::accept`].
///
/// 511 is nginx's default `listen ... backlog=` on Linux. Like a kernel backlog it costs
/// nothing until connections actually queue: only one listening socket is armed at a
/// time, and each queued connection is a socket that would exist anyway once accepted.
pub const DEFAULT_LISTEN_BACKLOG: usize = 511;

// ---------------------------------------------------------------------------
// phy::Device bridging the netstack to the Tunn plaintext side
// ---------------------------------------------------------------------------

/// WHY: smoltcp drives all I/O through a [`phy::Device`]; our "device" is the `Tunn`
/// plaintext boundary, not a real NIC.
///
/// WHAT: A pair of in-memory FIFO queues — `rx` holds decapsulated inbound IP packets,
/// `tx` holds outbound IP packets awaiting encryption.
///
/// HOW: [`inject_inbound_ip`](DataPlane::inject_inbound_ip) pushes to `rx`;
/// [`drain_outbound_ip`](DataPlane::drain_outbound_ip) pops `tx`.
struct TunnDevice {
    rx: VecDeque<Vec<u8>>,
    tx: VecDeque<Vec<u8>>,
    mtu: usize,
}

impl TunnDevice {
    fn new(mtu: usize) -> Self {
        Self {
            rx: VecDeque::new(),
            tx: VecDeque::new(),
            mtu,
        }
    }
}

/// RX token that owns one inbound packet buffer.
struct RxToken {
    buffer: Vec<u8>,
}

impl smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        f(&self.buffer)
    }
}

/// TX token that pushes the produced packet onto the device's outbound queue.
struct TxToken<'a> {
    tx: &'a mut VecDeque<Vec<u8>>,
}

impl smoltcp::phy::TxToken for TxToken<'_> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut buffer = vec![0u8; len];
        let result = f(&mut buffer);
        self.tx.push_back(buffer);
        result
    }
}

impl Device for TunnDevice {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken<'a>;

    fn receive(&mut self, _timestamp: SmolInstant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let buffer = self.rx.pop_front()?;
        let rx = RxToken { buffer };
        let tx = TxToken { tx: &mut self.tx };
        Some((rx, tx))
    }

    fn transmit(&mut self, _timestamp: SmolInstant) -> Option<Self::TxToken<'_>> {
        Some(TxToken { tx: &mut self.tx })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ip;
        caps.max_transmission_unit = self.mtu;
        caps
    }
}

// ---------------------------------------------------------------------------
// Shared inner state
// ---------------------------------------------------------------------------

/// All mutable netstack state, shared between the driver and every overlay socket
/// handle via `Arc<Mutex<..>>`. Single-threaded by construction (uncontended mutex).
struct Inner {
    device: TunnDevice,
    iface: Interface,
    sockets: SocketSet<'static>,
    /// Base wall-clock instant that smoltcp's monotonic clock is measured from.
    base: Instant,
    /// Next ephemeral local port for outbound connections.
    next_port: u16,
    /// One-shot wakers fired when a socket becomes readable.
    read_wakers: BTreeMap<SocketHandle, WakeFn>,
    /// One-shot wakers fired when a socket becomes writable.
    write_wakers: BTreeMap<SocketHandle, WakeFn>,
    /// Accept queues of every live [`OverlayListener`], keyed by listener id.
    listeners: BTreeMap<u64, ListenQueue>,
    /// Next id handed to a new [`OverlayListener`].
    next_listener_id: u64,
    /// Maximum queued (stack-completed, not yet accepted) connections per listener.
    listen_backlog: usize,
}

/// One listener's accept queue.
///
/// WHY: a smoltcp TCP socket in `Listen` state takes exactly one connection; once it
/// leaves `Listen` there is no socket on the port, so the next SYN is answered with a
/// RST. A kernel listener instead keeps accepting into a backlog until the application
/// calls `accept`. Without that, a client opening two connections back to back (or two
/// clients connecting at once) gets the second one refused whenever the server's accept
/// loop has not run in between.
///
/// WHAT: the currently armed listening socket plus the connections the stack has taken
/// on the listener's behalf, in arrival order.
///
/// HOW: [`Inner::rearm_listeners`] runs after every inbound packet the interface
/// processes: an armed socket that left `Listen` moves to `pending`, and a fresh socket
/// is armed while `pending` is below the backlog.
struct ListenQueue {
    local: SocketAddr,
    armed: Option<SocketHandle>,
    pending: VecDeque<SocketHandle>,
    accept_waker: Option<WakeFn>,
}

impl Inner {
    fn smol_now(&self, now: Instant) -> SmolInstant {
        let elapsed = now.saturating_duration_since(self.base);
        SmolInstant::from_micros(elapsed.as_micros() as i64)
    }

    fn from_smol(&self, instant: SmolInstant) -> Instant {
        let micros = instant.total_micros().max(0) as u64;
        self.base + Duration::from_micros(micros)
    }

    /// Create and arm a fresh listening TCP socket on `addr`, returning its handle.
    fn arm_listening_socket(&mut self, addr: SocketAddr) -> io::Result<SocketHandle> {
        let socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0u8; TCP_BUFFER_BYTES]),
            tcp::SocketBuffer::new(vec![0u8; TCP_BUFFER_BYTES]),
        );
        let handle = self.sockets.add(socket);
        if let Err(e) = self
            .sockets
            .get_mut::<tcp::Socket>(handle)
            .listen(to_listen_endpoint(addr))
        {
            self.sockets.remove(handle);
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("tcp listen: {e}"),
            ));
        }
        Ok(handle)
    }

    /// Move every armed listening socket that has taken a connection into its
    /// listener's accept queue, and arm a replacement while the queue has room.
    fn rearm_listeners(&mut self) {
        let backlog = self.listen_backlog;
        let ids: Vec<u64> = self.listeners.keys().copied().collect();
        for id in ids {
            let Some(queue) = self.listeners.get(&id) else {
                continue;
            };
            let local = queue.local;
            let mut armed = queue.armed;

            if let Some(handle) = armed {
                match self.sockets.get::<tcp::Socket>(handle).state() {
                    tcp::State::Listen => continue,
                    // Reset before it was ever queued: discard it and re-arm below.
                    tcp::State::Closed => {
                        self.sockets.remove(handle);
                    }
                    _ => {
                        if let Some(queue) = self.listeners.get_mut(&id) {
                            queue.pending.push_back(handle);
                        }
                    }
                }
                armed = None;
            }

            let pending_len = self
                .listeners
                .get(&id)
                .map_or(0, |queue| queue.pending.len());
            if armed.is_none() && pending_len < backlog {
                match self.arm_listening_socket(local) {
                    Ok(handle) => armed = Some(handle),
                    Err(e) => {
                        tracing::warn!(%local, error = %e, "failed to re-arm overlay listener");
                    }
                }
            }
            if let Some(queue) = self.listeners.get_mut(&id) {
                queue.armed = armed;
            }
        }
    }

    fn alloc_port(&mut self) -> u16 {
        let port = self.next_port;
        self.next_port = if self.next_port == u16::MAX {
            EPHEMERAL_PORT_START
        } else {
            self.next_port + 1
        };
        port
    }
}

// ---------------------------------------------------------------------------
// NetStack
// ---------------------------------------------------------------------------

/// Configuration for a [`NetStack`]: the overlay tunnel address, link MTU and
/// listener backlog.
#[derive(Debug, Clone)]
pub struct NetStackConfig {
    /// This node's overlay (tunnel) IP address.
    pub address: IpAddr,
    /// Prefix length of the overlay subnet (e.g. `24` for a `/24`).
    pub prefix_len: u8,
    /// Inner L3 MTU advertised to overlay sockets.
    pub mtu: usize,
    /// Maximum connections queued per [`OverlayListener`] before the application
    /// accepts them; further SYNs are refused until `accept` frees a slot. Defaults to
    /// [`DEFAULT_LISTEN_BACKLOG`]; `0` is treated as `1`.
    pub listen_backlog: usize,
}

impl NetStackConfig {
    /// WHY: The overlay address is mandatory (no silent default — spec cross-cutting
    /// constraint); the MTU has a sensible default.
    ///
    /// WHAT: Construct a config from an address and prefix length using [`DEFAULT_MTU`].
    ///
    /// HOW: Stores the values verbatim.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(address: IpAddr, prefix_len: u8) -> Self {
        Self {
            address,
            prefix_len,
            mtu: DEFAULT_MTU,
            listen_backlog: DEFAULT_LISTEN_BACKLOG,
        }
    }

    /// Override the inner L3 MTU (default 1420, `WireGuard`'s default interface MTU).
    #[must_use]
    pub fn with_mtu(mut self, mtu: usize) -> Self {
        self.mtu = mtu;
        self
    }

    /// Override the per-listener accept backlog (default [`DEFAULT_LISTEN_BACKLOG`]).
    /// `0` is treated as `1`: a listener always takes at least one connection.
    #[must_use]
    pub fn with_listen_backlog(mut self, backlog: usize) -> Self {
        self.listen_backlog = backlog;
        self
    }
}

/// A smoltcp-backed userspace TCP/IP data plane. Cheaply cloneable — clones share the
/// same underlying stack, so they must all be driven from the same thread.
#[derive(Clone)]
pub struct NetStack {
    inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for NetStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.inner.lock().expect("netstack mutex poisoned");
        f.debug_struct("NetStack")
            .field("mtu", &inner.device.mtu)
            .field("sockets", &inner.sockets.iter().count())
            .finish()
    }
}

impl NetStack {
    /// WHY: Callers stand up one overlay stack per WireGuard interface.
    ///
    /// WHAT: Build a [`NetStack`] with the given overlay address and MTU.
    ///
    /// HOW: Creates a `medium-ip` smoltcp [`Interface`], assigns the address, and adds an
    /// on-link route so peers in the same overlay subnet are directly reachable.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(config: NetStackConfig) -> Self {
        let mut device = TunnDevice::new(config.mtu);
        let base = Instant::now();
        let iface_config = Config::new(HardwareAddress::Ip);
        let mut iface = Interface::new(iface_config, &mut device, SmolInstant::ZERO);

        let cidr = IpCidr::new(to_smol_address(config.address), config.prefix_len);
        iface.update_ip_addrs(|addrs| {
            // `push` cannot fail for the first address; ignore the (impossible) full case.
            let _ = addrs.push(cidr);
        });

        let inner = Inner {
            device,
            iface,
            sockets: SocketSet::new(Vec::new()),
            base,
            next_port: EPHEMERAL_PORT_START,
            read_wakers: BTreeMap::new(),
            write_wakers: BTreeMap::new(),
            listeners: BTreeMap::new(),
            next_listener_id: 0,
            listen_backlog: config.listen_backlog.max(1),
        };

        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Number of active overlay sockets. Primarily for diagnostics and tests.
    #[must_use]
    pub fn socket_count(&self) -> usize {
        self.inner.lock().expect("netstack mutex poisoned").sockets.iter().count()
    }

    /// Number of outbound IP packets currently queued for encryption.
    #[must_use]
    pub fn pending_outbound(&self) -> usize {
        self.inner.lock().expect("netstack mutex poisoned").device.tx.len()
    }
}

impl DataPlane for NetStack {
    fn inject_inbound_ip(&mut self, packet: &[u8]) {
        if packet.is_empty() {
            return;
        }
        self.inner.lock().expect("netstack mutex poisoned").device.rx.push_back(packet.to_vec());
    }

    fn poll(&mut self, now: Instant) -> Option<Instant> {
        let mut guard = self.inner.lock().expect("netstack mutex poisoned");
        let inner = &mut *guard;
        let timestamp = inner.smol_now(now);

        // Run the interface until it makes no further progress this tick. Ingress is
        // processed one packet at a time so a listener re-arms between two SYNs that
        // arrive in the same batch (see `ListenQueue`).
        loop {
            let mut changed = false;
            loop {
                match inner.iface.poll_ingress_single(
                    timestamp,
                    &mut inner.device,
                    &mut inner.sockets,
                ) {
                    smoltcp::iface::PollIngressSingleResult::None => break,
                    smoltcp::iface::PollIngressSingleResult::PacketProcessed => {}
                    smoltcp::iface::PollIngressSingleResult::SocketStateChanged => {
                        changed = true;
                    }
                }
                inner.rearm_listeners();
            }
            if matches!(
                inner
                    .iface
                    .poll_egress(timestamp, &mut inner.device, &mut inner.sockets),
                smoltcp::iface::PollResult::SocketStateChanged
            ) {
                changed = true;
            }
            inner.rearm_listeners();
            if !changed {
                break;
            }
        }

        // Fire one-shot wakers for sockets that became ready.
        fire_ready_wakers(inner);

        let next = inner.iface.poll_at(timestamp, &inner.sockets);
        next.map(|deadline| inner.from_smol(deadline))
    }

    fn drain_outbound_ip(&mut self, f: &mut dyn FnMut(&[u8])) {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        while let Some(packet) = inner.device.tx.pop_front() {
            f(&packet);
        }
    }

    fn tcp_listen(&mut self, addr: SocketAddr) -> io::Result<OverlayListener> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let armed = inner.arm_listening_socket(addr)?;
        let id = inner.next_listener_id;
        inner.next_listener_id += 1;
        inner.listeners.insert(
            id,
            ListenQueue {
                local: addr,
                armed: Some(armed),
                pending: VecDeque::new(),
                accept_waker: None,
            },
        );
        Ok(OverlayListener {
            inner: Arc::clone(&self.inner),
            id,
            local: addr,
        })
    }

    fn tcp_connect(&mut self, addr: SocketAddr) -> io::Result<OverlayStream> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let local_port = inner.alloc_port();
        let remote = to_smol_endpoint(addr);

        let socket = tcp::Socket::new(
            tcp::SocketBuffer::new(vec![0u8; TCP_BUFFER_BYTES]),
            tcp::SocketBuffer::new(vec![0u8; TCP_BUFFER_BYTES]),
        );

        let Inner {
            iface, sockets, ..
        } = &mut *inner;
        let handle = sockets.add(socket);
        let socket = sockets.get_mut::<tcp::Socket>(handle);
        socket
            .connect(iface.context(), remote, local_port)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("tcp connect: {e}")))?;

        let local = SocketAddr::new(local_addr(iface), local_port);
        Ok(OverlayStream {
            inner: Arc::clone(&self.inner),
            handle,
            local,
            peer: addr,
        })
    }

    fn udp_bind(&mut self, addr: SocketAddr) -> io::Result<OverlayUdp> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let socket = udp::Socket::new(
            udp::PacketBuffer::new(
                vec![PacketMetadata::EMPTY; UDP_PACKETS],
                vec![0u8; UDP_BUFFER_BYTES],
            ),
            udp::PacketBuffer::new(
                vec![PacketMetadata::EMPTY; UDP_PACKETS],
                vec![0u8; UDP_BUFFER_BYTES],
            ),
        );
        let handle = inner.sockets.add(socket);
        let endpoint = to_listen_endpoint(addr);
        inner
            .sockets
            .get_mut::<udp::Socket>(handle)
            .bind(endpoint)
            .map_err(|e| io::Error::new(io::ErrorKind::AddrInUse, format!("udp bind: {e}")))?;
        Ok(OverlayUdp {
            inner: Arc::clone(&self.inner),
            handle,
            local: addr,
        })
    }
}

// ---------------------------------------------------------------------------
// Waker plumbing
// ---------------------------------------------------------------------------

/// Fire one-shot read/write wakers for every socket that is currently ready. Wakers are
/// removed as they fire (Future-style one-shot semantics); the consumer re-registers.
fn fire_ready_wakers(inner: &mut Inner) {
    let mut to_wake_read = Vec::new();
    let mut to_wake_write = Vec::new();

    for (handle, socket) in inner.sockets.iter() {
        let (can_read, can_write) = match socket {
            smoltcp::socket::Socket::Tcp(s) => {
                // Readable when data is buffered or the receive half has closed (EOF).
                let readable = s.can_recv() || !s.may_recv();
                // Writable when the send half accepts data, or fully closed (error surfaced).
                let writable = s.can_send() || !s.is_active();
                (readable, writable)
            }
            smoltcp::socket::Socket::Udp(s) => (s.can_recv(), s.can_send()),
        };
        if can_read && inner.read_wakers.contains_key(&handle) {
            to_wake_read.push(handle);
        }
        if can_write && inner.write_wakers.contains_key(&handle) {
            to_wake_write.push(handle);
        }
    }

    for handle in to_wake_read {
        if let Some(waker) = inner.read_wakers.remove(&handle) {
            waker();
        }
    }
    for queue in inner.listeners.values_mut() {
        if !queue.pending.is_empty() {
            if let Some(waker) = queue.accept_waker.take() {
                waker();
            }
        }
    }
    for handle in to_wake_write {
        if let Some(waker) = inner.write_wakers.remove(&handle) {
            waker();
        }
    }
}

// ---------------------------------------------------------------------------
// Overlay socket handles
// ---------------------------------------------------------------------------

/// An overlay TCP listener. The stack queues inbound connections on its behalf (up to
/// [`NetStackConfig::listen_backlog`]) and `accept` hands them out in arrival order.
pub struct OverlayListener {
    inner: Arc<Mutex<Inner>>,
    id: u64,
    local: SocketAddr,
}

impl std::fmt::Debug for OverlayListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayListener")
            .field("id", &self.id)
            .field("local", &self.local)
            .finish()
    }
}

impl OverlayListener {
    /// WHY: Servers accept overlay connections just like `TcpListener::accept`.
    ///
    /// WHAT: Return the next established connection, or [`io::ErrorKind::WouldBlock`] if
    /// none is ready yet.
    ///
    /// HOW: Pops the oldest connection from this listener's accept queue (connections
    /// reset before being accepted are discarded) and re-arms the listening socket if
    /// the queue had been full.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when no connection is pending.
    ///
    /// # Panics
    /// Panics if the netstack mutex is poisoned.
    pub fn accept(&mut self) -> io::Result<OverlayStream> {
        let mut guard = self.inner.lock().expect("netstack mutex poisoned");
        let inner = &mut *guard;
        // Pick up a connection the armed socket took since the last poll.
        inner.rearm_listeners();

        loop {
            let Some(handle) = inner
                .listeners
                .get_mut(&self.id)
                .and_then(|queue| queue.pending.pop_front())
            else {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "no pending connection",
                ));
            };
            let socket = inner.sockets.get::<tcp::Socket>(handle);
            let peer = match (socket.state(), socket.remote_endpoint()) {
                (tcp::State::Closed, _) | (_, None) => None,
                (_, Some(remote)) => Some(from_smol_endpoint(remote)),
            };
            let Some(peer) = peer else {
                // Reset or aborted while queued: drop it and try the next one.
                inner.read_wakers.remove(&handle);
                inner.write_wakers.remove(&handle);
                inner.sockets.remove(handle);
                continue;
            };
            // A slot freed up: re-arm if the queue had been full.
            inner.rearm_listeners();
            return Ok(OverlayStream {
                inner: Arc::clone(&self.inner),
                handle,
                local: self.local,
                peer,
            });
        }
    }

    /// Register a one-shot waker fired when a connection becomes acceptable.
    ///
    /// # Panics
    /// Panics if the netstack mutex is poisoned.
    pub fn set_accept_waker(&self, waker: WakeFn) {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        if let Some(queue) = inner.listeners.get_mut(&self.id) {
            queue.accept_waker = Some(waker);
        }
    }

    /// The local endpoint this listener is bound to.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }
}

impl Drop for OverlayListener {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let Some(queue) = inner.listeners.remove(&self.id) else {
            return;
        };
        // Connections queued but never accepted go with the listener, as when a kernel
        // listener is closed.
        for handle in queue.armed.into_iter().chain(queue.pending) {
            inner.read_wakers.remove(&handle);
            inner.write_wakers.remove(&handle);
            inner.sockets.remove(handle);
        }
    }
}

/// An overlay TCP stream (a single connection). Cheaply cloneable — clones
/// share the same underlying smoltcp socket via `Arc<Mutex<Inner>>`, so they
/// must be driven from the same thread.
#[derive(Clone)]
pub struct OverlayStream {
    inner: Arc<Mutex<Inner>>,
    handle: SocketHandle,
    local: SocketAddr,
    peer: SocketAddr,
}

impl std::fmt::Debug for OverlayStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayStream")
            .field("local", &self.local)
            .field("peer", &self.peer)
            .finish()
    }
}

/// Whether a TCP socket is still setting up its connection (an accepted
/// socket sits in `SynReceived` until the client's ACK arrives).
fn still_connecting(state: tcp::State) -> bool {
    matches!(
        state,
        tcp::State::Listen | tcp::State::SynSent | tcp::State::SynReceived
    )
}

impl OverlayStream {
    /// WHY: Overlay streams behave like non-blocking TCP sockets for the driver task.
    ///
    /// WHAT: Read up to `buf.len()` bytes; `Ok(0)` means the peer closed (EOF).
    ///
    /// HOW: Bridges to smoltcp `recv_slice`; returns [`io::ErrorKind::WouldBlock`] when
    /// connected but no data is buffered.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when no data is available yet; other kinds on a
    /// broken connection.
    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let socket = inner.sockets.get_mut::<tcp::Socket>(self.handle);
        if socket.can_recv() {
            socket
                .recv_slice(buf)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("tcp recv: {e}")))
        } else if socket.may_recv() || still_connecting(socket.state()) {
            // `may_recv` is false until the handshake completes; a socket
            // that is still connecting has not ended, it has nothing *yet*.
            Err(io::Error::new(io::ErrorKind::WouldBlock, "no data"))
        } else {
            Ok(0)
        }
    }

    /// WHY: Overlay streams behave like non-blocking TCP sockets for the driver task.
    ///
    /// WHAT: Write up to `buf.len()` bytes, returning how many were enqueued.
    ///
    /// HOW: Bridges to smoltcp `send_slice`; returns [`io::ErrorKind::WouldBlock`] when
    /// the send buffer is full.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when the send buffer is full;
    /// [`io::ErrorKind::BrokenPipe`] when the send half is closed.
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let socket = inner.sockets.get_mut::<tcp::Socket>(self.handle);
        if socket.can_send() {
            socket
                .send_slice(buf)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("tcp send: {e}")))
        } else if socket.may_send() {
            Err(io::Error::new(io::ErrorKind::WouldBlock, "send buffer full"))
        } else {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "send half closed"))
        }
    }

    /// True while the connection can still exchange data in either direction. Note this
    /// is `true` during the handshake (`SYN-SENT`); use [`Self::may_send`] to test for an
    /// established, writable connection.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .sockets
            .get::<tcp::Socket>(self.handle)
            .is_active()
    }

    /// True once the connection is established and its send half is open — i.e. the
    /// handshake completed and the socket will accept application data.
    #[must_use]
    pub fn may_send(&self) -> bool {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .sockets
            .get::<tcp::Socket>(self.handle)
            .may_send()
    }

    /// True while the connection's receive half is open (data may still arrive).
    #[must_use]
    pub fn may_recv(&self) -> bool {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .sockets
            .get::<tcp::Socket>(self.handle)
            .may_recv()
    }

    /// Begin an orderly close of the send half (FIN).
    pub fn close(&self) {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .sockets
            .get_mut::<tcp::Socket>(self.handle)
            .close();
    }

    /// Register a one-shot waker fired when the stream becomes readable.
    pub fn set_read_waker(&self, waker: WakeFn) {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .read_wakers
            .insert(self.handle, waker);
    }

    /// Register a one-shot waker fired when the stream becomes writable.
    pub fn set_write_waker(&self, waker: WakeFn) {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .write_wakers
            .insert(self.handle, waker);
    }

    /// The local overlay endpoint.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    /// The remote overlay endpoint.
    #[must_use]
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer
    }
}

impl Drop for OverlayStream {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        inner.read_wakers.remove(&self.handle);
        inner.write_wakers.remove(&self.handle);
        inner.sockets.remove(self.handle);
    }
}

/// An overlay UDP socket.
pub struct OverlayUdp {
    inner: Arc<Mutex<Inner>>,
    handle: SocketHandle,
    local: SocketAddr,
}

impl std::fmt::Debug for OverlayUdp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayUdp")
            .field("local", &self.local)
            .finish()
    }
}

impl OverlayUdp {
    /// WHY: Overlay datagram sockets mirror `UdpSocket::recv_from`.
    ///
    /// WHAT: Receive one datagram into `buf`, returning its length and source.
    ///
    /// HOW: Bridges to smoltcp `recv_slice`; [`io::ErrorKind::WouldBlock`] when empty.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when no datagram is buffered.
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let socket = inner.sockets.get_mut::<udp::Socket>(self.handle);
        if !socket.can_recv() {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "no datagram"));
        }
        let (len, meta) = socket
            .recv_slice(buf)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("udp recv: {e}")))?;
        Ok((len, from_smol_endpoint(meta.endpoint)))
    }

    /// WHY: Overlay datagram sockets mirror `UdpSocket::send_to`.
    ///
    /// WHAT: Send `buf` as one datagram to `addr`.
    ///
    /// HOW: Bridges to smoltcp `send_slice`; [`io::ErrorKind::WouldBlock`] when full.
    ///
    /// # Errors
    /// [`io::ErrorKind::WouldBlock`] when the send buffer is full; other kinds on error.
    pub fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        let socket = inner.sockets.get_mut::<udp::Socket>(self.handle);
        if !socket.can_send() {
            return Err(io::Error::new(io::ErrorKind::WouldBlock, "send buffer full"));
        }
        socket
            .send_slice(buf, to_smol_endpoint(addr))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("udp send: {e}")))?;
        Ok(buf.len())
    }

    /// Register a one-shot waker fired when a datagram becomes available.
    pub fn set_read_waker(&self, waker: WakeFn) {
        self.inner
            .lock().expect("netstack mutex poisoned")
            .read_wakers
            .insert(self.handle, waker);
    }

    /// The local overlay endpoint this socket is bound to.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }
}

impl Drop for OverlayUdp {
    fn drop(&mut self) {
        let mut inner = self.inner.lock().expect("netstack mutex poisoned");
        inner.read_wakers.remove(&self.handle);
        inner.write_wakers.remove(&self.handle);
        inner.sockets.remove(self.handle);
    }
}

// ---------------------------------------------------------------------------
// std <-> smoltcp address conversions
// ---------------------------------------------------------------------------

fn to_smol_address(addr: IpAddr) -> IpAddress {
    // smoltcp 0.12 wraps `core::net` address types directly.
    match addr {
        IpAddr::V4(v4) => IpAddress::Ipv4(v4),
        IpAddr::V6(v6) => IpAddress::Ipv6(v6),
    }
}

fn from_smol_address(addr: IpAddress) -> IpAddr {
    match addr {
        IpAddress::Ipv4(v4) => IpAddr::V4(v4),
        IpAddress::Ipv6(v6) => IpAddr::V6(v6),
    }
}

fn to_smol_endpoint(addr: SocketAddr) -> IpEndpoint {
    IpEndpoint::new(to_smol_address(addr.ip()), addr.port())
}

fn from_smol_endpoint(endpoint: IpEndpoint) -> SocketAddr {
    SocketAddr::new(from_smol_address(endpoint.addr), endpoint.port)
}

fn to_listen_endpoint(addr: SocketAddr) -> IpListenEndpoint {
    if addr.ip().is_unspecified() {
        IpListenEndpoint {
            addr: None,
            port: addr.port(),
        }
    } else {
        IpListenEndpoint {
            addr: Some(to_smol_address(addr.ip())),
            port: addr.port(),
        }
    }
}

fn local_addr(iface: &Interface) -> IpAddr {
    iface
        .ip_addrs()
        .first()
        .map(|cidr| from_smol_address(cidr.address()))
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED))
}
