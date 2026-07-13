//! Mesh orchestration — where the pieces become a network (spec-55, feature 04).
//!
//! WHY: [`WgNode`] ties together bootstrap join (F02), SWIM membership (F03), the
//! per-peer tunnels + data plane (F00/F01) into one running mesh, and exposes overlay
//! sockets to the application ([`WgHandle`]).
//!
//! WHAT: [`WgConfig`] describes a node (seed or joiner); [`WgNode::join`] bootstraps,
//! wires every peer into one [`TunnelDriver`], runs SWIM **inside the tunnel**, keeps its
//! own bootstrap door open (masterless), and returns a [`WgHandle`].
//!
//! HOW: A single runtime loop drives the tunnel pump + the SWIM machine + membership
//! reconciliation. Membership → tunnels is reconciled every tick: `Alive` peers get a
//! [`WgTunnel`]; departed peers are dropped. The overlay data plane routes both app
//! traffic and the in-tunnel SWIM gossip.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use boringtun::x25519::{PublicKey, StaticSecret};
use foundation_nativeapis::dataplane::netstack::{OverlayListener, OverlayStream, OverlayUdp};
use foundation_nativeapis::dataplane::{DataPlane, NetStack, NetStackConfig};
use foundation_nativeapis::native::net::UdpSocket;

use super::bootstrap::{BootstrapClient, BootstrapHandler, BootstrapServer};
use super::driver::TunnelDriver;
use crate::shared::bootstrap::Admission;
use crate::shared::error::{WgError, WgResult};
use crate::shared::keys::IdentityKeypair;
use crate::shared::membership::message::{decode as swim_decode, encode as swim_encode};
use crate::shared::membership::{Capabilities, MemberState, PeerId, PeerRecord, Swim, SwimConfig};
use crate::shared::tunnel::WgTunnel;

/// Reserved overlay service port for in-tunnel SWIM gossip (decision 06).
const GOSSIP_PORT: u16 = 51999;
/// WireGuard persistent-keepalive interval (seconds) for mesh tunnels.
const WG_KEEPALIVE: u16 = 25;
/// Overlay subnet prefix — a `/8` so every derived `10.x.y.z` address is on-link.
const OVERLAY_PREFIX: u8 = 8;
/// Runtime loop pacing.
const RUNTIME_TICK: Duration = Duration::from_millis(2);

// WgConfig is defined in shared::config (spec-55, feature 09).
pub use crate::shared::config::WgConfig;

/// A public view of a mesh member.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// The member's identity (public key).
    pub identity: PeerId,
    /// The member's overlay (tunnel) IP.
    pub tunnel_ip: IpAddr,
    /// The member's reachable UDP endpoints.
    pub endpoints: Vec<SocketAddr>,
    /// The member's liveness state.
    pub state: MemberState,
    /// The member's capabilities.
    pub caps: Capabilities,
}

/// Shared state between the runtime loop and the handle.
struct MeshShared {
    members: Mutex<Vec<PeerRecord>>,
    my_ip: IpAddr,
    my_id: PeerId,
    bootstrap_addr: SocketAddr,
    udp_addr: SocketAddr,
}

/// The admission policy this node applies at `Join` time (decision 11). Because the mesh
/// is masterless, admission is a per-member policy, not a central gate.
struct AdmissionPolicy {
    /// Set to immediately revoke the seed: subsequent joins are refused.
    revoked: AtomicBool,
    /// Optional seed TTL (unix seconds).
    expires_at: Option<u64>,
}

