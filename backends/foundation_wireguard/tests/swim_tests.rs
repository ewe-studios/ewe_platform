//! SWIM membership tests (spec-55, feature 03; decision 05).
//!
//! WHY: Prove the sans-I/O core's guarantees — CRDT merge (commutative/idempotent),
//! epidemic convergence under message loss, low-false-positive failure detection, and
//! tombstone GC — with a simulated clock and in-memory lossy network.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use foundation_wireguard::shared::membership::{decode, encode};
use foundation_wireguard::{
    Capabilities, MemberState, PeerId, PeerRecord, Swim, SwimConfig, SwimMessage,
};
use tracing_test::traced_test;

fn peer_id(n: u8) -> PeerId {
    let mut bytes = [0u8; 32];
    bytes[0] = n;
    bytes[31] = n;
    PeerId(bytes)
}

fn record(n: u8) -> PeerRecord {
    let ip: IpAddr = format!("10.7.0.{n}").parse().unwrap();
    let endpoint: SocketAddr = format!("127.0.0.1:{}", 4000 + n as u16).parse().unwrap();
    PeerRecord::new(peer_id(n), ip, vec![endpoint], Capabilities::default())
}

fn sim_config() -> SwimConfig {
    SwimConfig {
        probe_interval: Duration::from_millis(100),
        probe_timeout: Duration::from_millis(50),
        ping_req_k: 2,
        suspicion_timeout: Duration::from_millis(200),
        // Long by default so GC does not interfere with convergence/detection tests; the
        // dedicated GC test overrides this with a short TTL.
        tombstone_ttl: Duration::from_secs(60),
        anti_entropy_interval: Duration::from_millis(80),
        gossip_fanout: 3,
    }
}

// ---------------------------------------------------------------------------
// Codec
// ---------------------------------------------------------------------------

#[test]
#[traced_test]
fn swim_message_codec_round_trips_all_variants() {
    let messages = vec![
        SwimMessage::Ping { seq: 7 },
        SwimMessage::Ack { seq: 7 },
        SwimMessage::PingReq {
            seq: 9,
            target: peer_id(3),
        },
        SwimMessage::Gossip {
            updates: vec![record(1), record(2)],
        },
        SwimMessage::SyncResponse {
            updates: vec![record(4)],
        },
    ];
    for msg in messages {
        let bytes = encode(&msg);
        let decoded = decode(&bytes).expect("decode");
        assert_eq!(decoded, msg, "round-trip preserves the message");
    }
    // A corrupt/short buffer is rejected, not panicked on.
    assert!(decode(&[]).is_err());
    assert!(decode(&[0xFF, 1, 2, 3]).is_err(), "bad version rejected");
}

// ---------------------------------------------------------------------------
// CRDT merge properties
// ---------------------------------------------------------------------------

#[test]
#[traced_test]
fn membership_merge_is_order_independent_and_idempotent() {
    // A separate observer node (id 100) merges records about peer 1 in different orders.
    let observer = || Swim::new(record(100), sim_config(), 42);

    let mut a = record(1);
    a.incarnation = 2;
    a.heartbeat = 5;
    let mut a_newer = a.clone();
    a_newer.heartbeat = 9;
    a_newer.endpoints = vec!["127.0.0.1:9999".parse().unwrap()];
    let mut a_dead = a.clone();
    a_dead.state = MemberState::Dead;

    let batch = [a.clone(), a_newer.clone(), a_dead.clone()];

    let mut s1 = observer();
    for r in &batch {
        s1.merge(std::slice::from_ref(r));
    }
    let mut s2 = observer();
    for r in batch.iter().rev() {
        s2.merge(std::slice::from_ref(r));
    }
    assert_eq!(
        s1.membership().get(&peer_id(1)),
        s2.membership().get(&peer_id(1)),
        "merge is order-independent"
    );
    assert_eq!(
        s1.membership().get(&peer_id(1)).unwrap().state,
        MemberState::Dead,
        "Dead wins at equal incarnation"
    );

    // Idempotency: merging the winner again changes nothing.
    let before = s1.membership().get(&peer_id(1)).cloned();
    s1.merge(&[a_dead]);
    assert_eq!(s1.membership().get(&peer_id(1)).cloned(), before);

    // Higher incarnation refutes Dead.
    let mut a_refuted = a.clone();
    a_refuted.incarnation = 3;
    a_refuted.state = MemberState::Alive;
    s1.merge(&[a_refuted]);
    assert_eq!(
        s1.membership().get(&peer_id(1)).unwrap().state,
        MemberState::Alive
    );
}

