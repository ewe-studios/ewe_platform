use core::fmt;
use std::collections::BTreeMap;

use foundation_compact::crypto::chacha::ChaChaKey;
use zeroize::Zeroize;

use super::StateError;

/// Holds a key identifier and a 32-byte secret. The secret clears on drop.
pub struct StateKey {
    id: u32,
    pub(super) secret: ChaChaKey,
}

impl StateKey {
    /// Makes a key with a positive identifier and a host-supplied secret.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is zero.
    pub fn new(id: u32, mut secret: [u8; 32]) -> Result<Self, StateError> {
        let key = Self {
            id,
            secret: ChaChaKey::from_bytes(secret),
        };
        secret.zeroize();
        if id == 0 {
            return Err(StateError::BadKey);
        }
        Ok(key)
    }

    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }
}

impl fmt::Debug for StateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StateKey")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Holds the key for new states and the prior keys that can open old states.
pub struct StateKeyRing {
    pub(super) current: StateKey,
    previous: BTreeMap<u32, StateKey>,
}

impl StateKeyRing {
    #[must_use]
    pub fn new(current: StateKey) -> Self {
        Self {
            current,
            previous: BTreeMap::new(),
        }
    }

    /// Adds a key that can open previously sealed states.
    ///
    /// # Errors
    ///
    /// Returns an error if the identifier is already in the ring.
    pub fn with_previous(mut self, key: StateKey) -> Result<Self, StateError> {
        if self.find(key.id()).is_some() {
            return Err(StateError::BadKey);
        }
        self.previous.insert(key.id(), key);
        Ok(self)
    }

    #[must_use]
    pub fn current_id(&self) -> u32 {
        self.current.id()
    }

    pub(super) fn find(&self, id: u32) -> Option<&StateKey> {
        if self.current_id() == id {
            Some(&self.current)
        } else {
            self.previous.get(&id)
        }
    }
}
