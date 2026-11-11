//! Tests for [`foundation_compact::time`]: UNIX timestamp conversions.
//!
//! These tests check the two directions of the millisecond conversion, the
//! epoch, and the negative (pre-epoch) case.

use foundation_compact::time::{from_unix_millis, unix_millis, unix_seconds};
use foundation_compact::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn the_unix_epoch_is_zero() {
    assert_eq!(unix_millis(UNIX_EPOCH), 0);
    assert_eq!(unix_seconds(UNIX_EPOCH), 0);
    assert_eq!(from_unix_millis(0), UNIX_EPOCH);
}

#[test]
fn unix_millis_round_trips_through_from_unix_millis() {
    for millis in [0, 1, 999, 1_700_000_000_000, -1, -999, -1_700_000_000_000] {
        let time = from_unix_millis(millis);
        assert_eq!(unix_millis(time), millis, "round trip of {millis} ms");
    }
}

#[test]
fn a_time_before_the_epoch_is_negative() {
    let before = UNIX_EPOCH - Duration::from_millis(1_500);

    assert_eq!(unix_millis(before), -1_500);
    assert!(unix_seconds(before) < 0);
    assert_eq!(from_unix_millis(-1_500), before);
}

#[test]
fn unix_seconds_truncates_a_positive_time() {
    let time = UNIX_EPOCH + Duration::from_millis(1_999);

    assert_eq!(unix_seconds(time), 1);
}

#[test]
fn unix_seconds_rounds_a_pre_epoch_time_down() {
    // Half a second before the epoch is -1, not 0: the result rounds down.
    let before = UNIX_EPOCH - Duration::from_millis(500);
    assert_eq!(unix_seconds(before), -1);

    // One whole second before the epoch is -1.
    let before = UNIX_EPOCH - Duration::from_secs(1);
    assert_eq!(unix_seconds(before), -1);
}

#[test]
fn unix_seconds_counts_whole_seconds() {
    let time: SystemTime = UNIX_EPOCH + Duration::from_secs(1_700_000_000);

    assert_eq!(unix_seconds(time), 1_700_000_000);
}
