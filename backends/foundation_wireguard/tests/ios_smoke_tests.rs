//! iOS FFI smoke tests (moved from `src/ios/mod.rs`).
#![cfg(target_os = "ios")]

use foundation_wireguard::ios::{
    wg_node_create_from_toml, wg_node_destroy, wg_node_free_string, wg_node_generate_seed,
    wg_node_stop, wg_node_tick,
};

#[test]
fn smoke_create_null_config() {
    let h = wg_node_create_from_toml(std::ptr::null());
    assert!(h.is_null(), "null config should return null handle");
}

#[test]
fn smoke_generate_seed() {
    let s = wg_node_generate_seed();
    assert!(!s.is_null(), "seed generation should return a string");
    wg_node_free_string(s);
}

#[test]
fn smoke_tick_null() {
    let ms = wg_node_tick(std::ptr::null_mut());
    assert!(ms > 0, "tick should return a positive poll interval");
}

#[test]
fn smoke_destroy_null() {
    wg_node_destroy(std::ptr::null_mut());
    // Must not crash on null.
}

#[test]
fn smoke_stop_null() {
    assert_eq!(wg_node_stop(std::ptr::null_mut()), 0);
}
