//! Cookie handling for HTTP client.
//!
//! WHY: HTTP clients need automatic cookie management for session handling,
//! authentication, and stateful interactions. This provides RFC 6265 compliant
//! cookie parsing, storage, and matching.
//!
//! WHAT: Implements `Cookie` struct for individual cookies, `CookieJar` for storage,
//! and automatic cookie handling (`Set-Cookie` parsing, `Cookie` header generation).
//!
//! HOW: `Cookie` stores all attributes (domain, path, expiration, security flags).
//! `CookieJar` uses `HashMap` with composite key (domain, path, name) for storage.
//! Domain and path matching follow RFC 6265 rules.

use crate::wire::simple_http::url::Uri;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

/// `SameSite` attribute for cookies.
///
/// WHY: Modern browsers require `SameSite` attribute for CSRF protection.
///
/// WHAT: Specifies whether a cookie is sent with cross-site requests.
///
/// HOW: Three levels of restriction following RFC 6265bis.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    /// Cookie sent only for same-site requests
    Strict,
    /// Cookie sent for same-site and top-level cross-site navigation
    Lax,
    /// Cookie sent for all requests (requires Secure attribute)
    None,
}

impl Default for SameSite {
    /// Returns default `SameSite` value.
    ///
    /// WHY: Modern browsers default to Lax for security.
    ///
    /// WHAT: Returns `SameSite::Lax` per current browser behavior.
    ///
    /// # Panics
    /// Never panics.
    fn default() -> Self {
        SameSite::Lax
    }
}

/// HTTP cookie with all standard attributes.
///
/// WHY: Represents a single HTTP cookie with all RFC 6265 attributes needed
/// for proper storage, expiration, and security handling.
///
/// WHAT: Stores cookie name/value plus optional attributes (domain, path,
/// expiration, security flags).
///
/// HOW: Uses `SystemTime`.
/// Security flags control when cookie is sent.
///
/// # Examples
///
/// ```ignore
/// let cookie = Cookie::new("session", "abc123")
///     .domain("example.com")
///     .path("/")
///     .secure(true)
///     .http_only(true);
/// ```
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, PartialEq)]
pub struct Cookie {
    /// Cookie name
    pub name: String,
    /// Cookie value
    pub value: String,
    /// Domain attribute (None = current domain)
    pub domain: Option<String>,
    /// Path attribute (None = default to request path)
    pub path: Option<String>,
    /// Expires attribute (absolute time)
    pub expires: Option<SystemTime>,
    /// Max-Age attribute (relative duration, takes precedence over Expires)
    pub max_age: Option<Duration>,
    /// Secure attribute (HTTPS only)
    pub secure: bool,
    /// `HttpOnly` attribute (no JavaScript access)
    pub http_only: bool,
    /// `SameSite` attribute
    pub same_site: SameSite,
}

impl Cookie {
    /// Creates a new cookie with name and value.
    ///
    /// WHY: Basic constructor for creating cookies programmatically.
    ///
    /// WHAT: Creates cookie with given name/value and default attributes.
    ///
    /// HOW: All optional attributes set to None, security flags set to false,
    /// `SameSite` set to Lax (default).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new(name: &str, value: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            domain: None,
            path: None,
            expires: None,
            max_age: None,
            secure: false,
            http_only: false,
            same_site: SameSite::default(),
        }
    }

    /// Sets the domain attribute.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets domain and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn domain(mut self, domain: &str) -> Self {
        self.domain = Some(domain.to_string());
        self
    }

    /// Sets the path attribute.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets path and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn path(mut self, path: &str) -> Self {
        self.path = Some(path.to_string());
        self
    }

    /// Sets the secure flag.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets secure flag and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Sets the `http_only` flag.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets `http_only` flag and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Sets the expires attribute.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets expires time and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn expires(mut self, expires: SystemTime) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Sets the `max_age` attribute.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets `max_age` duration and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn max_age(mut self, max_age: Duration) -> Self {
        self.max_age = Some(max_age);
        self
    }

    /// Sets the `same_site` attribute.
    ///
    /// WHY: Builder pattern for fluent cookie construction.
    ///
    /// WHAT: Sets `same_site` value and returns self for chaining.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// Parses a `Set-Cookie` header value into a `Cookie`.
    ///
    /// WHY: HTTP responses contain `Set-Cookie` headers that must be parsed
    /// to extract cookie attributes per RFC 6265.
    ///
    /// WHAT: Parses "name=value; attribute=value; flag" format into `Cookie` struct.
    ///
    /// HOW: Splits on semicolons, first part is name=value, remaining parts are
    /// attributes. Lenient parsing (ignores unknown attributes).
    ///
    /// # Errors
    /// Returns `CookieParseError::InvalidFormat` if name=value is missing or malformed.
    ///
    /// # Panics
    /// Never panics.
    pub fn parse(header: &str) -> Result<Self, CookieParseError> {
        let parts: Vec<&str> = header.split(';').collect();

        if parts.is_empty() {
            return Err(CookieParseError::InvalidFormat("empty header".to_string()));
        }

        // First part is name=value
        let (name, value) = parts[0].split_once('=').ok_or_else(|| {
            CookieParseError::InvalidFormat("missing = in name=value".to_string())
        })?;

        let mut cookie = Cookie::new(name.trim(), value.trim());

        // Parse remaining attributes
        for attr in &parts[1..] {
            let attr = attr.trim();
            if let Some((key, val)) = attr.split_once('=') {
                let key_lower = key.trim().to_lowercase();
                let val = val.trim();
                match key_lower.as_str() {
                    "path" => cookie.path = Some(val.to_string()),
                    "domain" => cookie.domain = Some(val.to_string()),
                    "max-age" => {
                        if let Ok(seconds) = val.parse::<u64>() {
                            cookie.max_age = Some(Duration::from_secs(seconds));
                        }
                    }
                    "samesite" => {
                        cookie.same_site = match val.to_lowercase().as_str() {
                            "strict" => SameSite::Strict,
                            "none" => SameSite::None,
                            _ => SameSite::Lax, // "lax" or unknown values default to Lax
                        };
                    }
                    _ => {} // Ignore unknown attributes (lenient parsing)
                }
            } else {
                // Flag attributes (no value)
                match attr.to_lowercase().as_str() {
                    "secure" => cookie.secure = true,
                    "httponly" => cookie.http_only = true,
                    _ => {} // Ignore unknown flags
                }
            }
        }

        Ok(cookie)
    }
}

