//! Comprehensive URI/URL parsing and manipulation module.
//!
//! WHY: HTTP operations require robust URL parsing with proper validation,
//! normalization, and component extraction. A solid URI implementation is
//! fundamental to reliable HTTP client operations.
//!
//! WHAT: Provides RFC 3986 compliant URI parsing with support for:
//! - Scheme (http, https, and extensible for others)
//! - Authority (userinfo, host, port)
//! - Path (with normalization)
//! - Query parameters
//! - Fragment
//!
//! HOW: Inspired by hyperium/http, uses zero-copy parsing where possible
//! and proper validation at each component level.

mod authority;
mod error;
mod path;
mod query;
mod scheme;

pub use authority::{Authority, Host};
pub use error::{InvalidUri, InvalidUriParts};
pub use path::PathAndQuery;
pub use query::{Query, QueryError};
pub use scheme::Scheme;

use std::fmt;

/// A parsed URI with all components.
///
/// WHY: URIs are the fundamental addressing mechanism for HTTP. This type
/// provides a validated, structured representation that can be used for
/// HTTP requests, redirects, and URL manipulation.
///
/// WHAT: Represents a URI with optional components following RFC 3986:
/// `scheme:[//authority]path[?query][#fragment]`
///
/// HOW: Components are stored as owned strings after validation. The URI
/// can be constructed via parsing or building individual parts.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::url::Uri;
///
/// // Parse from string
/// let uri = Uri::parse("https://user:pass@example.com:8080/path?key=value#section").unwrap();
/// assert_eq!(uri.scheme().as_str(), "https");
/// assert_eq!(uri.host().unwrap(), "example.com");
/// assert_eq!(uri.port().unwrap(), 8080);
/// assert_eq!(uri.path(), "/path");
/// assert_eq!(uri.query().unwrap(), "key=value");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uri {
    /// URI scheme (http, https, etc.)
    scheme: Scheme,
    /// Optional authority (userinfo@host:port)
    authority: Option<Authority>,
    /// Path and query components
    path_and_query: PathAndQuery,
    /// Optional fragment (after #)
    fragment: Option<String>,
}

impl Uri {
    /// Parses a URI from a string.
    ///
    /// # Purpose (WHY)
    ///
    /// URIs come in many formats and need robust parsing with proper validation
    /// to prevent security issues and ensure correct HTTP operations.
    ///
    /// # Arguments (WHAT)
    ///
    /// * `uri` - The URI string to parse (e.g., "<https://example.com/path?query>")
    ///
    /// # Returns (HOW)
    ///
    /// A validated `Uri` with all components properly parsed.
    ///
    /// # Errors
    ///
    /// Returns `InvalidUri` if:
    /// - URI format is invalid
    /// - Scheme is missing or invalid
    /// - Authority components are malformed
    /// - Port is out of range
    /// - Path contains invalid characters
    ///
    /// # Panics
    ///
    /// This function does not panic. All parsing errors are returned as `Err(InvalidUri)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com/path").unwrap();
    /// assert_eq!(uri.scheme().as_str(), "http");
    /// assert_eq!(uri.host().unwrap(), "example.com");
    /// ```
    pub fn parse(uri: &str) -> Result<Self, InvalidUri> {
        // URI format: scheme:[//authority]path[?query][#fragment]

        // 1. Parse scheme (required)
        let (scheme, rest) = Scheme::parse_from_uri(uri)?;

        // 2. Check for authority (starts with //)
        let (authority, rest) = if let Some(after_slashes) = rest.strip_prefix("//") {
            Authority::parse_with_remainder(after_slashes)?
        } else {
            (None, rest)
        };

        // 3. Split off fragment first (after #)
        let (rest, fragment) = if let Some(hash_pos) = rest.find('#') {
            let frag = &rest[hash_pos + 1..];
            let fragment = if frag.is_empty() {
                None
            } else {
                Some(frag.to_string())
            };
            (&rest[..hash_pos], fragment)
        } else {
            (rest, None)
        };

        // 4. Parse path and query
        let path_and_query = PathAndQuery::parse(rest)?;

        Ok(Uri {
            scheme,
            authority,
            path_and_query,
            fragment,
        })
    }

    /// Returns the scheme component.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("https://example.com").unwrap();
    /// assert_eq!(uri.scheme().as_str(), "https");
    /// ```
    #[must_use]
    pub fn scheme(&self) -> &Scheme {
        &self.scheme
    }

    /// Returns the host component as a string if present.
    ///
    /// WHY: The host is needed for DNS resolution and connection establishment.
    ///
    /// WHAT: Returns the host as a string. For IP addresses, this allocates
    /// since they need to be formatted.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com").unwrap();
    /// assert_eq!(uri.host_str(), Some("example.com".to_string()));
    /// ```
    #[must_use]
    pub fn host_str(&self) -> Option<String> {
        self.authority.as_ref().map(|a| match a.host() {
            Host::RegName(name) => name.clone(),
            Host::Ipv4(addr) => addr.to_string(),
            Host::Ipv6(addr) => addr.to_string(), // Without brackets
        })
    }