impl AdmissionPolicy {
    fn decide(&self) -> Admission {
        if self.revoked.load(Ordering::Relaxed) {
            return Admission::Reject;
        }
        if let Some(expiry) = self.expires_at {
            if now_unix() > expiry {
                return Admission::Reject;
            }
        }
        Admission::Admit
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A configured (but not yet running) mesh node.
#[derive(Debug)]
pub struct WgNode {
    config: WgConfig,
}

impl WgNode {
    /// WHY: Build a node from configuration before joining.
    ///
    /// WHAT: Wrap a [`WgConfig`].
    ///
    /// HOW: Stores it; all work happens in [`Self::join`].
    #[must_use]
    pub fn from_config(config: WgConfig) -> Self {
        Self { config }
    }

    /// WHY: Bring the node onto the mesh.
    ///
    /// WHAT: Bootstrap (if a joiner), generate identity keys, wire every peer into the
    /// data plane, start the runtime + bootstrap-server threads, and return a handle.
    ///
    /// HOW: See the module docs. Blocks only on the initial bootstrap join.
    ///
    /// # Errors
    /// [`WgError`] if binding, key derivation, or the bootstrap join fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn join(self) -> WgResult<WgHandle> {
        let network_id = self.config.network_id();
        let boot = self.config.wg_seed().derive_bootstrap(&network_id);
        let identity = IdentityKeypair::generate()?;
        let my_id = PeerId(*identity.public().as_bytes());
        let my_ip = derive_tunnel_ip(&my_id);

        let udp = UdpSocket::bind(self.config.udp_listen())?;
        let udp_addr = udp.get_ref().local_addr()?;

        let netstack = NetStack::new(NetStackConfig {
            address: my_ip,
            prefix_len: OVERLAY_PREFIX,
            mtu: 1380,
        });

        let my_record =
            PeerRecord::new(my_id, my_ip, vec![udp_addr], self.config.caps());
        let mut swim = Swim::new(my_record.clone(), mesh_swim_config(), seed_from_id(&my_id));

        let seed_endpoints = self.config.seed_endpoints();
        // Bootstrap join (joiners only): pull membership + announce self.
        if !seed_endpoints.is_empty() {
            let client = BootstrapClient::new(boot.tls_psk, network_id)?;
            let mut joined = false;
            for endpoint in seed_endpoints {
                let Ok(mut conn) = client.connect(*endpoint) else {
                    continue;
                };
                let (decision, members) = conn.join(my_record.clone())?;
                match decision {
                    Admission::Admit => {
                        swim.merge(&members);
                        joined = true;
                        break;
                    }
                    Admission::Reject => {
                        return Err(WgError::Protocol("join rejected by seed".into()))
                    }
                    Admission::Pending => {
                        return Err(WgError::Protocol("join pending approval".into()))
                    }
                }
            }
            if !joined {
                return Err(WgError::Protocol("no reachable seed endpoint".into()));
            }
        }

        let swim = Arc::new(Mutex::new(swim));
        let policy = Arc::new(AdmissionPolicy {
            revoked: AtomicBool::new(false),
            expires_at: self.config.seed_expires_at(),
        });

        // Keep our own bootstrap door open (masterless — anyone can join through us).
        let handler: Arc<dyn BootstrapHandler> = Arc::new(MeshBootstrapHandler {
            swim: Arc::clone(&swim),
            policy: Arc::clone(&policy),
        });
        let server = BootstrapServer::bind(self.config.bootstrap_listen(), boot.tls_psk, handler)?;
        let bootstrap_addr = server.local_addr()?;

        let shared = Arc::new(MeshShared {
            members: Mutex::new(swim.lock().expect("swim lock").snapshot()),
            my_ip,
            my_id,
            bootstrap_addr,
            udp_addr,
        });
        let stop = Arc::new(AtomicBool::new(false));

        // Bootstrap server thread.
        let server_stop = Arc::clone(&stop);
        let bootstrap_thread = thread::spawn(move || {
            if let Err(err) = server.serve_until(&server_stop) {
                tracing::debug!(error = %err, "bootstrap server stopped");
            }
        });

        // Runtime: tunnel pump + SWIM + reconciliation. The first loop iteration
        // reconciles any peers already learned during bootstrap into tunnels.
        let driver = TunnelDriver::new(udp, netstack.clone());
        let gossip = netstack
            .clone()
            .udp_bind(SocketAddr::new(my_ip, GOSSIP_PORT))
            .map_err(WgError::Io)?;
        let runtime = Runtime {
            driver,
            swim: Arc::clone(&swim),
            gossip,
            my_id,
            my_secret: identity.secret().clone(),
            wg_psk: boot.psk,
            shared: Arc::clone(&shared),
            stop: Arc::clone(&stop),
            next_index: AtomicU32::new(1),
        };

        let runtime_thread = thread::spawn(move || run_runtime(runtime));

        Ok(WgHandle {
            netstack,
            shared,
            policy,
            stop,
            threads: vec![bootstrap_thread, runtime_thread],
        })
    }
}

/// A running node's app-facing handle. Cloneable overlay-socket factory + membership view.
pub struct WgHandle {
    netstack: NetStack,
    shared: Arc<MeshShared>,
    policy: Arc<AdmissionPolicy>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for WgHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WgHandle")
            .field("my_ip", &self.shared.my_ip)
            .field("bootstrap_addr", &self.shared.bootstrap_addr)
            .finish_non_exhaustive()
    }
}

