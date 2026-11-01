use std::{collections, sync};

use thiserror::Error;

use ewe_channels::mspc::{self};

use crate::domains;

#[derive(Error, Debug)]
pub enum PendingChannelError {
    #[error("Failed to find any pending channels registered with {0}")]
    NotFound(String),

    #[error("Corresponding channel sender for {0} was closed")]
    ClosedSender(domains::Id),
}

pub type PendingChannelResult<E> = std::result::Result<E, PendingChannelError>;

/// `PendingChannelsRegistry` provide a way to register a target channel
/// which is to carry provides a temporary storage of a [`ChannelGroup`]
/// to which is both provided to the caller but also stored in a key-value
/// hashmap that stores the relevant [`ChannelGroup`] to a giving target [`domains::Id`]
/// which then allows the channel to receive a response later on.
///
/// These channels are one-time use and generally exists to allow a request-response
/// via channels symantic behaviour. Once the response is received, then the channel should
/// be closed. This means whatever underlying response they carry should clearly know how to
/// communicate a stream.
pub struct PendingChannelsRegistry<E> {
    pending: sync::Arc<sync::Mutex<collections::HashMap<domains::Id, mspc::ChannelGroup<E>>>>,
}

impl<E> Clone for PendingChannelsRegistry<E> {
    fn clone(&self) -> Self {
        Self {
            pending: self.pending.clone(),
        }
    }
}

impl<E> Default for PendingChannelsRegistry<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> PendingChannelsRegistry<E> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: sync::Arc::new(sync::Mutex::new(collections::HashMap::new())),
        }
    }

    /// Checks if a pending channel exists for the given ID.
    ///
    /// # Panics
    ///
    /// Panics if the pending registry lock is poisoned.
    pub fn has(&mut self, id: &domains::Id) -> bool {
        let registry = self.pending.lock().unwrap();
        registry.contains_key(id)
    }

    /// Retrieves a pending channel group for the given ID.
    ///
    /// # Panics
    ///
    /// Panics if the pending registry lock is poisoned.
    pub fn retrieve(&mut self, id: &domains::Id) -> Option<mspc::ChannelGroup<E>> {
        let registry = self.pending.lock().unwrap();
        if let Some(grp) = registry.get(id) {
            return Some(grp.clone());
        }
        None
    }

    /// Registers a new pending channel for the given ID.
    ///
    /// # Panics
    ///
    /// Panics if the pending registry lock is poisoned.
    pub fn register(&mut self, id: domains::Id) -> mspc::ChannelGroup<E> {
        let group_channel = mspc::ChannelGroup::new();

        let mut registry = self.pending.lock().unwrap();
        registry.insert(id, group_channel.clone());

        group_channel
    }

    /// Resolves and removes a pending channel for the given ID.
    ///
    /// # Errors
    ///
    /// Returns `PendingChannelError::NotFound` if no channel exists for the ID.
    /// Returns `PendingChannelError::ClosedSender` if the sender has been closed.
    ///
    /// # Panics
    ///
    /// Panics if the pending registry lock is poisoned.
    pub fn resolve(&mut self, id: domains::Id) -> PendingChannelResult<mspc::SendChannel<E>> {
        let mut registry = self.pending.lock().unwrap();
        if !registry.contains_key(&id) {
            return PendingChannelResult::Err(PendingChannelError::NotFound(id.0.clone()));
        }

        if let Some((_, entry)) = registry.remove_entry(&id) {
            if let Some(sender) = entry.0 {
                return PendingChannelResult::Ok(sender);
            }
        }

        PendingChannelResult::Err(PendingChannelError::ClosedSender(id))
    }

    /// Clears all pending channels.
    ///
    /// # Panics
    ///
    /// Panics if the pending registry lock is poisoned.
    pub fn clear(&mut self) {
        let mut registry = self.pending.lock().unwrap();
        for (_, mut entry) in registry.drain() {
            if let Some(mut sender) = entry.0.take() {
                sender.close().expect("close sender");
            }
            _ = entry.1.take();
        }
    }
}
