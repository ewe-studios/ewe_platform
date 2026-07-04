//! gRPC timeout codec (Decision 05 §Timeout encoding, P13).
//!
//! WHY: gRPC carries the call deadline in the `grpc-timeout` header as
//! `<value><unit>`, where the value is at most 8 digits and the unit is one of
//! `n`/`u`/`m`/`S`/`M`/`H` (nanos → hours).
//!
//! WHAT: [`encode_grpc_timeout`] / [`decode_grpc_timeout`] + [`TimeoutError`].
//!
//! HOW: encode picks the finest unit whose value fits in 8 digits (grpc parity);
//! decode enforces the 8-digit cap (P13) and maps the unit back to nanoseconds.

use std::error::Error as StdError;
use std::time::Duration;

use crate::error::{ConnectError, ConnectResult};

/// Largest 8-digit value (`99_999_999`).
const MAX_VALUE: u128 = 99_999_999;

/// (nanoseconds-per-unit, wire suffix), finest → coarsest.
const UNITS: [(u128, char); 6] = [
    (1, 'n'),
    (1_000, 'u'),
    (1_000_000, 'm'),
    (1_000_000_000, 'S'),
    (60_000_000_000, 'M'),
    (3_600_000_000_000, 'H'),
];

/// Encode a duration as a `grpc-timeout` value, choosing the finest unit whose
/// numeric value fits in 8 digits (matching grpc's encoder). Durations too large
/// for even `99999999H` are clamped to that maximum.
#[must_use]
pub fn encode_grpc_timeout(duration: Duration) -> String {
    let nanos = duration.as_nanos();
    for (per_unit, suffix) in UNITS {
        let value = nanos / per_unit;
        if value <= MAX_VALUE {
            return format!("{value}{suffix}");
        }
    }
    format!("{MAX_VALUE}H")
}

/// A `grpc-timeout` parse failure (maps to [`Code::InvalidArgument`]).
#[derive(Debug)]
pub enum TimeoutError {
    /// The header was empty.
    Empty,
    /// More than 8 digits (Decision 05 P13).
    TooManyDigits,
    /// The value part was not a non-negative integer.
    InvalidValue,
    /// The unit suffix was not one of `n`/`u`/`m`/`S`/`M`/`H`.
    InvalidUnit(char),
}

impl core::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TimeoutError::Empty => write!(f, "empty grpc-timeout header"),
            TimeoutError::TooManyDigits => {
                write!(f, "grpc-timeout value exceeds 8 digits")
            }
            TimeoutError::InvalidValue => write!(f, "grpc-timeout value is not an integer"),
            TimeoutError::InvalidUnit(c) => write!(f, "invalid grpc-timeout unit {c:?}"),
        }
    }
}

impl StdError for TimeoutError {}

impl From<TimeoutError> for ConnectError {
    fn from(err: TimeoutError) -> Self {
        ConnectError::invalid_argument(err.to_string())
    }
}

/// Decode a `grpc-timeout` header into a [`Duration`]. Enforces the 8-digit cap
/// (Decision 05 P13).
///
/// # Errors
/// [`TimeoutError`] (→ [`Code::InvalidArgument`]) on an empty header, an
/// over-long value, a non-integer value, or an unknown unit.
pub fn decode_grpc_timeout(header: &str) -> Result<Duration, TimeoutError> {
    let unit = header.chars().last().ok_or(TimeoutError::Empty)?;
    let digits = &header[..header.len() - unit.len_utf8()];
    if digits.is_empty() {
        return Err(TimeoutError::Empty);
    }
    if digits.len() > 8 {
        return Err(TimeoutError::TooManyDigits);
    }
    let value: u64 = digits.parse().map_err(|_| TimeoutError::InvalidValue)?;
    let per_unit = UNITS
        .iter()
        .find(|(_, suffix)| *suffix == unit)
        .map(|(per_unit, _)| *per_unit as u64)
        .ok_or(TimeoutError::InvalidUnit(unit))?;
    Ok(Duration::from_nanos(value.saturating_mul(per_unit)))
}

/// Convenience: decode a `grpc-timeout` header into a [`ConnectResult`] deadline
/// duration, mapping parse failures to the RPC error model.
///
/// # Errors
/// A [`ConnectError`]-carrying trace on a malformed header.
pub fn decode_grpc_timeout_connect(header: &str) -> ConnectResult<Duration> {
    decode_grpc_timeout(header).map_err(|e| ConnectError::from(e).into())
}
