/// Routing selectors for the IPC bus.
///
/// Selectors determine which endpoints receive a message based on
/// label expressions, delivery mode (unicast/multicast), and TTL.

use std::time::Duration;

use super::label::LabelOp;

/// Delivery mode for a selector.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum SelectorMode {
    /// Delivers to the first matching endpoint.
    Unicast,
    /// Delivers to all matching endpoints.
    Multicast,
}

/// A routing selector that determines which endpoints receive a message.
#[derive(Clone, PartialEq, Debug)]
pub struct Selector {
    /// Label expression for routing.
    pub label_op: LabelOp,
    /// Delivery mode.
    pub mode: SelectorMode,
    /// Time-to-live if unroutable. Zero means don't buffer.
    pub ttl: Duration,
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

    /// Set a TTL for this message — if unroutable, the controller buffers it
    /// for this duration and retries when new endpoints join.
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
        assert!(sel.matches_label("foo"));
    }

    #[test]
    fn unicast_matches_only_target() {
        let sel = Selector::unicast("target");
        assert!(sel.mode == SelectorMode::Unicast);
        assert!(sel.matches_label("target"));
        assert!(!sel.matches_label("other"));
    }

    #[test]
    fn multicast_with_or() {
        let sel = Selector::multicast(LabelOp::Or(
            Box::new(LabelOp::Leaf("a".into())),
            Box::new(LabelOp::Leaf("b".into())),
        ));
        assert!(sel.matches_label("a"));
        assert!(sel.matches_label("b"));
        assert!(!sel.matches_label("c"));
    }

    #[test]
    fn ttl() {
        let sel = Selector::broadcast().ttl(Duration::from_secs(10));
        assert_eq!(sel.ttl, Duration::from_secs(10));
    }
}
