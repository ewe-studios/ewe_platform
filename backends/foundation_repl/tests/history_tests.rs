//! History tests moved out of `src/shared/history.rs` (spec 70 feature 59,
//! rule 7). The test only uses the public `ReplHistory` API.

use foundation_repl::ReplHistory;

#[test]
fn push_and_recall() {
    let mut h = ReplHistory::new(100);
    h.push("hello".into());
    h.push("world".into());
    assert_eq!(h.up(), Some("world"));
    assert_eq!(h.up(), Some("hello"));
    assert_eq!(h.up(), Some("hello")); // stuck at bottom
}

#[test]
fn down_returns_to_bottom() {
    let mut h = ReplHistory::new(100);
    h.push("a".into());
    h.push("b".into());
    h.up();
    h.up();
    assert_eq!(h.down(), Some("b"));
    assert_eq!(h.down(), None);
}

#[test]
fn empty_not_stored() {
    let mut h = ReplHistory::new(100);
    h.push("   ".into());
    assert_eq!(h.up(), None);
}

#[test]
fn duplicate_moves_to_end() {
    let mut h = ReplHistory::new(100);
    h.push("x".into());
    h.push("y".into());
    h.push("x".into());
    assert_eq!(h.up(), Some("x"));
    assert_eq!(h.up(), Some("y"));
    assert_eq!(h.up(), Some("y")); // x was removed from earlier position
}
