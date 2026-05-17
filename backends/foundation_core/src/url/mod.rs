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
mod errors;
mod path;
mod query;
mod scheme;

pub use authority::*;
pub use errors::*;
pub use path::*;
pub use query::*;
pub use scheme::*;

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
/// use foundation_core::url::Uri;
///
/// // Parse from string
/// let uri = Uri::parse("https://user:pass@example.com:8080/path?key=value#section").unwrap();
/// assert_eq!(uri.scheme().as_str(), "https");
/// assert_eq!(uri.host_str().unwrap(), "example.com");
/// assert_eq!(uri.port().unwrap(), 8080);
/// assert_eq!(uri.path(), "/path");
/// assert_eq!(uri.query().unwrap(), "key=value");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uri {
    /// URI scheme (http, https, etc.)
    pub scheme: Scheme,
    /// Optional authority (userinfo@host:port)
    pub authority: Option<Authority>,
    /// Path and query components
    pub path_and_query: PathAndQuery,
    /// Optional fragment (after #)
    pub fragment: Option<String>,
}

impl Uri {
    /// Parses a URI from a string.
    ///
    /// This parser accepts two shapes of input:
    /// 1. Absolute URIs with a scheme (e.g. "<http://example.com/path>")
    /// 2. Path-only inputs that start with '/' (e.g. "/path?query") — these are
    ///    treated as a path + query with a default scheme of `http` and no authority.
    ///
    /// Rationale: server-side code and many internal tests construct requests
    /// using path-only URLs ("/"). Treating leading-slash inputs as a valid,
    /// path-only URI simplifies callers and keeps `Uri::parse` useful in both
    /// contexts while still rejecting ambiguous inputs like "example.com/path"
    /// that lack a scheme and a leading slash.
    ///
    /// # Arguments
    ///
    /// * `uri` - The URI string to parse
    ///
    /// # Errors
    ///
    /// Returns `InvalidUri` if:
    /// - URI format is invalid for absolute URIs
    /// - For path-only form, path parsing fails
    pub fn parse(uri: &str) -> Result<Self, InvalidUri> {
        // Accept path-only URIs (starting with '/'). These are common for server
        // side request representations and tests that pass only the path.
        if uri.starts_with('/') {
            let path_and_query = PathAndQuery::parse(uri)?;
            return Ok(Uri {
                scheme: Scheme::HTTP,
                authority: None,
                path_and_query,
                fragment: None,
            });
        }

        // Otherwise require an explicit scheme as before.
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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
    /// use foundation_core::url::Uri;
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

    // ── Builder methods ─────────────────────────────────────────────

    /// Returns a new `Uri` with the given query string (without leading '?').
    #[must_use]
    pub fn with_query(&self, query: impl Into<String>) -> Self {
        let query_str = query.into();
        let path_and_query = PathAndQuery {
            path: self.path_and_query.path().to_string(),
            query: if query_str.is_empty() {
                None
            } else {
                Some(query_str)
            },
        };
        Uri {
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
            path_and_query,
            fragment: self.fragment.clone(),
        }
    }

    /// Returns a new `Uri` with the given path.
    #[must_use]
    pub fn with_path(&self, path: impl Into<String>) -> Self {
        let new_path = path.into();
        let path_and_query = PathAndQuery {
            path: if new_path.starts_with('/') {
                new_path
            } else {
                format!("/{new_path}")
            },
            query: self.path_and_query.query().map(|s| s.to_string()),
        };
        Uri {
            scheme: self.scheme.clone(),
            authority: self.authority.clone(),
            path_and_query,
            fragment: self.fragment.clone(),
        }
    }

    /// Returns a new `Uri` with the given authority.
    #[must_use]
    pub fn with_authority(&self, authority: impl Into<Option<Authority>>) -> Self {
        Uri {
            scheme: self.scheme.clone(),
            authority: authority.into(),
            path_and_query: self.path_and_query.clone(),
            fragment: self.fragment.clone(),
        }
    }

    /// Returns a new `Uri` with the given scheme.
    #[must_use]
    pub fn with_scheme(&self, scheme: Scheme) -> Self {
        Uri {
            scheme,
            authority: self.authority.clone(),
            path_and_query: self.path_and_query.clone(),
            fragment: self.fragment.clone(),
        }
    }
}

/// Builder for constructing URIs from scratch.
///
/// # Examples
///
/// ```
/// use foundation_core::url::{UriBuilder, Scheme, Query};
///
/// let uri = UriBuilder::new()
///     .scheme(Scheme::HTTPS)
///     .authority_str("auth.example.com")
///     .path("/oauth/authorize")
///     .query_str("response_type=code&client_id=abc")
///     .build();
///
/// assert!(uri.to_string().contains("https://auth.example.com"));
/// assert!(uri.to_string().contains("response_type=code"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct UriBuilder {
    scheme: Option<Scheme>,
    authority: Option<Authority>,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

impl UriBuilder {
    /// Creates a new empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the scheme.
    #[must_use]
    pub fn scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    /// Sets the authority from an [`Authority`] value.
    #[must_use]
    pub fn authority(mut self, authority: Authority) -> Self {
        self.authority = Some(authority);
        self
    }

    /// Parses and sets the authority from a string (e.g. "example.com:8080").
    ///
    /// # Errors
    ///
    /// Returns an error string if the authority cannot be parsed.
    pub fn authority_str(mut self, s: impl Into<String>) -> Result<Self, String> {
        let s = s.into();
        let authority = Authority::parse(&s).map_err(|e| e.to_string())?;
        self.authority = Some(authority);
        Ok(self)
    }

    /// Sets the path. Ensures it starts with '/'.
    #[must_use]
    pub fn path(mut self, path: impl Into<String>) -> Self {
        let p = path.into();
        self.path = if p.starts_with('/') { p } else { format!("/{p}") };
        self
    }

    /// Sets the query string (without leading '?').
    #[must_use]
    pub fn query_str(mut self, query: impl Into<String>) -> Self {
        let q = query.into();
        self.query = if q.is_empty() { None } else { Some(q) };
        self
    }

    /// Sets the query from a [`Query`] builder.
    #[must_use]
    pub fn query(mut self, query: Query) -> Self {
        let q = query.to_string();
        self.query = if q.is_empty() { None } else { Some(q) };
        self
    }

    /// Sets the fragment (without leading '#').
    #[must_use]
    pub fn fragment(mut self, fragment: impl Into<String>) -> Self {
        let f = fragment.into();
        self.fragment = if f.is_empty() { None } else { Some(f) };
        self
    }

    /// Builds the final [`Uri`].
    ///
    /// # Errors
    ///
    /// Returns an error if no scheme was set (scheme is required).
    pub fn build(self) -> Result<Uri, InvalidUri> {
        let scheme = self.scheme.ok_or_else(|| {
            InvalidUri::new("cannot build Uri without a scheme — call .scheme() first")
        })?;
        let path_and_query = PathAndQuery {
            path: if self.path.is_empty() {
                "/".to_string()
            } else if self.path.starts_with('/') {
                self.path
            } else {
                format!("/{}", self.path)
            },
            query: self.query,
        };
        Ok(Uri {
            scheme,
            authority: self.authority,
            path_and_query,
            fragment: self.fragment,
        })
    }
}

#[allow(clippy::from_over_into)]
impl Into<String> for Uri {
    fn into(self) -> String {
        format!("{self:}")
    }
}

// impl Uri {
//     pub fn to_string(&self) -> String {
//         format!("{self:}")
//     }
// }

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