// ---------------------------------------------------------------------------
// Simulation harness
// ---------------------------------------------------------------------------

struct Sim {
    nodes: Vec<Swim>,
    index: HashMap<PeerId, usize>,
    queue: VecDeque<(PeerId, PeerId, SwimMessage)>,
    dead_nodes: Vec<PeerId>,
    base: Instant,
    clock: Duration,
    drop_per_thousand: u64,
    rng: u64,
}

impl Sim {
    fn new(n: u8, drop_per_thousand: u64) -> Self {
        Self::with_config(n, drop_per_thousand, sim_config())
    }

    fn with_config(n: u8, drop_per_thousand: u64, cfg: SwimConfig) -> Self {
        let mut nodes = Vec::new();
        let mut index = HashMap::new();
        for i in 0..n {
            let swim = Swim::new(record(i), cfg.clone(), (i as u64) + 1);
            index.insert(peer_id(i), i as usize);
            nodes.push(swim);
        }
        // Bootstrap: every non-seed node learns the seed (node 0) and announces itself.
        let seed_snapshot = nodes[0].snapshot();
        let mut queue = VecDeque::new();
        for i in 1..n as usize {
            nodes[i].merge(&seed_snapshot);
            queue.push_back((peer_id(i as u8), peer_id(0), SwimMessage::Gossip {
                updates: vec![nodes[i].my_record().clone()],
            }));
        }
        Self {
            nodes,
            index,
            queue,
            dead_nodes: Vec::new(),
            base: Instant::now(),
            clock: Duration::ZERO,
            drop_per_thousand,
            rng: 0x9E37_79B9_7F4A_7C15,
        }
    }

