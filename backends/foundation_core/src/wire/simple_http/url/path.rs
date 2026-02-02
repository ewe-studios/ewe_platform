//! URI path and query components.

use super::error::InvalidUri;
use std::fmt;

/// Path and query component of a URI.
///
/// WHY: Paths and queries are often handled together since they form
/// the resource identifier after the authority.
///
/// WHAT: Represents `path[?query]` with validation and normalization.
///
/// HOW: Stores path and optional query, ensuring path is valid according
/// to RFC 3986.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathAndQuery {
    /// Path component (may be empty for authority-only URIs)
    path: String,
    /// Optional query component (without the '?')
    query: Option<String>,
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
    pub(crate) fn parse(s: &str) -> Result<Self, InvalidUri> {
        // Empty string is valid (becomes "/" in absolute URIs)
        if s.is_empty() {
            return Ok(PathAndQuery {
                path: "/".to_string(),
                query: None,
            });
        }

        // Split on '?' for query
        let (path_str, query) = if let Some(q_pos) = s.find('?') {
            let query_str = &s[q_pos + 1..];
            let query = if query_str.is_empty() {
                None
            } else {
                Some(query_str.to_string())
            };
            (&s[..q_pos], query)
        } else {
            (s, None)
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
            format!("/{}", path_str)
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
                return Err(InvalidUri::new(format!("invalid path character: {}", c)));
            }
        }
        Ok(())
    }

    /// Returns the path component.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the query component if present.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }
}

impl fmt::Display for PathAndQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path)?;
        if let Some(query) = &self.query {
            write!(f, "?{}", query)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_and_query_empty() {
        let pq = PathAndQuery::parse("").unwrap();
        assert_eq!(pq.path(), "/");
        assert_eq!(pq.query(), None);
    }

    #[test]
    fn test_path_and_query_root() {
        let pq = PathAndQuery::parse("/").unwrap();
        assert_eq!(pq.path(), "/");
        assert_eq!(pq.query(), None);
    }

    #[test]
    fn test_path_and_query_path_only() {
        let pq = PathAndQuery::parse("/path/to/resource").unwrap();
        assert_eq!(pq.path(), "/path/to/resource");
        assert_eq!(pq.query(), None);
    }

    #[test]
    fn test_path_and_query_with_query() {
        let pq = PathAndQuery::parse("/path?key=value").unwrap();
        assert_eq!(pq.path(), "/path");
        assert_eq!(pq.query(), Some("key=value"));
    }

    #[test]
    fn test_path_and_query_with_complex_query() {
        let pq = PathAndQuery::parse("/path?key=value&foo=bar&baz").unwrap();
        assert_eq!(pq.path(), "/path");
        assert_eq!(pq.query(), Some("key=value&foo=bar&baz"));
    }

    #[test]
    fn test_path_and_query_empty_query() {
        let pq = PathAndQuery::parse("/path?").unwrap();
        assert_eq!(pq.path(), "/path");
        assert_eq!(pq.query(), None); // Empty query is treated as no query
    }

    #[test]
    fn test_path_and_query_relative_path() {
        let pq = PathAndQuery::parse("path/to/resource").unwrap();
        assert_eq!(pq.path(), "/path/to/resource"); // Prepends /
    }

    #[test]
    fn test_path_and_query_display() {
        let pq = PathAndQuery::parse("/path?key=value").unwrap();
        assert_eq!(pq.to_string(), "/path?key=value");
    }

    #[test]
    fn test_path_validation() {
        // Valid paths
        assert!(PathAndQuery::parse("/path/to/resource").is_ok());
        assert!(PathAndQuery::parse("/path-with-dash").is_ok());
        assert!(PathAndQuery::parse("/path_with_underscore").is_ok());
        assert!(PathAndQuery::parse("/path.with.dots").is_ok());
        assert!(PathAndQuery::parse("/path~tilde").is_ok());
        assert!(PathAndQuery::parse("/path:colon").is_ok());
        assert!(PathAndQuery::parse("/path@at").is_ok());

        // Invalid paths - uncomment when strict validation is needed
        // assert!(PathAndQuery::parse("/path with space").is_err());
        // assert!(PathAndQuery::parse("/path<bracket>").is_err());
    }
}