impl WgHandle {
    /// This node's overlay (tunnel) IP.
    #[must_use]
    pub fn overlay_ip(&self) -> IpAddr {
        self.shared.my_ip
    }

    /// This node's TLS-PSK bootstrap address (hand to later joiners).
    #[must_use]
    pub fn bootstrap_addr(&self) -> SocketAddr {
        self.shared.bootstrap_addr
    }

    /// This node's outer WireGuard UDP address.
    #[must_use]
    pub fn udp_addr(&self) -> SocketAddr {
        self.shared.udp_addr
    }

    /// This node's identity.
    #[must_use]
    pub fn identity(&self) -> PeerId {
        self.shared.my_id
    }

    /// Open an overlay TCP listener on this node's overlay IP.
    ///
    /// # Errors
    /// [`std::io::Error`] if the listen fails.
    pub fn tcp_listen(&self, port: u16) -> std::io::Result<OverlayListener> {
        self.netstack
            .clone()
            .tcp_listen(SocketAddr::new(self.shared.my_ip, port))
    }

    /// Open an overlay TCP connection to a peer's overlay IP.
    ///
    /// # Errors
    /// [`std::io::Error`] if the connect fails.
    pub fn tcp_connect(&self, peer_ip: IpAddr, port: u16) -> std::io::Result<OverlayStream> {
        self.netstack
            .clone()
            .tcp_connect(SocketAddr::new(peer_ip, port))
    }

    /// Bind an overlay UDP socket on this node's overlay IP.
    ///
    /// # Errors
    /// [`std::io::Error`] if the bind fails.
    pub fn udp_bind(&self, port: u16) -> std::io::Result<OverlayUdp> {
        self.netstack
            .clone()
            .udp_bind(SocketAddr::new(self.shared.my_ip, port))
    }

    /// The current membership view (including self).
    #[must_use]
    pub fn members(&self) -> Vec<PeerInfo> {
        self.shared
            .members
            .lock()
            .expect("members lock")
            .iter()
            .map(|r| PeerInfo {
                identity: r.id,
                tunnel_ip: r.tunnel_ip,
                endpoints: r.endpoints.clone(),
                state: r.state,
                caps: r.caps,
            })
            .collect()
    }

    /// WHY: Tests and callers often need to wait until a peer is reachable.
    ///
    /// WHAT: Block until a member with `tunnel_ip` is `Alive`, or `timeout` elapses.
    ///
    /// HOW: Polls the membership snapshot.
    ///
    /// # Panics
    /// Never panics.
    pub fn wait_for_peer(&self, tunnel_ip: IpAddr, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.members().iter().any(|p| {
                p.tunnel_ip == tunnel_ip && p.state == MemberState::Alive
            }) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// WHY: After the mesh has formed on identity keys, the ephemeral seed can be revoked
    /// so a later leak of the old seed cannot join through this node (decision 11).
    ///
    /// WHAT: Immediately refuse all future bootstrap joins presenting this seed.
    ///
    /// HOW: Flips the shared admission policy to reject; existing tunnels are unaffected
    /// (they run on identity keys, not the seed).
    pub fn revoke_seed(&self) {
        self.policy.revoked.store(true, Ordering::Relaxed);
    }

    /// Stop the node's threads and tear down the mesh runtime.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for handle in self.threads.drain(..) {
            let _ = handle.join();
        }
    }
}

impl Drop for WgHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

struct Runtime {
    driver: TunnelDriver,
    swim: Arc<Mutex<Swim>>,
    gossip: OverlayUdp,
    my_id: PeerId,
    my_secret: StaticSecret,
    wg_psk: [u8; 32],
    shared: Arc<MeshShared>,
    stop: Arc<AtomicBool>,
    next_index: AtomicU32,
}

