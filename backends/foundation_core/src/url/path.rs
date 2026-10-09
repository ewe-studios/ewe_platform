//! URI path and query components.

use std::fmt;

use crate::url::errors::InvalidUri;
use crate::url::query::Query;

/// Path and query component of a URI.
///
/// WHY: Paths and queries are often handled together since they form
/// the resource identifier after the authority.
///
/// WHAT: Represents `path[?query]` with validation and normalization.
///
/// HOW: Stores path and a structured [`Query`]. The `Query` owns the decoded
/// key-value pairs *and* preserves the raw parsed string, so callers can
/// manipulate individual parameters (via [`crate::url::Uri`] helpers) while
/// unmutated queries still render byte-for-byte — critical for proxying and
/// signed URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathAndQuery {
    /// Path component (may be empty for authority-only URIs)
    pub path: String,
    /// Structured query component (empty [`Query`] when absent).
    pub query: Query,
}

impl PathAndQuery {
    /// Parses a path and query from a string.
    ///
    /// # Arguments
    ///
    /// * `s` - String containing path and optionally query (no scheme/authority)
    ///
    /// # Errors
    ///
    /// Returns `InvalidUri` if path contains invalid characters.
    ///
    /// # Panics
    ///
    /// This function does not panic. Invalid path characters return `Err(InvalidUri)`.
    pub fn parse(s: &str) -> Result<Self, InvalidUri> {
        // Empty string is valid (becomes "/" in absolute URIs)
        if s.is_empty() {
            return Ok(PathAndQuery {
                path: "/".to_string(),
                query: Query::new(),
            });
        }

        // Split on '?' for query
        let (path_str, query) = if let Some(q_pos) = s.find('?') {
            let query_str = &s[q_pos + 1..];
            let query = Query::parse(query_str)
                .map_err(|e| InvalidUri::new(format!("invalid query: {e}")))?;
            (&s[..q_pos], query)
        } else {
            (s, Query::new())
        };

        // Validate path
        Self::validate_path(path_str)?;

        // Ensure path starts with '/' for absolute URIs
        let path = if path_str.is_empty() || path_str.starts_with('/') {
            if path_str.is_empty() {
                "/".to_string()
            } else {
                path_str.to_string()
            }
        } else {
            // Relative path - prepend '/'
            format!("/{path_str}")
        };

        Ok(PathAndQuery { path, query })
    }

    /// Validates a path string according to RFC 3986.
    ///
    /// Allows: unreserved / pct-encoded / sub-delims / ":" / "@" / "/"
    fn validate_path(s: &str) -> Result<(), InvalidUri> {
        for c in s.chars() {
            if !(c.is_ascii_alphanumeric()
                || c == '-'
                || c == '.'
                || c == '_'
                || c == '~' // unreserved
                || c == '!'
                || c == '$'
                || c == '&'
                || c == '\''
                || c == '('
                || c == ')'
                || c == '*'
                || c == '+'
                || c == ','
                || c == ';'
                || c == '=' // sub-delims
                || c == ':'
                || c == '@' // pchar
                || c == '/' // path separator
                || c == '%')
            // pct-encoded
            {
                return Err(InvalidUri::new(format!("invalid path character: {c}")));
            }
        }
        Ok(())
    }

    /// Returns the path component.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the serialized query component if present.
    ///
    /// WHY: Callers that only need the raw `key=value&...` string (for
    /// rendering a request target) shouldn't reach into the structured [`Query`].
    ///
    /// WHAT: Returns `Some(query_string)` when the query has at least one pair,
    /// `None` when empty.
    ///
    /// HOW: Serializes the structured [`Query`] on demand (percent-encoded).
    /// This allocates for non-empty queries. For structured access use
    /// [`PathAndQuery::query_params`].
    ///
    /// # Panics
    ///
    /// This function does not panic.
    #[must_use]
    pub fn query(&self) -> Option<String> {
        if self.query.is_empty() {
            None
        } else {
            Some(self.query.to_string())
        }
    }

    /// Returns a reference to the structured query parameters.
    #[must_use]
    pub fn query_params(&self) -> &Query {
        &self.query
    }

    /// Returns a mutable reference to the structured query parameters.
    #[must_use]
    pub fn query_params_mut(&mut self) -> &mut Query {
        &mut self.query
    }
}

impl fmt::Display for PathAndQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)?;
        if !self.query.is_empty() {
            write!(f, "?{}", self.query)?;
        }
        Ok(())
    }
}
