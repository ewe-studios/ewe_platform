//! URI scheme component.

use super::error::InvalidUri;
use std::fmt;

/// URI scheme (http, https, etc.).
///
/// WHY: The scheme determines the protocol and default port for a URI.
/// It's the first component parsed and is required for all absolute URIs.
///
/// WHAT: Represents the URI scheme with support for HTTP, HTTPS, and
/// extensibility for other schemes.
///
/// HOW: Stores the scheme as a lowercase string and provides validation
/// during parsing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Scheme {
    inner: SchemeInner,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SchemeInner {
    Http,
    Https,
    Custom(String),
}

impl Scheme {
    /// HTTP scheme constant.
    pub const HTTP: Scheme = Scheme {
        inner: SchemeInner::Http,
    };

    /// HTTPS scheme constant.
    pub const HTTPS: Scheme = Scheme {
        inner: SchemeInner::Https,
    };

    /// Parses a scheme from the beginning of a URI string.
    ///
    /// # Returns
    ///
    /// A tuple of (Scheme, remainder) where remainder is everything after "scheme:"
    ///
    /// # Errors
    ///
    /// Returns `InvalidUri` if:
    /// - No colon separator found
    /// - Scheme is empty
    /// - Scheme contains invalid characters
    ///
    /// # Panics
    ///
    /// This function does not panic. Missing or invalid schemes return `Err(InvalidUri)`.
    pub fn parse_from_uri(uri: &str) -> Result<(Self, &str), InvalidUri> {
        // Find the colon separator
        let colon_pos = uri
            .find(':')
            .ok_or_else(|| InvalidUri::new("missing scheme (no ':' found)"))?;

        if colon_pos == 0 {
            return Err(InvalidUri::new("empty scheme"));
        }

        let scheme_str = &uri[..colon_pos];

        // Validate scheme characters (RFC 3986: ALPHA *( ALPHA / DIGIT / "+" / "-" / "." ))
        if !Self::is_valid_scheme(scheme_str) {
            return Err(InvalidUri::new(format!(
                "invalid scheme characters: {scheme_str}"
            )));
        }

        let scheme = Self::from_str(scheme_str);
        let remainder = &uri[colon_pos + 1..];

        Ok((scheme, remainder))
    }

    /// Creates a scheme from a string (case-insensitive).
    fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        match lower.as_str() {
            "http" => Self::HTTP,
            "https" => Self::HTTPS,
            _ => Scheme {
                inner: SchemeInner::Custom(lower),
            },
        }
    }

    /// Validates scheme string according to RFC 3986.
    fn is_valid_scheme(s: &str) -> bool {
        if s.is_empty() {
            return false;
        }

        let mut chars = s.chars();

        // First character must be alphabetic
        if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
            return false;
        }

        // Remaining characters: ALPHA / DIGIT / "+" / "-" / "."
        chars.all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    }

    /// Returns the scheme as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.inner {
            SchemeInner::Http => "http",
            SchemeInner::Https => "https",
            SchemeInner::Custom(s) => s,
        }
    }

    /// Returns the default port for this scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use foundation_core::wire::simple_http::url::Scheme;
    ///
    /// assert_eq!(Scheme::HTTP.default_port(), 80);
    /// assert_eq!(Scheme::HTTPS.default_port(), 443);
    /// ```
    #[must_use]
    pub fn default_port(&self) -> u16 {
        match &self.inner {
            SchemeInner::Http => 80,
            SchemeInner::Https => 443,
            SchemeInner::Custom(_) => 0, // No default for custom schemes
        }
    }

    /// Returns true if this is the HTTP scheme.
    #[must_use]
    pub fn is_http(&self) -> bool {
        matches!(self.inner, SchemeInner::Http)
    }

    /// Returns true if this is the HTTPS scheme.
    #[must_use]
    pub fn is_https(&self) -> bool {
        matches!(self.inner, SchemeInner::Https)
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
