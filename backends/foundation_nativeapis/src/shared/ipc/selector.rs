/// Routing selectors for the IPC bus.
///
/// Selectors determine which endpoints receive a message based on
/// label expressions, delivery mode (unicast/multicast), and the payload's type UUID.

use foundation_core::type_uuid::Bytes;

use super::label::LabelOp;
use super::selector;

/// Delivery mode for a selector.
#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize, Eq, PartialEq)]
pub enum SelectorMode {
    /// The message can only be consumed by one endpoint.
    Unicast,
    /// The message can be consumed by multiple endpoints.
    Multicast,
}

/// Describes how a message is routed.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Selector {
    pub label_op: LabelOp,
    pub mode: SelectorMode,
    /// Type UUID of the payload — set automatically from `TypeUuid::UUID`.
    pub uuid: Bytes,
    /// Number of trailing objects that are `MemoryRegion`s (vs regular `Object`s).
    pub memory_region_count: u16,
    /// Time-to-live if unroutable. Zero means don't buffer.
    #[serde(with = "duration_serde")]
    pub ttl: std::time::Duration,
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
    pub fn unicast(label_op: impl Into<LabelOp>) -> Self {
        Self {
            label_op: label_op.into(),
            mode: SelectorMode::Unicast,
            uuid: [0; 16],
            memory_region_count: 0,
            ttl: std::time::Duration::ZERO,
        }
    }

    pub fn multicast(label_op: impl Into<LabelOp>) -> Self {
        Self {
            label_op: label_op.into(),
            mode: SelectorMode::Multicast,
            uuid: [0; 16],
            memory_region_count: 0,
            ttl: std::time::Duration::ZERO,
        }
    }

    /// Set a TTL for this message.
    pub fn ttl(mut self, ttl: std::time::Duration) -> Self {
        self.ttl = ttl;
        self
    }

    /// Check if this selector matches a given endpoint label.
    pub fn validate(&self, label: &super::Label) -> bool {
        self.label_op.matches(&label.0)
    }
}

