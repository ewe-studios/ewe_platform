//! Tests for the cache and collapsing-cursor helpers.

use foundation_core::durable::cache::{etag, generate_cursor, CachePolicy, CursorConfig};

#[test]
fn given_a_read_range_when_it_closes_then_the_etag_changes() {
    let open = etag("events/a", 0, 10, false);
    let closed = etag("events/a", 0, 10, true);

    assert_ne!(open, closed);
    assert_eq!(open, etag("events/a", 0, 10, false));
    assert_eq!(closed, "\"events/a:0:10:c\"");
}

#[test]
fn given_a_cache_policy_when_it_renders_then_the_value_matches_the_protocol() {
    assert_eq!(
        CachePolicy::Public.header_value(),
        "public, max-age=60, stale-while-revalidate=300"
    );
    assert_eq!(CachePolicy::NoStore.header_value(), "no-store");
    assert_eq!(CachePolicy::NoCache.header_value(), "no-cache");
}

#[test]
fn given_a_cursor_when_it_collides_then_it_is_strictly_greater() {
    let config = CursorConfig::default();
    let now = config.epoch_unix_secs + 20 * 100;

    let first = generate_cursor(None, now, &config, 0);
    assert_eq!(first, 100);

    let next = generate_cursor(Some(first), now, &config, 30);
    assert!(next > first);

    let ahead = generate_cursor(Some(200), now, &config, 1);
    assert!(ahead > 200);
}