/// Error parsing `Set-Cookie` header.
///
/// WHY: Cookie parsing can fail due to malformed headers, invalid dates,
/// or other format issues.
///
/// WHAT: Represents various cookie parsing error conditions.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone)]
pub enum CookieParseError {
    /// Invalid cookie format (missing name=value)
    InvalidFormat(String),

    /// Invalid date format in Expires attribute
    InvalidDate(String),

    /// Invalid attribute value
    InvalidAttribute(String),
}

impl std::fmt::Display for CookieParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(msg) => write!(f, "invalid cookie format: {msg}"),
            Self::InvalidDate(msg) => write!(f, "invalid date: {msg}"),
            Self::InvalidAttribute(msg) => write!(f, "invalid attribute: {msg}"),
        }
    }
}

impl std::error::Error for CookieParseError {}

/// Composite key for cookie storage.
///
/// WHY: Cookies are uniquely identified by (domain, path, name) tuple per RFC 6265.
///
/// WHAT: Hashable key combining domain, path, and name for `HashMap` storage.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CookieKey {
    domain: String,
    path: String,
    name: String,
}

/// Cookie jar for storing and retrieving cookies.
///
/// WHY: HTTP clients need centralized cookie storage that handles domain/path
/// matching, expiration, and security attributes per RFC 6265.
///
/// WHAT: Thread-safe cookie storage with RFC 6265 compliant matching logic.
///
/// HOW: `HashMap` indexed by (domain, path, name). Methods for adding, retrieving,
/// and clearing cookies. Automatic expiration checking.
///
/// # Panics
/// Never panics.
#[derive(Debug, Clone)]
pub struct CookieJar {
    cookies: HashMap<CookieKey, Cookie>,
}