    fn rng_next(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn should_drop(&mut self) -> bool {
        if self.drop_per_thousand == 0 {
            return false;
        }
        self.rng_next() % 1000 < self.drop_per_thousand
    }

    fn enqueue(&mut self, from: PeerId, to: PeerId, msg: SwimMessage) {
        if self.dead_nodes.contains(&to) || self.dead_nodes.contains(&from) {
            return;
        }
        if self.should_drop() {
            return;
        }
        self.queue.push_back((from, to, msg));
    }

    /// Advance the clock by `dt`, tick every live node, then deliver all queued messages
    /// (delivery is "instant" within a step; the clock only moves between steps).
    fn step(&mut self, dt: Duration) {
        self.clock += dt;
        let now = self.base + self.clock;

        // Deterministic tick order (HashMap iteration order is randomized per process).
        let mut ids: Vec<PeerId> = self.index.keys().copied().collect();
        ids.sort();
        for id in ids {
            if self.dead_nodes.contains(&id) {
                continue;
            }
            let idx = self.index[&id];
            let (out, _events) = self.nodes[idx].tick(now);
            for om in out {
                self.enqueue(id, om.to, om.message);
            }
        }

        let mut budget = 200_000;
        while let Some((from, to, msg)) = self.queue.pop_front() {
            budget -= 1;
            assert!(budget > 0, "message delivery did not converge (gossip storm?)");
            if self.dead_nodes.contains(&to) {
                continue;
            }
            let Some(&idx) = self.index.get(&to) else {
                continue;
            };
            let (out, _events) = self.nodes[idx].on_message(from, msg);
            for om in out {
                self.enqueue(to, om.to, om.message);
            }
        }
    }

    fn run(&mut self, steps: usize, dt: Duration) {
        for _ in 0..steps {
            self.step(dt);
        }
    }

    fn kill(&mut self, n: u8) {
        self.dead_nodes.push(peer_id(n));
    }

    /// Every live node sees every other live node as `Alive`.
    fn fully_converged(&self) -> bool {
        let live: Vec<PeerId> = self
            .index
            .keys()
            .copied()
            .filter(|id| !self.dead_nodes.contains(id))
            .collect();
        for &id in &live {
            let node = &self.nodes[self.index[&id]];
            for &other in &live {
                if other == id {
                    continue;
                }
                match node.membership().get(&other) {
                    Some(r) if r.state == MemberState::Alive => {}
                    _ => return false,
                }
            }
        }
        true
    }
}

#[test]
#[traced_test]
fn swim_converges_across_nodes_without_loss() {
    let mut sim = Sim::new(6, 0);
    for _ in 0..100 {
        sim.step(Duration::from_millis(40));
        if sim.fully_converged() {
            break;
        }
    }
    assert!(sim.fully_converged(), "all nodes learned the full membership");
}

#[test]
#[traced_test]
fn swim_converges_under_message_loss() {
    // 30% of messages dropped; anti-entropy must still reconcile everyone.
    let mut sim = Sim::new(5, 300);
    for _ in 0..300 {
        sim.step(Duration::from_millis(40));
        if sim.fully_converged() {
            break;
        }
    }
    assert!(
        sim.fully_converged(),
        "membership converges despite 30% packet loss (anti-entropy)"
    );
}

#[test]
#[traced_test]
fn swim_detects_dead_node_without_false_positives() {
    let mut sim = Sim::new(5, 0);
    sim.run(60, Duration::from_millis(40));
    assert!(sim.fully_converged(), "converged before failure");

    // Kill node 4: it stops responding and its messages are dropped.
    sim.kill(4);
    let victim = peer_id(4);

    // Advance well past probe + suspicion timeouts so survivors declare it Dead.
    sim.run(120, Duration::from_millis(40));

    let survivors = [0u8, 1, 2, 3];
    for &s in &survivors {
        let node = &sim.nodes[sim.index[&peer_id(s)]];
        // The victim is Dead everywhere.
        assert_eq!(
            node.membership().get(&victim).map(|r| r.state),
            Some(MemberState::Dead),
            "node {s} declared the victim Dead"
        );
        // No false positives: every *other* survivor is still Alive.
        for &o in &survivors {
            if o == s {
                continue;
            }
            assert_eq!(
                node.membership().get(&peer_id(o)).map(|r| r.state),
                Some(MemberState::Alive),
                "node {s} still sees survivor {o} as Alive (no false positive)"
            );
        }
    }
}

#[test]
#[traced_test]
fn swim_garbage_collects_tombstones() {
    let mut cfg = sim_config();
    cfg.tombstone_ttl = Duration::from_millis(2000);
    let mut sim = Sim::with_config(4, 0, cfg);
    sim.run(60, Duration::from_millis(40));
    sim.kill(3);
    let victim = peer_id(3);
    let node0 = sim.index[&peer_id(0)];

    // Catch the tombstone the moment it appears (before it is GC'd).
    let mut detected = false;
    for _ in 0..80 {
        sim.step(Duration::from_millis(40));
        if sim.nodes[node0].membership().get(&victim).map(|r| r.state) == Some(MemberState::Dead) {
            detected = true;
            break;
        }
    }
    assert!(detected, "victim was tombstoned (Dead) after detection");

    // Advance well past the tombstone TTL; the record is garbage-collected and not
    // resurrected by peers that GC slightly later.
    sim.run(80, Duration::from_millis(40));
    assert!(
        sim.nodes[node0].membership().get(&victim).is_none(),
        "tombstone GC'd after TTL"
    );
}
