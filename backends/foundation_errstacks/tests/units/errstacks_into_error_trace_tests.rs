//! Tests for the [`IntoErrorTrace`] trait.
//!
//! **WHY:** `IntoErrorTrace` is the conversion trait used by the `bail!`
//! macro and other points where arbitrary errors become `ErrorTrace` values.
//!
//! **WHAT:** Tests verify that `IntoErrorTrace::into_error_trace` correctly
//! wraps errors and that the identity impl for `ErrorTrace` works.
//!
//! **HOW:** Create various error types, convert them, and assert the
//! resulting `ErrorTrace` has the expected structure.

#![cfg(feature = "to_structured")]

use core::fmt;

use foundation_errstacks::{ErrorTrace, IntoErrorTrace};

/// A simple test error type.
#[derive(Debug, PartialEq, Eq)]
struct SimpleError(&'static str);

impl fmt::Display for SimpleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SimpleError: {}", self.0)
    }
}

impl core::error::Error for SimpleError {}

/// A more complex error type with multiple fields.
#[derive(Debug, PartialEq, Eq)]
struct ComplexError {
    code: u16,
    message: &'static str,
}

impl fmt::Display for ComplexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}

impl core::error::Error for ComplexError {}

// --- IntoErrorTrace impl for E: Error ----------------------------------------

#[test]
fn into_error_trace_wraps_simple_error() {
    let err = SimpleError("something went wrong");
    let trace: ErrorTrace<SimpleError> = err.into_error_trace();

    assert_eq!(
        trace.current_context(),
        &SimpleError("something went wrong")
    );
    assert_eq!(trace.frames().count(), 1, "should have exactly one frame");
}

#[test]
fn into_error_trace_wraps_complex_error() {
    let err = ComplexError {
        code: 404,
        message: "not found",
    };
    let trace: ErrorTrace<ComplexError> = err.into_error_trace();

    assert_eq!(
        trace.current_context(),
        &ComplexError {
            code: 404,
            message: "not found"
        }
    );
}

#[test]
fn into_error_trace_result_has_single_context_frame() {
    let err = SimpleError("test");
    let trace: ErrorTrace<SimpleError> = err.into_error_trace();

    let context_frames = trace
        .frames()
        .filter(|f| matches!(f.kind(), foundation_errstacks::FrameKind::Context(_)))
        .count();

    assert_eq!(context_frames, 1);
}

// --- Display and Error impls for ErrorTrace ----------------------------------

#[test]
fn display_impl_shows_current_context() {
    let trace = ErrorTrace::new(SimpleError("visible"));
    let display = format!("{}", trace);

    assert!(display.contains("visible"));
}

#[test]
fn display_on_enriched_trace_shows_top_context() {
    let trace = ErrorTrace::new(SimpleError("inner")).change_context(ComplexError {
        code: 500,
        message: "outer",
    });
    let display = format!("{}", trace);

    assert!(display.contains("500"));
    assert!(display.contains("outer"));
}

#[test]
fn error_impl_source_is_none_for_phase1() {
    // Phase 1: source() returns None; full chain iteration is via frames().
    let trace = ErrorTrace::new(SimpleError("test"));
    assert!(core::error::Error::source(&trace).is_none());
}

// --- From<ErrorTrace> for Box<dyn Error> -------------------------------------

#[test]
fn from_error_trace_for_box_error() {
    let trace = ErrorTrace::new(SimpleError("boxed"));
    let boxed: Box<dyn core::error::Error + Send + Sync> = trace.into();

    // The boxed error should display correctly.
    let display = format!("{}", boxed);
    assert!(display.contains("boxed"));
}

// --- to_structured() tests (Task 3.2) ----------------------------------------

#[test]
fn to_structured_produces_correct_frame_count() {
    let trace = ErrorTrace::new(SimpleError("base"))
        .attach("path=/test")
        .change_context(ComplexError {
            code: 500,
            message: "server error",
        });

    let structured = trace.to_structured();

    assert_eq!(structured.current_context, "Error 500: server error");
    assert_eq!(structured.frames.len(), 3);

    // First frame: original context
    assert_eq!(structured.frames[0].kind, "context");
    assert!(structured.frames[0].message.contains("base"));

    // Second frame: attachment
    assert_eq!(structured.frames[1].kind, "printable");
    assert_eq!(structured.frames[1].message, "path=/test");

    // Third frame: new context
    assert_eq!(structured.frames[2].kind, "context");
    assert!(structured.frames[2].message.contains("500"));
    assert!(structured.frames[2].message.contains("server error"));
}

#[cfg(feature = "serde")]
#[test]
fn to_structured_json_serialization() {
    let trace = ErrorTrace::new(SimpleError("test")).attach("key=value");

    let structured = trace.to_structured();
    let json = structured.to_json().expect("should serialize to JSON");

    assert!(json.contains("test"));
    assert!(json.contains("key=value"));
    assert!(json.contains("current_context"));
    assert!(json.contains("frames"));
}

#[cfg(feature = "slack")]
#[test]
fn to_slack_blocks_produces_correct_structure() {
    let trace = ErrorTrace::new(SimpleError("connection failed")).attach("host=db.example.com");

    let structured = trace.to_structured();
    let slack_blocks = structured.to_slack_blocks();

    // Should have: header section, divider, fields section
    assert!(slack_blocks.blocks.len() >= 3);

    // First block is the header section
    assert_eq!(slack_blocks.blocks[0].block_type, "section");
    let header_text = slack_blocks.blocks[0].text.as_ref().unwrap();
    assert!(header_text.text.contains("connection failed"));

    // Second block is divider
    assert_eq!(slack_blocks.blocks[1].block_type, "divider");
}

#[cfg(all(feature = "slack", feature = "serde"))]
#[test]
fn to_slack_json_produces_valid_json() {
    let trace = ErrorTrace::new(SimpleError("test error"));

    let structured = trace.to_structured();
    let json = structured
        .to_slack_json()
        .expect("should serialize to JSON");

    assert!(json.contains("\"blocks\""));
    assert!(json.contains("\"type\""));
    assert!(json.contains("test error"));
}
