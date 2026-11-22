//! Bearer token parsing and formatting (RFC 9110).

use super::secret::BEARER_PREFIX;
use super::Secret;

/// The token from an `Authorization: Bearer` value (RFC 9110).
///
/// The scheme is case-insensitive. The token must be visible ASCII; see
/// [`Secret::from_bearer`].
#[must_use]
pub fn extract_bearer_token(auth_header: Option<&str>) -> Option<String> {
    Secret::from_bearer(auth_header).map(|secret| secret.expose().to_owned())
}

/// The header value for a secret: `Bearer <token>`.
///
/// This is the one place that builds the header, so a client and a test do not
/// write `format!("Bearer {}", ..)`.
#[must_use]
pub fn bearer_header(secret: &Secret) -> String {
    format!("Bearer {}", secret.expose())
}

/// Whether a header names the `Bearer` scheme.
///
/// A provider uses this to tell "no Bearer credential of this kind" (a
/// different scheme) from "a Bearer credential that is malformed".
pub(crate) fn is_bearer_scheme(header: &str) -> bool {
    header
        .as_bytes()
        .get(..BEARER_PREFIX.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(BEARER_PREFIX))
}
