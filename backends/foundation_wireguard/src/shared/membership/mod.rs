//! Masterless SWIM full-membership gossip (spec-55, feature 03; decision 05).
//!
//! WHY: Peer discovery with no coordinator — every node learns every other node,
//! membership spreads epidemically, failures are detected, and the mesh survives the
//! seed's death.
//!
//! WHAT: A sans-I/O core: [`Membership`] (the CRDT-like set), [`PeerRecord`] (the gossiped
//! unit), [`SwimMessage`]/[`SwimOutbound`] (wire I/O), and [`Swim`] (the state machine
//! driven by timer ticks + inbound messages).
//!
//! HOW: Merges are commutative/idempotent so anti-entropy is safe anytime; the state
//! machine consumes `tick(now)` + `on_message(...)` and emits outbound messages +
//! [`SwimEvent`]s, keeping it fully unit-testable with a simulated clock and network.

pub mod message;
pub mod record;
pub mod swim;

pub use message::{decode, encode, DigestEntry, SwimMessage, SwimOutbound, SWIM_WIRE_VERSION};
pub use record::{Capabilities, MemberState, PeerId, PeerRecord};
pub use swim::{Swim, SwimConfig, SwimEvent};

use std::collections::BTreeMap;

/// The outcome of merging a single record into the membership set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MergeOutcome {
    /// The incoming record was older/equal and ignored.
    Unchanged,
    /// A brand-new member appeared.
    Inserted,
    /// An existing member was updated. Flags describe *what* observably changed.
    Updated {
        /// The member transitioned out of the `Dead` tombstone state (rejoin).
        was_dead: bool,
        /// The member's state became `Dead`.
        now_dead: bool,
        /// The member's endpoint set changed.
        endpoints_changed: bool,
        /// The member's capabilities changed.
        caps_changed: bool,
    },
}

/// The full-membership set: one [`PeerRecord`] per [`PeerId`], merged as a CRDT.
#[derive(Debug, Clone, Default)]
pub struct Membership {
    records: BTreeMap<PeerId, PeerRecord>,
}

impl Membership {
    /// An empty membership set.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
        }
    }

    /// Look up a member by id.
    #[must_use]
    pub fn get(&self, id: &PeerId) -> Option<&PeerRecord> {
        self.records.get(id)
    }

    /// Number of members currently tracked (including tombstoned `Dead` ones).
    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Iterate over every record.
    pub fn iter(&self) -> impl Iterator<Item = &PeerRecord> {
        self.records.values()
    }

    /// Every record, cloned — for anti-entropy responses and `PullMembership`.
    #[must_use]
    pub fn all(&self) -> Vec<PeerRecord> {
        self.records.values().cloned().collect()
    }

    /// The ids of all `Alive` members — probe/gossip candidates.
    #[must_use]
    pub fn alive_ids(&self) -> Vec<PeerId> {
        self.records
            .values()
            .filter(|r| r.state == MemberState::Alive)
            .map(|r| r.id)
            .collect()
    }

    /// WHY: The CRDT heart — apply one record, deterministically and idempotently.
    ///
    /// WHAT: Merge `incoming`, returning what observably changed.
    ///
    /// HOW: Inserts if new; otherwise replaces only if `incoming` supersedes the current
    /// record (see [`PeerRecord::supersedes`]), reporting the observable delta.
    pub(crate) fn merge_record(&mut self, incoming: PeerRecord) -> MergeOutcome {
        match self.records.get(&incoming.id) {
            None => {
                self.records.insert(incoming.id, incoming);
                MergeOutcome::Inserted
            }
            Some(existing) => {
                if !existing.supersedes(&incoming) {
                    return MergeOutcome::Unchanged;
                }
                let was_dead = existing.state == MemberState::Dead;
                let now_dead = incoming.state == MemberState::Dead;
                let endpoints_changed = existing.endpoints != incoming.endpoints;
                let caps_changed = existing.caps != incoming.caps;
                self.records.insert(incoming.id, incoming);
                MergeOutcome::Updated {
                    was_dead,
                    now_dead,
                    endpoints_changed,
                    caps_changed,
                }
            }
        }
    }

    /// Remove a tombstoned member outright (tombstone GC).
    pub(crate) fn remove(&mut self, id: &PeerId) {
        self.records.remove(id);
    }
}
