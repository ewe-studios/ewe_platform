//! Tests for the `PlainResultExt` and `ErrorTraceResultExt` extension traits.
//!
//! These tests cover both impl paths:
//! * `Result<T, E>` where `E: core::error::Error` — plain errors lifted into `ErrorTrace`
//! * `Result<T, ErrorTrace<C>>` — enriching existing traces

use core::fmt;

use foundation_errstacks::{
    AttachmentKind, ErrorTrace, ErrorTraceResultExt, FrameKind, PlainResultExt,
};

#[derive(Debug)]
struct IoError;

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("io error")
    }
}

impl core::error::Error for IoError {}

#[derive(Debug)]
struct DbError;

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("db error")
    }
}

impl core::error::Error for DbError {}

// --- Result<T, E: Error> path (PlainResultExt) ------------------------------

#[test]
fn attach_on_plain_error_result_lifts_into_error_trace() {
    let result: Result<(), IoError> = Err(IoError);
    let lifted: Result<(), ErrorTrace<IoError>> = result.attach("path=/tmp/x");

    let trace = lifted.unwrap_err();
    assert_eq!(trace.current_context().to_string(), "io error");

    let printable = trace
        .frames()
        .filter(|f| {
            matches!(
                f.kind(),
                FrameKind::Attachment(AttachmentKind::Printable(_))
            )
        })
        .count();
    assert_eq!(printable, 1);
}

#[test]
fn attach_on_ok_is_passthrough() {
    let result: Result<u32, IoError> = Ok(7);
    let lifted = result.attach("never runs");
    assert_eq!(lifted.unwrap(), 7);
}

#[test]
fn attach_with_is_lazy_on_ok() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    let result: Result<u32, IoError> = Ok(1);
    let _ = result.attach_with(|| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        "lazy"
    });
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn attach_with_runs_on_err() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    let result: Result<u32, IoError> = Err(IoError);
    let lifted = result.attach_with(|| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        "lazy"
    });
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);
    assert!(lifted.is_err());
}

#[test]
fn attach_opaque_on_plain_error_result_is_downcastable() {
    #[derive(Debug, PartialEq, Eq)]
    struct ReqId(u64);

    let result: Result<(), IoError> = Err(IoError);
    let lifted = result.attach_opaque(ReqId(9));
    let trace = lifted.unwrap_err();
    assert_eq!(trace.downcast_ref::<ReqId>(), Some(&ReqId(9)));
}

#[test]
fn attach_opaque_with_is_lazy_on_ok() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    let result: Result<u32, IoError> = Ok(1);
    let _ = result.attach_opaque_with(|| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        42_u64
    });
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn change_context_on_plain_error_result_retags_type() {
    let result: Result<(), IoError> = Err(IoError);
    let retagged: Result<(), ErrorTrace<DbError>> = result.change_context(DbError);

    let trace = retagged.unwrap_err();
    assert_eq!(trace.current_context().to_string(), "db error");

    // Both context frames are preserved.
    let contexts = trace
        .frames()
        .filter(|f| matches!(f.kind(), FrameKind::Context(_)))
        .count();
    assert_eq!(contexts, 2);
}

#[test]
fn change_context_with_is_lazy_on_ok() {
    use core::sync::atomic::{AtomicUsize, Ordering};
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    let result: Result<u32, IoError> = Ok(1);
    let _ = result.change_context_with(|| {
        CALLS.fetch_add(1, Ordering::SeqCst);
        DbError
    });
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
}

// --- Result<T, ErrorTrace<C>> path (ErrorTraceResultExt) ---------------------

#[test]
fn attach_on_existing_trace_result_appends_frame() {
    let existing: Result<(), ErrorTrace<IoError>> = Err(ErrorTrace::new(IoError));
    let enriched = ErrorTraceResultExt::attach(existing, "step=open");
    let trace = enriched.unwrap_err();

    let printable = trace
        .frames()
        .filter(|f| {
            matches!(
                f.kind(),
                FrameKind::Attachment(AttachmentKind::Printable(_))
            )
        })
        .count();
    assert_eq!(printable, 1);
}

#[test]
fn change_context_on_existing_trace_result_retags_and_preserves_frames() {
    let existing: Result<(), ErrorTrace<IoError>> =
        Err(ErrorTrace::new(IoError).attach("prior=attachment"));
    let retagged: Result<(), ErrorTrace<DbError>> =
        ErrorTraceResultExt::change_context(existing, DbError);
    let trace = retagged.unwrap_err();

    assert_eq!(trace.current_context().to_string(), "db error");

    // Two contexts + one prior printable = 3 frames total.
    assert_eq!(trace.frames().count(), 3);
}
