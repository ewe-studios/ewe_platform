//! `Code` — the 16 ConnectRPC / gRPC status codes and their mappings.
//!
//! WHY: Every RPC error carries one of these codes; they are shared across all
//! three wire protocols (Connect, gRPC, gRPC-Web) and map deterministically to
//! HTTP status codes (Decision 03). The numeric values match gRPC exactly, so
//! `grpc_code()` is the identity.
//!
//! WHAT: A `#[repr(u32)]` enum plus name/number/HTTP-status conversions matching
//! the Decision 03 tables byte-for-byte.
//!
//! HOW: Hand-written match arms — a table small enough that a lookup array would
//! be less readable and no faster.

/// A ConnectRPC status code (identical numbering to gRPC status codes).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Code {
    /// The operation was canceled (typically by the caller).
    Canceled = 1,
    /// Unknown error (e.g. a `Status` from another address space with an
    /// unrecognized code, or an error with no status).
    Unknown = 2,
    /// The client specified an invalid argument (independent of system state).
    InvalidArgument = 3,
    /// The deadline expired before the operation could complete.
    DeadlineExceeded = 4,
    /// A requested entity was not found.
    NotFound = 5,
    /// The entity a client attempted to create already exists.
    AlreadyExists = 6,
    /// The caller does not have permission to execute the operation.
    PermissionDenied = 7,
    /// A resource has been exhausted (quota, disk, …).
    ResourceExhausted = 8,
    /// The system is not in a state required for the operation.
    FailedPrecondition = 9,
    /// The operation was aborted (concurrency conflict — retry higher-level txn).
    Aborted = 10,
    /// The operation was attempted past the valid range.
    OutOfRange = 11,
    /// The operation is not implemented or supported.
    Unimplemented = 12,
    /// Internal error — an invariant expected by the system was broken.
    Internal = 13,
    /// The service is currently unavailable (retry with backoff).
    Unavailable = 14,
    /// Unrecoverable data loss or corruption.
    DataLoss = 15,
    /// The request does not have valid authentication credentials.
    Unauthenticated = 16,
}

impl Code {
    /// The Connect wire name (lowercase snake_case), e.g. `"invalid_argument"`.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Code::Canceled => "canceled",
            Code::Unknown => "unknown",
            Code::InvalidArgument => "invalid_argument",
            Code::DeadlineExceeded => "deadline_exceeded",
            Code::NotFound => "not_found",
            Code::AlreadyExists => "already_exists",
            Code::PermissionDenied => "permission_denied",
            Code::ResourceExhausted => "resource_exhausted",
            Code::FailedPrecondition => "failed_precondition",
            Code::Aborted => "aborted",
            Code::OutOfRange => "out_of_range",
            Code::Unimplemented => "unimplemented",
            Code::Internal => "internal",
            Code::Unavailable => "unavailable",
            Code::DataLoss => "data_loss",
            Code::Unauthenticated => "unauthenticated",
        }
    }

    /// Parse a Connect wire name (`"canceled"` → [`Code::Canceled`]).
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "canceled" => Code::Canceled,
            "unknown" => Code::Unknown,
            "invalid_argument" => Code::InvalidArgument,
            "deadline_exceeded" => Code::DeadlineExceeded,
            "not_found" => Code::NotFound,
            "already_exists" => Code::AlreadyExists,
            "permission_denied" => Code::PermissionDenied,
            "resource_exhausted" => Code::ResourceExhausted,
            "failed_precondition" => Code::FailedPrecondition,
            "aborted" => Code::Aborted,
            "out_of_range" => Code::OutOfRange,
            "unimplemented" => Code::Unimplemented,
            "internal" => Code::Internal,
            "unavailable" => Code::Unavailable,
            "data_loss" => Code::DataLoss,
            "unauthenticated" => Code::Unauthenticated,
            _ => return None,
        })
    }

    /// Parse a numeric code (`1` → [`Code::Canceled`]). `0` (gRPC OK) and any
    /// value outside `1..=16` return `None`.
    #[must_use]
    pub fn from_u32(n: u32) -> Option<Self> {
        Some(match n {
            1 => Code::Canceled,
            2 => Code::Unknown,
            3 => Code::InvalidArgument,
            4 => Code::DeadlineExceeded,
            5 => Code::NotFound,
            6 => Code::AlreadyExists,
            7 => Code::PermissionDenied,
            8 => Code::ResourceExhausted,
            9 => Code::FailedPrecondition,
            10 => Code::Aborted,
            11 => Code::OutOfRange,
            12 => Code::Unimplemented,
            13 => Code::Internal,
            14 => Code::Unavailable,
            15 => Code::DataLoss,
            16 => Code::Unauthenticated,
            _ => return None,
        })
    }

    /// HTTP status for a Connect unary error response (Decision 03 table).
    #[must_use]
    pub fn http_status(&self) -> u16 {
        match self {
            Code::Canceled => 499,
            Code::Unknown => 500,
            Code::InvalidArgument => 400,
            Code::DeadlineExceeded => 504,
            Code::NotFound => 404,
            Code::AlreadyExists => 409,
            Code::PermissionDenied => 403,
            Code::ResourceExhausted => 429,
            Code::FailedPrecondition => 400,
            Code::Aborted => 409,
            Code::OutOfRange => 400,
            Code::Unimplemented => 501,
            Code::Internal => 500,
            Code::Unavailable => 503,
            Code::DataLoss => 500,
            Code::Unauthenticated => 401,
        }
    }

    /// Infer a `Code` from the HTTP status of a **non-Connect** response
    /// (Decision 03 reverse table). Anything not listed maps to
    /// [`Code::Unknown`].
    #[must_use]
    pub fn from_http_status(status: u16) -> Self {
        match status {
            400 => Code::Internal,
            401 => Code::Unauthenticated,
            403 => Code::PermissionDenied,
            404 => Code::Unimplemented,
            429 | 502 | 503 | 504 => Code::Unavailable,
            _ => Code::Unknown,
        }
    }

    /// The numeric gRPC status code — identical to this code's discriminant.
    #[must_use]
    pub fn grpc_code(&self) -> u32 {
        *self as u32
    }
}

impl core::fmt::Display for Code {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
