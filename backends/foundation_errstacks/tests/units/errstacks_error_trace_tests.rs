//! Tests for `ErrorTrace<C>` construction, context-changing, and attachments.

use core::fmt;

use foundation_errstacks::{AttachmentKind, ErrorTrace, FrameKind};

/// A minimal error type used as a context in these tests.
#[derive(Debug)]
struct FileError;

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("file operation failed")
    }
}

impl core::error::Error for FileError {}

/// A second error type to exercise `change_context`.
#[derive(Debug)]
struct ParseError;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("parse error")
    }
}

impl core::error::Error for ParseError {}

#[test]
fn new_creates_trace_with_single_context_frame() {
    let trace: ErrorTrace<FileError> = ErrorTrace::new(FileError);

    // The current context is retrievable and matches.
    let ctx = trace.current_context();
    assert_eq!(ctx.to_string(), "file operation failed");

    // Exactly one context frame is present initially.
    let context_frames = trace
        .frames()
        .filter(|f| matches!(f.kind(), FrameKind::Context(_)))
        .count();
    assert_eq!(
        context_frames, 1,
        "expected exactly one context frame after ErrorTrace::new"
    );
}

#[test]
fn change_context_transforms_type_and_adds_frame() {
    let trace: ErrorTrace<FileError> = ErrorTrace::new(FileError);
    let trace: ErrorTrace<ParseError> = trace.change_context(ParseError);

    // Current context is now the new type.
    assert_eq!(trace.current_context().to_string(), "parse error");

    // Both context frames are preserved in the trace.
    let context_count = trace
        .frames()
        .filter(|f| matches!(f.kind(), FrameKind::Context(_)))
        .count();
    assert_eq!(
        context_count, 2,
        "change_context should preserve the prior context frame"
    );
}

#[test]
fn attach_adds_printable_attachment_frame() {
    let trace: ErrorTrace<FileError> = ErrorTrace::new(FileError).attach("path=/etc/config.toml");

    let printable_attachments = trace
        .frames()
        .filter(|f| {
            matches!(
                f.kind(),
                FrameKind::Attachment(AttachmentKind::Printable(_))
            )
        })
        .count();
    assert_eq!(
        printable_attachments, 1,
        "attach() should add exactly one printable attachment frame"
    );
}

#[test]
fn attach_opaque_adds_opaque_attachment_frame_and_downcasts() {
    #[derive(Debug, PartialEq, Eq)]
    struct RequestId(u64);

    let trace: ErrorTrace<FileError> = ErrorTrace::new(FileError).attach_opaque(RequestId(42));

    // Opaque frame is present.
    let opaque_attachments = trace
        .frames()
        .filter(|f| matches!(f.kind(), FrameKind::Attachment(AttachmentKind::Opaque(_))))
        .count();
    assert_eq!(opaque_attachments, 1);

    // And can be recovered via downcast_ref.
    let recovered = trace.downcast_ref::<RequestId>();
    assert_eq!(recovered, Some(&RequestId(42)));

    // contains() returns true for a type present in the trace.
    assert!(trace.contains::<RequestId>());
    // And false for a type that is not.
    assert!(!trace.contains::<u8>());
}

#[test]
#[cfg(feature = "backtrace")]
fn backtrace_is_captured_on_new() {
    let trace: ErrorTrace<FileError> = ErrorTrace::new(FileError);

    // The alternate Display format should include the backtrace.
    let full_trace = format!("{trace:#}");
    assert!(
        full_trace.contains("Backtrace:"),
        "alternate Display should show backtrace header"
    );
}
