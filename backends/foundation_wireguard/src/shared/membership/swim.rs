//! The sans-I/O SWIM state machine (spec-55, feature 03; decision 05).
//!
//! WHY: Robust, low-false-positive failure detection and epidemic membership spread with
//! no coordinator — driven by a valtron timer + inbound messages, so it is fully testable
//! with a simulated clock and network.
//!
//! WHAT: [`Swim`] consumes [`Swim::tick`] and [`Swim::on_message`] and emits
//! [`SwimOutbound`] messages + [`SwimEvent`]s. [`Swim::merge`] applies a batch of records
//! (join / `PullMembership` / anti-entropy).
//!
//! HOW: Direct `Ping` → indirect `PingReq` (k random members) → `Suspect` → `Dead`
//! tombstone, with incarnation-based refutation and periodic anti-entropy digests. All
//! random choices come from a seeded xorshift PRNG for reproducible simulations.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::message::{DigestEntry, SwimMessage, SwimOutbound};
use super::record::{MemberState, PeerId, PeerRecord};
use super::{Membership, MergeOutcome};

/// Tunables for the SWIM protocol. Defaults suit a small service mesh; simulations shrink
/// the intervals.
#[derive(Debug, Clone)]
pub struct SwimConfig {
    /// How often a direct probe is issued.
    pub probe_interval: Duration,
    /// How long to wait for an ack before escalating to an indirect probe.
    pub probe_timeout: Duration,
    /// Number of random members asked to probe indirectly (`ping-req` fan-out).
    pub ping_req_k: usize,
    /// How long a `Suspect` has to be refuted before it is declared `Dead`.
    pub suspicion_timeout: Duration,
    /// How long a `Dead` tombstone lingers before garbage collection.
    pub tombstone_ttl: Duration,
    /// How often an anti-entropy digest is sent to a random peer.
    pub anti_entropy_interval: Duration,
    /// How many random peers a gossip broadcast reaches.
    pub gossip_fanout: usize,
}

impl Default for SwimConfig {
    fn default() -> Self {
        Self {
            probe_interval: Duration::from_secs(1),
            probe_timeout: Duration::from_millis(500),
            ping_req_k: 3,
            suspicion_timeout: Duration::from_secs(5),
            tombstone_ttl: Duration::from_secs(60),
            anti_entropy_interval: Duration::from_secs(10),
            gossip_fanout: 3,
        }
    }
}

/// Observable membership changes emitted by [`Swim`]; consumed by the mesh (feature 04)
/// to add/update/remove tunnels and by relay selection (feature 05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwimEvent {
    /// A peer became reachable (new member or a rejoin).
    PeerUp(PeerRecord),
    /// A known peer's reachable endpoints changed.
    PeerEndpointChanged(PeerRecord),
    /// A known peer's capabilities changed.
    PeerCapsChanged(PeerRecord),
    /// A peer was declared dead (tombstoned).
    PeerDown(PeerId),
}

/// An in-flight liveness probe.
#[derive(Debug, Clone)]
struct PendingProbe {
    target: PeerId,
    seq: u64,
    deadline: Instant,
    indirect: bool,
}

/// The SWIM state machine for one node.
#[derive(Debug)]
pub struct Swim {
    my_record: PeerRecord,
    members: Membership,
    cfg: SwimConfig,
    rng: u64,
    seq: u64,
    started: bool,
    last_probe: Instant,
    last_anti_entropy: Instant,
    pending: Option<PendingProbe>,
    /// Set when a gossip challenged us and we bumped our incarnation; drives one
    /// refutation broadcast.
    refute_pending: bool,
    /// PeerId → instant at which a `Suspect` becomes `Dead`.
    suspicion: BTreeMap<PeerId, Instant>,
    /// PeerId → instant at which a `Dead` tombstone is garbage-collected.
    tombstones: BTreeMap<PeerId, Instant>,
    /// PeerId → incarnation at which it was reaped (GC'd). Suppresses resurrection of a
    /// tombstone by a peer that GCs later; a genuine rejoin (higher-incarnation `Alive`)
    /// clears the entry.
    reaped: BTreeMap<PeerId, u64>,
    /// probe seq → (requester, target) for indirect probes we are servicing.
    forwards: BTreeMap<u64, (PeerId, PeerId)>,
}

