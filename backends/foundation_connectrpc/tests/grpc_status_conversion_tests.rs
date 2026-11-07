//! Regression tests for the gRPC status conversion (spec 73, feature 01).
//!
//! WHY: `parse_grpc_error_trailer` built the status code with an unsafe
//! transmute. Feature 01 replaces it with the checked `Code::from_u32`
//! conversion.
//!
//! WHAT: The test calls `parse_grpc_error_trailer` with every `grpc-status`
//! value from 0 to 255. It asserts the resulting code.
//!
//! HOW: It builds one `SimpleHeaders` per value and reads the code from the
//! error. The old code clamps every value, so this behaviour test passes on
//! the old code too. The source gate in the feature file proves that the
//! transmute is gone.

use foundation_connectrpc::shared::error::Code;
use foundation_connectrpc::shared::protocol::grpc::parse_grpc_error_trailer;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};

/// The `Code` for each defined gRPC status code, 1 through 16.
const DEFINED_CODES: [(u32, Code); 16] = [
    (1, Code::Canceled),
    (2, Code::Unknown),
    (3, Code::InvalidArgument),
    (4, Code::DeadlineExceeded),
    (5, Code::NotFound),
    (6, Code::AlreadyExists),
    (7, Code::PermissionDenied),
    (8, Code::ResourceExhausted),
    (9, Code::FailedPrecondition),
    (10, Code::Aborted),
    (11, Code::OutOfRange),
    (12, Code::Unimplemented),
    (13, Code::Internal),
    (14, Code::Unavailable),
    (15, Code::DataLoss),
    (16, Code::Unauthenticated),
];

/// Build trailers that carry one `grpc-status` value.
fn trailers_with_status(status: u32) -> SimpleHeaders {
    let mut trailers = SimpleHeaders::new();
    trailers.insert(
        SimpleHeader::from("grpc-status".to_string()),
        vec![status.to_string()],
    );
    trailers
}

#[test]
fn grpc_status_conversion_maps_every_wire_value() {
    // 0 means "no error", so the parser returns None before the conversion.
    assert!(parse_grpc_error_trailer(&trailers_with_status(0)).is_none());

    // Every defined code keeps its own variant.
    for (wire, expected) in DEFINED_CODES {
        let error = parse_grpc_error_trailer(&trailers_with_status(wire))
            .unwrap_or_else(|| panic!("grpc-status {wire} must map to an error"));
        assert_eq!(
            error.current_context().code(),
            expected,
            "grpc-status {wire}"
        );
    }

    // Every other non-zero wire value maps to Unknown and never panics.
    for wire in 17..=255 {
        let error = parse_grpc_error_trailer(&trailers_with_status(wire))
            .unwrap_or_else(|| panic!("grpc-status {wire} must map to an error"));
        assert_eq!(
            error.current_context().code(),
            Code::Unknown,
            "grpc-status {wire}"
        );
    }
}