    /// Returns a reference to the Host enum if present.
    ///
    /// WHY: Allows pattern matching on the host type without allocation.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::{Uri, Host};
    ///
    /// let uri = Uri::parse("http://192.168.1.1").unwrap();
    /// match uri.host_enum() {
    ///     Some(Host::Ipv4(addr)) => println!("IPv4: {}", addr),
    ///     Some(Host::Ipv6(addr)) => println!("IPv6: {}", addr),
    ///     Some(Host::RegName(name)) => println!("Domain: {}", name),
    ///     None => println!("No host"),
    /// }
    /// ```
    #[must_use]
    pub fn host_enum(&self) -> Option<&Host> {
        self.authority.as_ref().map(authority::Authority::host)
    }

    /// Returns the port component if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com:8080").unwrap();
    /// assert_eq!(uri.port(), Some(8080));
    /// ```
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        self.authority.as_ref().and_then(authority::Authority::port)
    }

    /// Returns the port or the default port for the scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com").unwrap();
    /// assert_eq!(uri.port_or_default(), 80);
    ///
    /// let uri = Uri::parse("https://example.com:8443").unwrap();
    /// assert_eq!(uri.port_or_default(), 8443);
    /// ```
    #[must_use]
    pub fn port_or_default(&self) -> u16 {
        self.port().unwrap_or_else(|| self.scheme.default_port())
    }

    /// Returns the path component.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com/path").unwrap();
    /// assert_eq!(uri.path(), "/path");
    /// ```
    #[must_use]
    pub fn path(&self) -> &str {
        self.path_and_query.path()
    }

    /// Returns the query component if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com?key=value").unwrap();
    /// assert_eq!(uri.query().unwrap(), "key=value");
    /// ```
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.path_and_query.query()
    }

    /// Returns the fragment component if present.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Uri;
    ///
    /// let uri = Uri::parse("http://example.com#section").unwrap();
    /// assert_eq!(uri.fragment().unwrap(), "section");
    /// ```
    #[must_use]
    pub fn fragment(&self) -> Option<&str> {
        self.fragment.as_deref()
    }

    /// Returns the authority component if present.
    #[must_use]
    pub fn authority(&self) -> Option<&Authority> {
        self.authority.as_ref()
    }

    /// Returns the path and query component.
    #[must_use]
    pub fn path_and_query(&self) -> &PathAndQuery {
        &self.path_and_query
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // scheme:
        write!(f, "{}:", self.scheme)?;

        // //authority
        if let Some(auth) = &self.authority {
            write!(f, "//{auth}")?;
        }

        // path?query
        write!(f, "{}", self.path_and_query)?;

        // #fragment
        if let Some(frag) = &self.fragment {
            write!(f, "#{frag}")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_parse_simple_http() {
        let uri = Uri::parse("http://example.com").unwrap();
        assert_eq!(uri.scheme().as_str(), "http");
        assert_eq!(uri.host_str().unwrap(), "example.com");
        assert_eq!(uri.port_or_default(), 80);
        assert_eq!(uri.path(), "/");
        assert!(uri.query().is_none());
    }

    #[test]
    fn test_uri_parse_with_port() {
        let uri = Uri::parse("https://example.com:8443/path").unwrap();
        assert_eq!(uri.scheme().as_str(), "https");
        assert_eq!(uri.host_str().unwrap(), "example.com");
        assert_eq!(uri.port().unwrap(), 8443);
        assert_eq!(uri.path(), "/path");
    }

    #[test]
    fn test_uri_parse_with_query() {
        let uri = Uri::parse("http://example.com/path?key=value&foo=bar").unwrap();
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.query().unwrap(), "key=value&foo=bar");
    }

    #[test]
    fn test_uri_parse_with_fragment() {
        let uri = Uri::parse("http://example.com/path#section").unwrap();
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.fragment().unwrap(), "section");
    }

    #[test]
    fn test_uri_parse_complete() {
        let uri = Uri::parse("https://user:pass@example.com:8080/path?key=value#section").unwrap();
        assert_eq!(uri.scheme().as_str(), "https");
        assert_eq!(uri.host_str().unwrap(), "example.com");
        assert_eq!(uri.port().unwrap(), 8080);
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.query().unwrap(), "key=value");
        assert_eq!(uri.fragment().unwrap(), "section");
    }

    #[test]
    fn test_uri_display() {
        let original = "http://example.com:8080/path?query#fragment";
        let uri = Uri::parse(original).unwrap();
        assert_eq!(uri.to_string(), original);
    }

    #[test]
    fn test_uri_parse_missing_scheme() {
        assert!(Uri::parse("example.com/path").is_err());
    }

    #[test]
    fn test_uri_parse_invalid_port() {
        assert!(Uri::parse("http://example.com:99999/path").is_err());
    }
}
