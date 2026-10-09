//! Tests for `http2::flow_control` — window arithmetic, WINDOW_UPDATE thresholds,
//! SETTINGS window-change semantics (Feature 29).
//!
//! WHY: Flow control is the foundation for HTTP/2 backpressure — every window
//! operation must be correct at the boundary (overflow/underflow/negative).

use foundation_netio::http2::flow_control::*;

const INITIAL_WINDOW: WindowSize = 65_535;

fn fc_with(sz: WindowSize) -> FlowControl {
    let mut fc = FlowControl::new();
    fc.inc_window(sz).unwrap();
    fc.assign_capacity(sz).unwrap();
    fc
}

// ── Window arithmetic ──────────────────────────────────────────────────────

#[test]
fn window_new_is_zero() {
    let w = Window::new();
    assert_eq!(w.as_size(), 0);
    assert_eq!(w.checked_size(), 0);
}

#[test]
fn window_increase_decrease_round_trip() {
    let mut w = Window::new();
    w.increase_by(100).unwrap();
    assert_eq!(w.as_size(), 100);
    w.decrease_by(30).unwrap();
    assert_eq!(w.as_size(), 70);
}

#[test]
fn window_decrease_past_zero_goes_negative() {
    let mut w = Window::new();
    w.increase_by(50).unwrap();
    w.decrease_by(100).unwrap();
    // as_size clamps to 0
    assert_eq!(w.as_size(), 0);
}

#[test]
#[should_panic(expected = "negative Window")]
fn checked_size_panics_on_negative() {
    let mut w = Window::new();
    w.decrease_by(1).unwrap();
    let _ = w.checked_size();
}

#[test]
fn window_overflow_is_error() {
    let mut w = Window::new();
    w.increase_by(MAX_WINDOW_SIZE).unwrap();
    assert!(w.increase_by(1).is_err());
}

#[test]
fn window_underflow_is_error() {
    let mut w = Window::new();
    w.increase_by(10).unwrap();
    assert!(w.decrease_by(11).is_ok()); // negative is allowed (goes to -1)
    w.decrease_by(MAX_WINDOW_SIZE).unwrap(); // -1 - MAX = i32::MIN
    assert!(w.decrease_by(1).is_err()); // overflow i32::MIN
}

// ── FlowControl basics ─────────────────────────────────────────────────────

#[test]
fn new_flow_control_is_zero() {
    let fc = FlowControl::new();
    assert_eq!(fc.window_size(), 0);
    assert_eq!(fc.available().as_size(), 0);
}

#[test]
fn inc_window_grows_peer_side() {
    let mut fc = FlowControl::new();
    fc.inc_window(INITIAL_WINDOW).unwrap();
    assert_eq!(fc.window_size(), INITIAL_WINDOW);
    assert_eq!(fc.available().as_size(), 0);
}

#[test]
fn assign_capacity_grows_available() {
    let fc = fc_with(INITIAL_WINDOW);
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW);
}

#[test]
fn claim_capacity_reduces_available() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.claim_capacity(1000).unwrap();
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW - 1000);
    assert_eq!(fc.window_size(), INITIAL_WINDOW);
}

#[test]
fn claim_capacity_can_go_negative() {
    let mut fc = fc_with(100);
    assert!(fc.claim_capacity(101).is_ok());
}

#[test]
fn send_data_reduces_both_windows() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(500).unwrap();
    assert_eq!(fc.window_size(), INITIAL_WINDOW - 500);
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW - 500);
}

#[test]
fn send_data_zero_is_noop() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(0).unwrap();
    assert_eq!(fc.window_size(), INITIAL_WINDOW);
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW);
}

#[test]
#[should_panic]
fn send_data_exceeding_window_panics() {
    let mut fc = fc_with(100);
    let _ = fc.send_data(101);
}

// ── WINDOW_UPDATE threshold ────────────────────────────────────────────────

#[test]
fn unclaimed_none_when_no_capacity_returned() {
    let fc = fc_with(INITIAL_WINDOW);
    assert_eq!(fc.unclaimed_capacity(), None);
}

#[test]
fn unclaimed_none_below_threshold() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(1000).unwrap();
    fc.assign_capacity(500).unwrap();
    assert_eq!(fc.unclaimed_capacity(), None);
}

#[test]
fn unclaimed_some_above_threshold() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(INITIAL_WINDOW).unwrap();
    fc.assign_capacity(INITIAL_WINDOW / 2).unwrap();
    assert!(fc.unclaimed_capacity().is_some());
}

#[test]
fn unclaimed_returns_full_increment() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(INITIAL_WINDOW).unwrap();
    fc.assign_capacity(INITIAL_WINDOW).unwrap();
    assert_eq!(fc.unclaimed_capacity(), Some(INITIAL_WINDOW));
}

// ── SETTINGS window changes ────────────────────────────────────────────────

#[test]
fn dec_send_window_reduces_peer_side() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.dec_send_window(1000).unwrap();
    assert_eq!(fc.window_size(), INITIAL_WINDOW - 1000);
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW);
}

#[test]
fn dec_recv_window_reduces_both() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.dec_recv_window(1000).unwrap();
    assert_eq!(fc.window_size(), INITIAL_WINDOW - 1000);
    assert_eq!(fc.available().as_size(), INITIAL_WINDOW - 1000);
}

#[test]
fn inc_window_overflow_is_error() {
    let mut fc = fc_with(MAX_WINDOW_SIZE);
    assert!(fc.inc_window(1).is_err());
}

#[test]
fn inc_window_exceeding_max_is_error() {
    let mut fc = fc_with(MAX_WINDOW_SIZE - 10);
    assert!(fc.inc_window(20).is_err());
}

// ── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn window_can_go_negative_from_settings_change() {
    let mut fc = fc_with(INITIAL_WINDOW);
    fc.send_data(32_768).unwrap();
    fc.dec_send_window(49_151).unwrap();
    assert_eq!(fc.window_size(), 0); // clamped
}

#[test]
fn has_unavailable_false_when_window_negative() {
    let mut fc = fc_with(1000);
    fc.send_data(1000).unwrap();
    fc.dec_send_window(1).unwrap();
    assert!(!fc.has_unavailable());
}

#[test]
fn property_window_size_never_exceeds_max() {
    let mut fc = FlowControl::new();
    fc.inc_window(MAX_WINDOW_SIZE).unwrap();
    assert!(fc.window_size() <= MAX_WINDOW_SIZE);
    assert!(fc.inc_window(1).is_err());
}

#[test]
fn claim_zero_is_noop() {
    let mut fc = fc_with(100);
    fc.claim_capacity(0).unwrap();
    assert_eq!(fc.available().as_size(), 100);
}