fn run_runtime(mut runtime: Runtime) {
    let mut buf = [0u8; 8192];
    while !runtime.stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        runtime.driver.drive_once(now);

        {
            let mut swim = runtime.swim.lock().expect("swim lock");

            // Inbound in-tunnel gossip.
            loop {
                match runtime.gossip.recv_from(&mut buf) {
                    Ok((n, src)) => {
                        if let Some(from) = id_by_ip(&swim, src.ip()) {
                            if let Ok(msg) = swim_decode(&buf[..n]) {
                                let (out, _events) = swim.on_message(from, msg);
                                send_swim(&runtime.gossip, &swim, out);
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => break,
                }
            }

            let (out, _events) = swim.tick(now);
            send_swim(&runtime.gossip, &swim, out);

            reconcile(
                &mut runtime.driver,
                &swim,
                &runtime.my_id,
                &runtime.my_secret,
                runtime.wg_psk,
                &runtime.next_index,
            );

            *runtime.shared.members.lock().expect("members lock") = swim.snapshot();
        }

        thread::sleep(RUNTIME_TICK);
    }
}

/// Send a batch of SWIM outbound messages over the in-tunnel gossip socket.
fn send_swim(
    gossip: &OverlayUdp,
    swim: &Swim,
    out: Vec<crate::shared::membership::SwimOutbound>,
) {
    for om in out {
        let Some(record) = swim.membership().get(&om.to) else {
            continue;
        };
        let bytes = swim_encode(&om.message);
        let _ = gossip.send_to(&bytes, SocketAddr::new(record.tunnel_ip, GOSSIP_PORT));
    }
}

/// Reconcile the driver's peer tunnels against the current alive membership.
fn reconcile(
    driver: &mut TunnelDriver,
    swim: &Swim,
    my_id: &PeerId,
    my_secret: &StaticSecret,
    wg_psk: [u8; 32],
    next_index: &AtomicU32,
) {
    let mut alive_ips: HashSet<IpAddr> = HashSet::new();
    for record in swim.membership().iter() {
        if record.id == *my_id || record.state != MemberState::Alive {
            continue;
        }
        alive_ips.insert(record.tunnel_ip);
        if !driver.has_peer_ip(record.tunnel_ip) {
            let Some(endpoint) = record.endpoints.first() else {
                continue;
            };
            let index = next_index.fetch_add(1, Ordering::Relaxed);
            let peer_public = PublicKey::from(record.id.0);
            let tunnel = WgTunnel::new(
                my_secret.clone(),
                peer_public,
                Some(wg_psk),
                Some(WG_KEEPALIVE),
                index,
            );
            driver.add_peer(tunnel, *endpoint, record.tunnel_ip);
        }
    }
    // Drop tunnels to peers that are gone.
    for ip in driver.peer_ips() {
        if !alive_ips.contains(&ip) {
            driver.remove_peer_ip(ip);
        }
    }
}

fn id_by_ip(swim: &Swim, ip: IpAddr) -> Option<PeerId> {
    swim.membership()
        .iter()
        .find(|r| r.tunnel_ip == ip)
        .map(|r| r.id)
}

// ---------------------------------------------------------------------------
// Bootstrap handler backed by the shared Swim
// ---------------------------------------------------------------------------

struct MeshBootstrapHandler {
    swim: Arc<Mutex<Swim>>,
    policy: Arc<AdmissionPolicy>,
}

impl BootstrapHandler for MeshBootstrapHandler {
    fn on_join(&self, record: PeerRecord) -> (Admission, Vec<PeerRecord>) {
        match self.policy.decide() {
            Admission::Admit => {
                let mut swim = self.swim.lock().expect("swim lock");
                swim.merge(std::slice::from_ref(&record));
                (Admission::Admit, swim.snapshot())
            }
            // Revoked/expired seed → refuse, hand back nothing.
            other => (other, Vec::new()),
        }
    }

    fn membership(&self) -> Vec<PeerRecord> {
        self.swim.lock().expect("swim lock").snapshot()
    }

    fn on_announce(&self, record: PeerRecord) {
        self.swim
            .lock()
            .expect("swim lock")
            .merge(std::slice::from_ref(&record));
    }
}

// ---------------------------------------------------------------------------
// IPAM + helpers
// ---------------------------------------------------------------------------

/// Derive a stable overlay IP from an identity key (decision 12: IP-from-pubkey). Uses a
/// `10.x.y.z/8` address; collisions are gossip-detectable (deferred).
fn derive_tunnel_ip(id: &PeerId) -> IpAddr {
    let b = id.as_bytes();
    let last = if b[2] == 0 || b[2] == 255 { 1 } else { b[2] };
    IpAddr::V4(Ipv4Addr::new(10, b[0], b[1], last))
}

fn seed_from_id(id: &PeerId) -> u64 {
    let b = id.as_bytes();
    u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

fn mesh_swim_config() -> SwimConfig {
    SwimConfig {
        probe_interval: Duration::from_millis(400),
        probe_timeout: Duration::from_millis(250),
        ping_req_k: 2,
        suspicion_timeout: Duration::from_millis(1500),
        tombstone_ttl: Duration::from_secs(30),
        anti_entropy_interval: Duration::from_millis(500),
        gossip_fanout: 4,
    }
}
