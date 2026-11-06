//! Spec 70 feature 134 — the profile name reads the injected wall clock.
//!
//! The old `unique_profile_dir` read `SystemTime::now` directly. It now takes
//! `&dyn Clock` and converts `wall_now` with `foundation_compact::time`, so a
//! simulated clock controls the name.

use std::time::Duration;

use foundation_browser::supervisor::unique_profile_dir;
use foundation_core::valtron::SimulatedClock;

/// The `pid-<millis>-` segment a profile name must carry.
fn segment(millis: i64) -> String {
    format!("{}-{millis}-", std::process::id())
}

#[test]
fn unique_profile_dir_reads_the_injected_wall_clock() {
    let clock = SimulatedClock::new(Duration::ZERO);

    let first = unique_profile_dir(&clock);
    let first_name = first
        .file_name()
        .expect("the profile path has a file name")
        .to_string_lossy()
        .into_owned();
    assert!(
        first_name.contains(&segment(0)),
        "the name must carry the simulated wall millis 0: {first_name}"
    );

    clock.advance_wall(Duration::from_secs(1_700_000_000));
    let second = unique_profile_dir(&clock);
    let second_name = second
        .file_name()
        .expect("the profile path has a file name")
        .to_string_lossy()
        .into_owned();
    assert!(
        second_name.contains(&segment(1_700_000_000_000)),
        "the name must carry the advanced wall millis: {second_name}"
    );
    assert_ne!(
        first_name, second_name,
        "each call makes a distinct profile name"
    );
}
