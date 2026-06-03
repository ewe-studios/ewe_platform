/// Routing selectors for the IPC bus.
///
/// Selectors determine which endpoints receive a message based on
/// label expressions, delivery mode (unicast/multicast), and TTL.

use std::time::Duration;

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

use super::label::LabelOp;

/// Delivery mode for a selector.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum SelectorMode {
    /// Delivers to the first matching endpoint.
    Unicast,
    /// Delivers to all matching endpoints.
    Multicast,
}

/// A routing selector that determines which endpoints receive a message.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Encode, Decode)]
pub struct Selector {
    /// Label expression for routing.
    pub label_op: LabelOp,
    /// Delivery mode.
    pub mode: SelectorMode,
    /// Time-to-live if unroutable. Zero means don't buffer.
    #[serde(with = "duration_serde")]
    #[bincode(with_serde)]
    pub ttl: Duration,
}

// serde helper for Duration
mod duration_serde {
    use std::time::Duration;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(val: &Duration, s: S) -> Result<S::Ok, S::Error> {
        val.as_millis().serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(d)?;
        Ok(Duration::from_millis(ms))
    }
}

impl Selector {
    /// Create a broadcast selector — sends to all endpoints.
    pub fn broadcast() -> Self {
        Self {
            label_op: LabelOp::True,
            mode: SelectorMode::Multicast,
            ttl: Duration::ZERO,
        }
    }

    /// Create a unicast selector — sends to a specific endpoint.
    pub fn unicast(label: impl Into<String>) -> Self {
        Self {
            label_op: LabelOp::Leaf(label.into()),
            mode: SelectorMode::Unicast,
            ttl: Duration::ZERO,
        }
    }

    /// Create a multicast selector — sends to all endpoints matching the label expression.
    pub fn multicast(label_op: LabelOp) -> Self {
        Self {
            label_op,
            mode: SelectorMode::Multicast,
            ttl: Duration::ZERO,
        }
    }

    /// Set a TTL for this message.
    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Check if this selector matches a given endpoint label.
    pub fn matches_label(&self, label: &str) -> bool {
        self.label_op.matches(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_matches_all() {
        let sel = Selector::broadcast();
        assert!(sel.mode == SelectorMode::Multicast);
        assert!(sel.matches_label("anything"));
    }

    #[test]
    fn unicast_matches_only_target() {
        let sel = Selector::unicast("target");
        assert!(sel.mode == SelectorMode::Unicast);
        assert!(sel.matches_label("target"));
        assert!(!sel.matches_label("other"));
    }
}