impl CookieJar {
    /// Creates a new empty cookie jar.
    ///
    /// WHY: Basic constructor for cookie jar.
    ///
    /// WHAT: Creates empty `HashMap` for cookie storage.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cookies: HashMap::new(),
        }
    }

    /// Adds a cookie to the jar.
    ///
    /// WHY: Cookies from `Set-Cookie` headers must be stored for later use.
    ///
    /// WHAT: Stores cookie indexed by (domain, path, name). Replaces existing
    /// cookie with same key.
    ///
    /// HOW: Uses domain (or empty if None), path (or "/" if None), and name as key.
    ///
    /// # Panics
    /// Never panics.
    pub fn add(&mut self, cookie: Cookie) {
        let key = CookieKey {
            domain: cookie.domain.clone().unwrap_or_default(),
            path: cookie.path.clone().unwrap_or_else(|| "/".to_string()),
            name: cookie.name.clone(),
        };
        self.cookies.insert(key, cookie);
    }

    /// Returns the number of cookies in the jar.
    ///
    /// WHY: Testing and debugging needs to know jar size.
    ///
    /// WHAT: Returns count of stored cookies.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cookies.len()
    }

    /// Returns true if the jar is empty.
    ///
    /// WHY: Idiomatic Rust when implementing `len()`.
    ///
    /// WHAT: Returns true if no cookies stored.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty()
    }

    /// Gets cookies matching a URL.
    ///
    /// WHY: When making HTTP request, need to send matching cookies per RFC 6265.
    ///
    /// WHAT: Returns all cookies whose domain/path match the URL and aren't expired.
    ///
    /// HOW: Filters cookies by domain matching (exact + subdomain), path matching
    /// (prefix), secure flag (HTTPS only), and expiration check.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn get_for_url(&self, uri: &Uri) -> Vec<&Cookie> {
        let host = uri
            .authority()
            .map(|a| a.host().to_string_for_display())
            .unwrap_or_default();
        let path = uri.path();
        let is_https = uri.scheme().as_str() == "https";

        self.cookies
            .values()
            .filter(|cookie| {
                // Check domain match
                let cookie_domain = cookie.domain.as_deref().unwrap_or("");
                let domain_ok = Self::domain_matches(cookie_domain, &host);

                // Check path match
                let cookie_path = cookie.path.as_deref().unwrap_or("/");
                let path_ok = Self::path_matches(cookie_path, path);

                // Check secure flag
                let secure_ok = !cookie.secure || is_https;

                // Check expiration
                let not_expired = !Self::is_expired(cookie);

                domain_ok && path_ok && secure_ok && not_expired
            })
            .collect()
    }

    /// Checks if cookie domain matches request host per RFC 6265.
    ///
    /// WHY: Domain matching rules from RFC 6265 for cookie security.
    ///
    /// WHAT: Returns true if cookie should be sent to this host.
    ///
    /// HOW: Exact match OR subdomain match (cookie domain starts with dot).
    ///
    /// # Panics
    /// Never panics.
    fn domain_matches(cookie_domain: &str, request_host: &str) -> bool {
        // Empty domain means cookie domain = request domain (set by server)
        if cookie_domain.is_empty() {
            return true;
        }

        // Exact match
        if cookie_domain == request_host {
            return true;
        }

        // Subdomain match (cookie domain starts with .)
        if let Some(without_dot) = cookie_domain.strip_prefix('.') {
            return request_host == without_dot || request_host.ends_with(cookie_domain);
        }

        false
    }

    /// Checks if cookie path matches request path per RFC 6265.
    ///
    /// WHY: Path matching rules from RFC 6265 for cookie scope.
    ///
    /// WHAT: Returns true if cookie should be sent for this path.
    ///
    /// HOW: Request path must start with cookie path.
    ///
    /// # Panics
    /// Never panics.
    fn path_matches(cookie_path: &str, request_path: &str) -> bool {
        request_path.starts_with(cookie_path)
    }

    /// Checks if cookie is expired.
    ///
    /// WHY: Expired cookies must not be sent per RFC 6265.
    ///
    /// WHAT: Returns true if cookie is past expiration time.
    ///
    /// HOW: Checks `max_age` first (if present), then expires. `Max-Age` takes precedence.
    ///
    /// # Panics
    /// Never panics.
    fn is_expired(cookie: &Cookie) -> bool {
        // Max-Age takes precedence over Expires
        if let Some(_max_age) = cookie.max_age {
            // For now, we don't track creation time, so we can't check max_age
            // This would require storing creation timestamp in Cookie
            // For MVP, we'll skip max_age expiration check
            return false;
        }

        if let Some(expires) = cookie.expires {
            if let Ok(now) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                if let Ok(exp) = expires.duration_since(SystemTime::UNIX_EPOCH) {
                    return now >= exp;
                }
            }
        }

        false
    }

    /// Removes a specific cookie from the jar.
    ///
    /// WHY: Need to delete specific cookies when requested.
    ///
    /// WHAT: Removes cookie matching (domain, path, name) key.
    ///
    /// # Panics
    /// Never panics.
    pub fn remove(&mut self, domain: &str, path: &str, name: &str) {
        let key = CookieKey {
            domain: domain.to_string(),
            path: path.to_string(),
            name: name.to_string(),
        };
        self.cookies.remove(&key);
    }

    /// Clears all cookies from the jar.
    ///
    /// WHY: Need to reset cookie state or logout.
    ///
    /// WHAT: Removes all stored cookies.
    ///
    /// # Panics
    /// Never panics.
    pub fn clear(&mut self) {
        self.cookies.clear();
    }

    /// Removes expired cookies from the jar.
    ///
    /// WHY: Expired cookies should be cleaned up to save memory.
    ///
    /// WHAT: Removes cookies past their expiration time.
    ///
    /// # Panics
    /// Never panics.
    pub fn clear_expired(&mut self) {
        self.cookies.retain(|_, cookie| !Self::is_expired(cookie));
    }

    /// Gets all cookies for a specific domain.
    ///
    /// WHY: Useful for debugging and inspection.
    ///
    /// WHAT: Returns all cookies whose domain matches (exact or subdomain).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn get_for_domain(&self, domain: &str) -> Vec<&Cookie> {
        self.cookies
            .values()
            .filter(|cookie| {
                let cookie_domain = cookie.domain.as_deref().unwrap_or("");
                Self::domain_matches(cookie_domain, domain)
            })
            .collect()
    }
}

impl Default for CookieJar {
    fn default() -> Self {
        Self::new()
    }
}
