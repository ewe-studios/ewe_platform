// Broadcast tests: pending-message caching, eager delivery, closed subscribers.

use ewe_channels::broadcast;

#[test]
fn broadcast_should_cache_pending_messages_when_no_subscribers() {
    let mut broadcaster = broadcast::create::<String>(5);

    broadcaster.broadcast(String::from("first"));
    broadcaster.broadcast(String::from("second"));

    assert!(broadcaster.has_pending_messages());
}

#[test]
fn broadcast_should_cache_pending_messages_until_atleast_one_subscriber_joins() {
    let mut broadcaster = broadcast::create::<String>(5);

    broadcaster.broadcast(String::from("first"));
    broadcaster.broadcast(String::from("second"));

    assert!(broadcaster.has_pending_messages());

    let mut subscriber = broadcaster.subscribe();
    assert!(!subscriber.is_empty().unwrap());

    assert!(!broadcaster.has_pending_messages());
}

#[test]
fn broadcast_should_immediately_all_pending_messages_to_subscribers() {
    let mut broadcaster = broadcast::create::<String>(5);

    let mut subscriber = broadcaster.subscribe();
    assert!(subscriber.is_empty().unwrap());

    broadcaster.broadcast(String::from("first"));
    broadcaster.broadcast(String::from("second"));

    assert!(!broadcaster.has_pending_messages());
    assert!(!subscriber.is_empty().unwrap());
}

#[test]
fn broadcast_new_subscribers_should_miss_already_sent_messages() {
    let mut broadcaster = broadcast::create::<String>(5);

    let mut subscriber = broadcaster.subscribe();

    broadcaster.broadcast(String::from("first"));
    broadcaster.broadcast(String::from("second"));

    assert!(!broadcaster.has_pending_messages());
    assert!(!subscriber.is_empty().unwrap());

    let mut subscriber2 = broadcaster.subscribe();
    assert!(subscriber2.is_empty().unwrap());
}

#[test]
fn broadcast_new_subscribers_should_miss_already_sent_messages_but_get_new_messages() {
    let mut broadcaster = broadcast::create::<String>(5);

    let mut subscriber = broadcaster.subscribe();

    broadcaster.broadcast(String::from("first"));

    assert!(!broadcaster.has_pending_messages());
    assert!(!subscriber.is_empty().unwrap());

    let mut subscriber2 = broadcaster.subscribe();

    broadcaster.broadcast(String::from("second"));

    assert!(!subscriber2.is_empty().unwrap());
}

#[test]
fn broadcast_closed_subscriber_still_lets_others_get_message() {
    let mut broadcaster = broadcast::create::<String>(5);

    let mut subscriber = broadcaster.subscribe();
    let mut subscriber2 = broadcaster.subscribe();

    subscriber.close();

    broadcaster.broadcast(String::from("first"));
    broadcaster.broadcast(String::from("second"));

    assert!(!subscriber2.is_empty().unwrap());
    assert!(subscriber.is_empty().is_err());
}
