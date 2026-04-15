//! Tests for `Frame`, `FrameIter`, `FrameKind`, and `AttachmentKind`.

use core::fmt;

use foundation_errstacks::{AttachmentKind, ErrorTrace, FrameKind};

#[derive(Debug)]
struct DemoError;

impl fmt::Display for DemoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("demo error")
    }
}

impl core::error::Error for DemoError {}

#[test]
fn frame_iter_visits_frames_in_push_order() {
    let trace: ErrorTrace<DemoError> = ErrorTrace::new(DemoError)
        .attach("first-attachment")
        .attach("second-attachment");

    // We expect three frames: the initial Context plus two Printable
    // attachments, in the order they were added.
    let kinds: Vec<_> = trace
        .frames()
        .map(|f| match f.kind() {
            FrameKind::Context(_) => "context",
            FrameKind::Attachment(AttachmentKind::Printable(_)) => "printable",
            FrameKind::Attachment(AttachmentKind::Opaque(_)) => "opaque",
        })
        .collect();

    assert_eq!(kinds, vec!["context", "printable", "printable"]);
}

#[test]
fn frame_iter_terminates_on_empty_tail() {
    // Regression guard for the "never use `loop {}` in Iterator::next"
    // workspace rule: after walking the known frames the iterator must
    // return `None` cleanly, not block or spin.
    let trace: ErrorTrace<DemoError> = ErrorTrace::new(DemoError);
    let mut iter = trace.frames();

    // Walk to exhaustion.
    assert!(iter.next().is_some());
    assert!(iter.next().is_none());
    // And stays exhausted.
    assert!(iter.next().is_none());
}