impl Swim {
    /// WHY: Each node runs one SWIM instance describing itself.
    ///
    /// WHAT: Create a state machine for `my_record` with `cfg` and a PRNG `seed`.
    ///
    /// HOW: Starts with an empty membership; timers initialise on the first `tick`.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(my_record: PeerRecord, cfg: SwimConfig, seed: u64) -> Self {
        Self {
            my_record,
            members: Membership::new(),
            cfg,
            rng: seed | 1, // xorshift must be non-zero
            seq: 0,
            started: false,
            last_probe: Instant::now(),
            last_anti_entropy: Instant::now(),
            pending: None,
            refute_pending: false,
            suspicion: BTreeMap::new(),
            tombstones: BTreeMap::new(),
            reaped: BTreeMap::new(),
            forwards: BTreeMap::new(),
        }
    }

    /// This node's own record (for `Announce`).
    #[must_use]
    pub fn my_record(&self) -> &PeerRecord {
        &self.my_record
    }

    /// This node's identity.
    #[must_use]
    pub fn my_id(&self) -> PeerId {
        self.my_record.id
    }

    /// Read-only access to the membership set.
    #[must_use]
    pub fn membership(&self) -> &Membership {
        &self.members
    }

    /// WHY: A joiner's `PullMembership` (feature 02/04) needs the full known set,
    /// including this node's own record.
    ///
    /// WHAT: Every record we know plus our own self-record.
    ///
    /// HOW: Clones the membership set and appends `my_record`.
    #[must_use]
    pub fn snapshot(&self) -> Vec<PeerRecord> {
        let mut all = self.members.all();
        all.push(self.my_record.clone());
        all
    }

    /// WHY: Join and anti-entropy deliver batches of records at once.
    ///
    /// WHAT: Merge `records`, returning the observable [`SwimEvent`]s and refuting any
    /// record about ourselves.
    ///
    /// HOW: Applies each record through the CRDT merge; a record about us that marks us
    /// non-`Alive` triggers refutation (handled by the caller draining the returned
    /// broadcast is unnecessary — refutation is emitted from `tick`/`on_message`; here we
    /// simply bump our incarnation).
    ///
    /// # Panics
    /// Never panics.
    pub fn merge(&mut self, records: &[PeerRecord]) -> Vec<SwimEvent> {
        let mut events = Vec::new();
        for record in records {
            if let Some(event) = self.apply_record(record.clone()) {
                events.push(event);
            }
        }
        events
    }

    /// WHY: The driver's periodic timer advances failure detection and anti-entropy.
    ///
    /// WHAT: Advance timers as of `now`, returning outbound messages + membership events.
    ///
    /// HOW: Escalates expired probes, ages suspicions into deaths, GCs tombstones, issues
    /// the next probe, and periodically emits an anti-entropy digest.
    ///
    /// # Panics
    /// Never panics.
    pub fn tick(&mut self, now: Instant) -> (Vec<SwimOutbound>, Vec<SwimEvent>) {
        if !self.started {
            self.started = true;
            self.last_probe = now;
            self.last_anti_entropy = now;
        }

        let mut out = Vec::new();
        let mut events = Vec::new();

        self.expire_pending_probe(now, &mut out, &mut events);
        self.age_suspicions(now, &mut out, &mut events);
        self.gc_tombstones(now);
        self.maybe_probe(now, &mut out);
        self.maybe_anti_entropy(now, &mut out);

        (out, events)
    }

    /// WHY: Inbound SWIM traffic drives liveness and membership.
    ///
    /// WHAT: Handle one message from `from`, returning replies + events.
    ///
    /// HOW: Dispatches by message type (see module docs). Any direct contact from a peer
    /// is evidence it is alive, so a local suspicion of `from` is cleared.
    ///
    /// # Panics
    /// Never panics.
    pub fn on_message(
        &mut self,
        from: PeerId,
        msg: SwimMessage,
    ) -> (Vec<SwimOutbound>, Vec<SwimEvent>) {
        let mut out = Vec::new();
        let mut events = Vec::new();

        // Hearing directly from a peer proves it is alive.
        self.suspicion.remove(&from);

        match msg {
            SwimMessage::Ping { seq } => {
                out.push(SwimOutbound::new(from, SwimMessage::Ack { seq }));
            }
            SwimMessage::Ack { seq } => self.on_ack(seq, &mut out),
            SwimMessage::PingReq { seq, target } => {
                // Service the indirect probe: remember the requester, ping the target.
                self.forwards.insert(seq, (from, target));
                out.push(SwimOutbound::new(target, SwimMessage::Ping { seq }));
            }
            SwimMessage::Gossip { updates } => {
                self.absorb_and_regossip(updates, &mut out, &mut events);
            }
            SwimMessage::SyncDigest { entries } => {
                let updates = self.records_newer_than(&entries);
                out.push(SwimOutbound::new(from, SwimMessage::SyncResponse { updates }));
            }
            SwimMessage::SyncResponse { updates } => {
                self.absorb_and_regossip(updates, &mut out, &mut events);
            }
        }

        (out, events)
    }

    // ----- probe lifecycle -------------------------------------------------

    fn maybe_probe(&mut self, now: Instant, out: &mut Vec<SwimOutbound>) {
        if self.pending.is_some() || now.duration_since(self.last_probe) < self.cfg.probe_interval {
            return;
        }
        self.last_probe = now;
        // Bump our heartbeat so LWW favours our freshest self-record.
        self.my_record.heartbeat += 1;

        let Some(target) = self.random_alive_member() else {
            return;
        };
        self.seq += 1;
        let seq = self.seq;
        self.pending = Some(PendingProbe {
            target,
            seq,
            deadline: now + self.cfg.probe_timeout,
            indirect: false,
        });
        out.push(SwimOutbound::new(target, SwimMessage::Ping { seq }));
    }

    fn expire_pending_probe(
        &mut self,
        now: Instant,
        out: &mut Vec<SwimOutbound>,
        events: &mut Vec<SwimEvent>,
    ) {
        let Some(probe) = self.pending.clone() else {
            return;
        };
        if now < probe.deadline {
            return;
        }
        if probe.indirect {
            // Indirect probe also failed → suspect the target.
            self.pending = None;
            self.suspect(probe.target, now, out, events);
        } else {
            // Escalate: ask k random members to probe on our behalf.
            let helpers = self.random_members_excluding(probe.target, self.cfg.ping_req_k);
            for helper in helpers {
                out.push(SwimOutbound::new(
                    helper,
                    SwimMessage::PingReq {
                        seq: probe.seq,
                        target: probe.target,
                    },
                ));
            }
            self.pending = Some(PendingProbe {
                deadline: now + self.cfg.probe_timeout,
                indirect: true,
                ..probe
            });
        }
    }

    fn on_ack(&mut self, seq: u64, out: &mut Vec<SwimOutbound>) {
        // Forwarded ack: relay it back to the original requester.
        if let Some((requester, _target)) = self.forwards.remove(&seq) {
            out.push(SwimOutbound::new(requester, SwimMessage::Ack { seq }));
            return;
        }
        // Our own probe acked → target is alive; cancel any suspicion.
        if let Some(probe) = &self.pending {
            if probe.seq == seq {
                let target = probe.target;
                self.pending = None;
                self.suspicion.remove(&target);
            }
        }
    }

    // ----- suspicion / death ----------------------------------------------

    fn suspect(
        &mut self,
        target: PeerId,
        now: Instant,
        out: &mut Vec<SwimOutbound>,
        events: &mut Vec<SwimEvent>,
    ) {
        let Some(record) = self.members.get(&target).cloned() else {
            return;
        };
        if record.state != MemberState::Alive {
            return;
        }
        let mut suspected = record;
        suspected.state = MemberState::Suspect;
        if let Some(event) = self.apply_record(suspected.clone()) {
            events.push(event);
        }
        self.suspicion
            .insert(target, now + self.cfg.suspicion_timeout);
        self.broadcast(SwimMessage::Gossip {
            updates: vec![suspected],
        }, out);
    }

    fn age_suspicions(
        &mut self,
        now: Instant,
        out: &mut Vec<SwimOutbound>,
        events: &mut Vec<SwimEvent>,
    ) {
        let expired: Vec<PeerId> = self
            .suspicion
            .iter()
            .filter(|(_, &deadline)| now >= deadline)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            self.suspicion.remove(&id);
            let Some(record) = self.members.get(&id).cloned() else {
                continue;
            };
            if record.state != MemberState::Suspect {
                continue;
            }
            let mut dead = record;
            dead.state = MemberState::Dead;
            if let Some(event) = self.apply_record(dead.clone()) {
                events.push(event);
            }
            self.tombstones.insert(id, now + self.cfg.tombstone_ttl);
            self.broadcast(SwimMessage::Gossip { updates: vec![dead] }, out);
        }
    }

    fn gc_tombstones(&mut self, now: Instant) {
        // Start a GC clock for every `Dead` member — including deaths learned via gossip,
        // which never went through our own `age_suspicions`.
        let dead_ids: Vec<PeerId> = self
            .members
            .iter()
            .filter(|r| r.state == MemberState::Dead)
            .map(|r| r.id)
            .collect();
        for &id in &dead_ids {
            self.tombstones
                .entry(id)
                .or_insert(now + self.cfg.tombstone_ttl);
        }
        // Forget timers for members that are no longer `Dead` (rejoined/refuted).
        self.tombstones.retain(|id, _| dead_ids.contains(id));

        let expired: Vec<PeerId> = self
            .tombstones
            .iter()
            .filter(|(_, &deadline)| now >= deadline)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            self.tombstones.remove(&id);
            if let Some(record) = self.members.get(&id) {
                self.reaped.insert(id, record.incarnation);
            }
            self.members.remove(&id);
        }
    }

    // ----- membership merge + refutation ----------------------------------

    /// Apply one record and translate the merge outcome into an event. Records about
    /// ourselves are never stored; they only drive refutation (handled by callers).
    fn apply_record(&mut self, record: PeerRecord) -> Option<SwimEvent> {
        if record.id == self.my_record.id {
            // Refutation bookkeeping: adopt a higher incarnation if we are challenged.
            if record.state != MemberState::Alive && record.incarnation >= self.my_record.incarnation
            {
                self.my_record.incarnation = record.incarnation + 1;
                self.my_record.state = MemberState::Alive;
                self.my_record.heartbeat += 1;
                self.refute_pending = true;
            }
            return None;
        }
        let id = record.id;
        // Resurrection guard: a record for a reaped id is ignored unless it is a genuine
        // rejoin — an `Alive` record with a strictly higher incarnation.
        if let Some(&reaped_inc) = self.reaped.get(&id) {
            let is_rejoin = record.state == MemberState::Alive && record.incarnation > reaped_inc;
            if !is_rejoin {
                return None;
            }
            self.reaped.remove(&id);
        }
        match self.members.merge_record(record) {
            MergeOutcome::Unchanged => None,
            MergeOutcome::Inserted => {
                let inserted = self.members.get(&id)?.clone();
                if inserted.state == MemberState::Dead {
                    // A record we first learn as dead: tombstone it, no PeerUp.
                    None
                } else {
                    Some(SwimEvent::PeerUp(inserted))
                }
            }
            MergeOutcome::Updated {
                was_dead,
                now_dead,
                endpoints_changed,
                caps_changed,
            } => {
                if now_dead {
                    Some(SwimEvent::PeerDown(id))
                } else if was_dead {
                    Some(SwimEvent::PeerUp(self.members.get(&id)?.clone()))
                } else if endpoints_changed {
                    Some(SwimEvent::PeerEndpointChanged(self.members.get(&id)?.clone()))
                } else if caps_changed {
                    Some(SwimEvent::PeerCapsChanged(self.members.get(&id)?.clone()))
                } else {
                    None
                }
            }
        }
    }

    /// Merge a batch of records, re-gossiping only those that observably changed (epidemic
    /// spread that terminates once the mesh has converged) and emitting a refutation if we
    /// were challenged.
    fn absorb_and_regossip(
        &mut self,
        updates: Vec<PeerRecord>,
        out: &mut Vec<SwimOutbound>,
        events: &mut Vec<SwimEvent>,
    ) {
        let mut changed = Vec::new();
        for record in updates {
            if let Some(event) = self.apply_record(record.clone()) {
                events.push(event);
                changed.push(record);
            }
        }
        if !changed.is_empty() {
            self.broadcast(SwimMessage::Gossip { updates: changed }, out);
        }
        self.emit_refutation(out);
    }

    /// Broadcast our refreshed self-record exactly once after a challenge.
    fn emit_refutation(&mut self, out: &mut Vec<SwimOutbound>) {
        if !self.refute_pending {
            return;
        }
        self.refute_pending = false;
        self.broadcast(
            SwimMessage::Gossip {
                updates: vec![self.my_record.clone()],
            },
            out,
        );
    }

    // ----- anti-entropy ----------------------------------------------------

    fn maybe_anti_entropy(&mut self, now: Instant, out: &mut Vec<SwimOutbound>) {
        if now.duration_since(self.last_anti_entropy) < self.cfg.anti_entropy_interval {
            return;
        }
        self.last_anti_entropy = now;
        let Some(peer) = self.random_alive_member() else {
            return;
        };
        let mut entries: Vec<DigestEntry> = self
            .members
            .iter()
            .map(|r| DigestEntry {
                id: r.id,
                incarnation: r.incarnation,
                heartbeat: r.heartbeat,
                state_rank: r.state.rank(),
            })
            .collect();
        // Include ourselves so peers learn our latest self-record.
        entries.push(DigestEntry {
            id: self.my_record.id,
            incarnation: self.my_record.incarnation,
            heartbeat: self.my_record.heartbeat,
            state_rank: self.my_record.state.rank(),
        });
        out.push(SwimOutbound::new(peer, SwimMessage::SyncDigest { entries }));
    }

    /// Return records the sender's digest is missing or holds a staler version of.
    fn records_newer_than(&self, entries: &[DigestEntry]) -> Vec<PeerRecord> {
        let digest: BTreeMap<PeerId, (u64, u64, u8)> = entries
            .iter()
            .map(|e| (e.id, (e.incarnation, e.heartbeat, e.state_rank)))
            .collect();

        let mut updates = Vec::new();
        // Our own record.
        self.push_if_newer(&self.my_record, &digest, &mut updates);
        for record in self.members.iter() {
            self.push_if_newer(record, &digest, &mut updates);
        }
        updates
    }

    fn push_if_newer(
        &self,
        record: &PeerRecord,
        digest: &BTreeMap<PeerId, (u64, u64, u8)>,
        updates: &mut Vec<PeerRecord>,
    ) {
        match digest.get(&record.id) {
            None => updates.push(record.clone()),
            Some(&(inc, hb, rank)) => {
                let mine = (record.incarnation, record.state.rank(), record.heartbeat);
                let theirs = (inc, rank, hb);
                if mine > theirs {
                    updates.push(record.clone());
                }
            }
        }
    }

    // ----- helpers ---------------------------------------------------------

    fn broadcast(&mut self, message: SwimMessage, out: &mut Vec<SwimOutbound>) {
        let targets = self.random_members_excluding_none(self.cfg.gossip_fanout);
        for target in targets {
            out.push(SwimOutbound::new(target, message.clone()));
        }
    }

    fn random_alive_member(&mut self) -> Option<PeerId> {
        let candidates = self.members.alive_ids();
        if candidates.is_empty() {
            return None;
        }
        let idx = (self.rng_next() as usize) % candidates.len();
        Some(candidates[idx])
    }

    fn random_members_excluding(&mut self, exclude: PeerId, k: usize) -> Vec<PeerId> {
        let mut candidates: Vec<PeerId> = self
            .members
            .alive_ids()
            .into_iter()
            .filter(|&id| id != exclude)
            .collect();
        self.take_random(&mut candidates, k)
    }

    fn random_members_excluding_none(&mut self, k: usize) -> Vec<PeerId> {
        let mut candidates: Vec<PeerId> = self.members.alive_ids();
        self.take_random(&mut candidates, k)
    }

    fn take_random(&mut self, candidates: &mut Vec<PeerId>, k: usize) -> Vec<PeerId> {
        let mut chosen = Vec::new();
        while !candidates.is_empty() && chosen.len() < k {
            let idx = (self.rng_next() as usize) % candidates.len();
            chosen.push(candidates.swap_remove(idx));
        }
        chosen
    }

    /// xorshift64* — deterministic, seeded, good enough for peer selection.
    fn rng_next(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}
