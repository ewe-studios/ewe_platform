//! Test for [`Clock::wall_now`] on the real [`MonotonicClock`].
//!
//! HOW: the test compares the real clock with `SystemTime::now`. This is the one real
//! read in this file.

use foundation_compact::SystemTime;
use foundation_context::{Clock, MonotonicClock};

#[test]
fn monotonic_clock_wall_now_is_near_real_time() {
    let clock = MonotonicClock::new();
    let start = SystemTime::now();
    let wall = clock.wall_now();
    let end = SystemTime::now();

    assert!(
        wall >= start && wall <= end,
        "the real clock's wall time must fall between the two reads around it"
    );
}
