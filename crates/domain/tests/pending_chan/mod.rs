// Pending-channels registry tests.

use ewe_channels::mspc;
use ewe_domain::{domains, pending_chan};

#[test]
fn pending_channels_can_clear_all_registered_channels() {
    let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

    let target_id = domains::Id(String::from("server_1"));

    let grp = registry.register(target_id.clone());

    assert!(registry.has(&target_id));

    registry.clear();

    assert!(!registry.has(&target_id));

    drop(grp);
}

#[test]
fn pending_channels_registry_should_be_able_to_register_request_id() {
    let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

    let target_id = domains::Id(String::from("server_1"));

    _ = registry.register(target_id.clone());

    assert!(registry.has(&target_id));
}

#[test]
fn pending_channels_registry_should_be_able_to_retrieve_channel_grp() {
    let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

    let target_id = domains::Id(String::from("server_1"));

    _ = registry.register(target_id.clone());

    assert!(registry.has(&target_id));

    assert!(registry.retrieve(&target_id).is_some());
}

#[test]
fn pending_channels_registry_should_be_able_to_resolve_a_pending_grp() {
    let mut registry = pending_chan::PendingChannelsRegistry::<String>::new();

    let target_id = domains::Id(String::from("server_1"));

    let grp = registry.register(target_id.clone());

    assert!(registry.has(&target_id));

    let result = registry
        .resolve(target_id.clone())
        .unwrap()
        .block_send(String::from("server_2"));

    assert!(matches!(result, mspc::ChannelResult::Ok(())));

    drop(grp);

    assert!(!registry.has(&target_id));
}
