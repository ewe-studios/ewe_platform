//! Tests for `multi::get_allocatable_thread_count`.

#![cfg(feature = "multi")]

use foundation_core::valtron::multi::{get_allocatable_thread_count, get_max_threads};
use serial_test::serial;
use std::env;

#[test]
#[serial]
fn get_allocatable_thread_count_as_far_as_1_remains() {
    let max_threads = get_max_threads();
    assert!(max_threads > 3);

    env::remove_var("VALTRON_NUM_THREADS");

    let new_thread_count = max_threads - 2;
    tracing::debug!("Setting thread count to: {new_thread_count:}");

    let new_thread_count_str = format!("{new_thread_count:}");
    env::set_var("VALTRON_NUM_THREADS", new_thread_count_str.clone());
    assert_eq!(
        env::var("VALTRON_NUM_THREADS").unwrap(),
        new_thread_count_str
    );

    // Very flaky, for now asset non-zero
    // assert!(get_allocatable_thread_count() != 0);
    //
    assert_eq!(get_allocatable_thread_count(), max_threads - 2);

    // Don't leak the override into the other pool tests in this binary.
    env::remove_var("VALTRON_NUM_THREADS");
}
