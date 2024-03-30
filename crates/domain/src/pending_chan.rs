use std::{collections, sync};

use thiserror::Error;

use channels::mspc::{self};

use crate::domains;

#[derive(Error, Debug)]
pub enum PendingChannelError {
    #[error("Failed to find any pending channels registered with {0}")]
    NotFound(String),

    #[error("Corresponding channel sender for {0} was closed")]
    ClosedSender(domains::Id),
}

pub type PendingChannelResult<E> = std::result::Result<E, PendingChannelError>;

/// PendingChannelsRegistry provide a way to register a target channel
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

impl<E> PendingChannelsRegistry<E> {
    pub fn new() -> Self {
        Self {
            pending: sync::Arc::new(sync::Mutex::new(collections::HashMap::new())),
        }
    }

    pub fn has(&mut self, id: domains::Id) -> bool {
        let registry = self.pending.lock().unwrap();
        registry.contains_key(&id)
    }

    pub fn retrieve(&mut self, id: domains::Id) -> Option<mspc::ChannelGroup<E>> {
        let registry = self.pending.lock().unwrap();
        if let Some(grp) = registry.get(&id) {
            return Some(grp.clone());
        }
        None
    }

    pub fn register(&mut self, id: domains::Id) -> mspc::ChannelGroup<E> {
        let group_channel = mspc::ChannelGroup::new();

        let mut registry = self.pending.lock().unwrap();
        registry.insert(id, group_channel.clone());

        group_channel
    }

    pub fn resolve(&mut self, id: domains::Id) -> PendingChannelResult<mspc::SendChannel<E>> {
        let mut registry = self.pending.lock().unwrap();
        if !registry.contains_key(&id) {
            return PendingChannelResult::Err(PendingChannelError::NotFound(id.0.to_string()));
        }

        if let Some((_, entry)) = registry.remove_entry(&id) {
            if let Some(sender) = entry.0 {
                return PendingChannelResult::Ok(sender);
            }
        }

        PendingChannelResult::Err(PendingChannelError::ClosedSender(id))
    }

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

#[cfg(test)]
mod tests {
    use channels::mspc;

    use crate::{domains, pending_chan};

    #[test]
    fn pending_channels_can_clear_all_registered_channels() {
        let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

        let target_id = domains::Id(String::from("server_1"));

        let grp = registry.register(target_id.clone());

        assert!(registry.has(target_id.clone()));

        registry.clear();

        assert!(!registry.has(target_id));

        drop(grp)
    }

    #[test]
    fn pending_channels_registry_should_be_able_to_register_request_id() {
        let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

        let target_id = domains::Id(String::from("server_1"));

        _ = registry.register(target_id.clone());

        assert!(registry.has(target_id));
    }

    #[test]
    fn pending_channels_registry_should_be_able_to_retrieve_channel_grp() {
        let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

        let target_id = domains::Id(String::from("server_1"));

        _ = registry.register(target_id.clone());

        assert!(registry.has(target_id.clone()));

        assert!(registry.retrieve(target_id).is_some());
    }

    #[test]
    fn pending_channels_registry_should_be_able_to_resolve_a_pending_grp() {
        let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

        let target_id = domains::Id(String::from("server_1"));

        let grp = registry.register(target_id.clone());

        assert!(registry.has(target_id.clone()));

        let result = registry
            .resolve(target_id.clone())
            .unwrap()
            .block_send(String::from("server_2"));

        assert!(matches!(result, mspc::ChannelResult::Ok(())));

        drop(grp);

        assert!(!registry.has(target_id));
    }
}
