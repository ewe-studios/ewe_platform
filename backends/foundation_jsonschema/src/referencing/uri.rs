//! Minimal URI parser with RFC 3986 §5 reference resolution.
//!
//! WHY: JSON Schema reference resolution requires RFC 3986 URI parsing and
//! relative-to-absolute URI resolution. We implement the minimal subset needed
//! rather than pulling in a full URI crate (e.g., `url` or `fluent-uri`) to
//! keep the dependency footprint small and `no_std` compatible.
//!
//! WHAT: `Uri` struct that decomposes a URI into scheme, authority, path,
//! query, and fragment. Supports `resolve()` for RFC 3986 §5 reference
//! resolution and `normalize()` for dot-segment removal.
//!
//! HOW: Positions within the raw string are tracked via byte offsets.
//! Resolution follows the algorithm in RFC 3986 §5.2.2.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A parsed URI decomposed into its RFC 3986 components.
///
/// WHY: Reference resolution requires access to individual URI components
/// (scheme, authority, path, query, fragment) to merge a relative reference
/// against a base URI.
///
/// WHAT: Stores the original string plus byte offsets to each component boundary.
///
/// HOW: Parsed lazily by scanning for `://`, `?`, and `#` delimiters.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Uri {
    raw: String,
    scheme_end: Option<usize>,
    authority_end: Option<usize>,
    path_end: usize,
    query_end: Option<usize>,
}

/// URI parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UriError {
    /// The URI that failed to parse.
    pub uri: String,
    /// The reason parsing failed.
    pub reason: &'static str,
}

impl fmt::Display for UriError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid URI '{}': {}", self.uri, self.reason)
    }
}

impl fmt::Debug for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Uri")
            .field("raw", &self.raw)
            .field("scheme_end", &self.scheme_end)
            .field("authority_end", &self.authority_end)
            .field("path_end", &self.path_end)
            .field("query_end", &self.query_end)
            .field("scheme", &self.scheme())
            .field("authority", &self.authority())
            .field("path", &self.path())
            .field("query", &self.query())
            .field("fragment", &self.fragment())
            .finish()
    }
}

impl Uri {
    /// Parse a URI or URI-reference from a string.
    ///
    /// WHY: Entry point for all URI handling in the referencing engine.
    ///
    /// WHAT: Decomposes the input into scheme, authority, path, query, fragment.
    ///
    /// HOW: Scans for `://`, `?`, `#` delimiters to determine component boundaries.
    /// Accepts both absolute URIs and relative references.
    ///
    /// # Errors
    ///
    /// Returns `UriError` if the input is fundamentally malformed (currently lenient).
    pub fn parse(input: &str) -> Result<Self, UriError> {
        let mut scheme_end = None;
        let mut authority_end = None;
        let mut path_end;
        let mut query_end = None;

        let mut rest = input;
        let mut offset = 0;

        // Extract fragment first (everything after '#')
        let fragment_start = rest.find('#');
        let before_fragment = match fragment_start {
            Some(pos) => &rest[..pos],
            None => rest,
        };

        rest = before_fragment;

        // Extract query (everything after '?' before fragment)
        let query_start = rest.find('?');
        let before_query = match query_start {
            Some(pos) => {
                query_end = Some(pos + offset);
                &rest[..pos]
            }
            None => rest,
        };

        rest = before_query;

        // Detect scheme (letters followed by ':')
        if let Some(colon_pos) = rest.find(':') {
            let potential_scheme = &rest[..colon_pos];
            if !potential_scheme.is_empty()
                && potential_scheme.as_bytes()[0].is_ascii_alphabetic()
                && potential_scheme
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
            {
                scheme_end = Some(colon_pos + 1);
                offset = colon_pos + 1;
                rest = &rest[colon_pos + 1..];
            }
        }

        // Detect authority (starts with "//")
        if rest.starts_with("//") {
            let after_slashes = &rest[2..];
            let authority_len = after_slashes.find('/').unwrap_or(after_slashes.len());
            authority_end = Some(offset + 2 + authority_len);
            offset += 2 + authority_len;
            rest = &after_slashes[authority_len..];
        }

        // Rest is path
        path_end = offset + rest.len();

        // Now account for query
        if let Some(qe) = query_end {
            // query_end was relative to before_fragment, recalc
            let query_content_start = qe + 1; // skip '?'
            let query_content = &before_fragment[qe + 1..];
            path_end = qe;
            query_end = Some(qe + 1 + query_content.len());
        }

        Ok(Uri {
            raw: input.into(),
            scheme_end,
            authority_end,
            path_end,
            query_end,
        })
    }

    /// The URI scheme (e.g., "http", "https", "urn"), without the trailing ':'.
    #[must_use]
    pub fn scheme(&self) -> Option<&str> {
        self.scheme_end.map(|end| &self.raw[..end - 1])
    }

    /// The URI authority (e.g., "example.com", "user@host:8080"), without the leading "//".
    #[must_use]
    pub fn authority(&self) -> Option<&str> {
        self.authority_end.map(|end| {
            let start = self.scheme_end.unwrap_or(0) + 2; // skip "//"
            &self.raw[start..end]
        })
    }

    /// The URI path component.
    #[must_use]
    pub fn path(&self) -> &str {
        let start = self.authority_end.unwrap_or(self.scheme_end.unwrap_or(0));
        &self.raw[start..self.path_end]
    }

    /// The URI query component (without the leading '?').
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query_end.map(|end| &self.raw[self.path_end + 1..end])
    }

    /// The URI fragment (without the leading '#').
    #[must_use]
    pub fn fragment(&self) -> Option<&str> {
        let hash_pos = self.raw.find('#');
        hash_pos.map(|pos| &self.raw[pos + 1..])
    }

    /// The full URI as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Return the URI without its fragment.
    #[must_use]
    pub fn without_fragment(&self) -> &str {
        match self.raw.find('#') {
            Some(pos) => &self.raw[..pos],
            None => &self.raw,
        }
    }

    /// Check if this URI has a scheme.
    #[must_use]
    pub fn has_scheme(&self) -> bool {
        self.scheme_end.is_some()
    }

    /// Resolve a reference URI against this base URI per RFC 3986 §5.
    ///
    /// WHY: `$ref` values in JSON Schema are often relative URIs that must be
    /// resolved against the current base URI to produce an absolute target.
    ///
    /// WHAT: Implements the 4-branch algorithm from RFC 3986 §5.2.2.
    ///
    /// HOW: If the reference has a scheme, it's already absolute. Otherwise,
    /// merge authority/path/query from the reference with components from this
    /// base URI.
    ///
    /// # Errors
    ///
    /// Returns `UriError` if the reference cannot be parsed.
    pub fn resolve(&self, reference: &str) -> Result<Uri, UriError> {
        if reference.is_empty() {
            return Ok(self.clone());
        }

        let r = Uri::parse(reference)?;

        // RFC 3986 §5.2.2
        let (t_scheme, t_authority, t_path, t_query, t_fragment);

        if r.scheme_end.is_some() {
            // Reference has a scheme => use it as-is (except normalize path)
            t_scheme = r.scheme();
            t_authority = r.authority();
            t_path = remove_dot_segments(r.path());
            t_query = r.query().map(String::from);
        } else if r.authority_end.is_some() {
            t_scheme = self.scheme();
            t_authority = r.authority();
            t_path = remove_dot_segments(r.path());
            t_query = r.query().map(String::from);
        } else if r.path().is_empty() {
            t_scheme = self.scheme();
            t_authority = self.authority();
            t_path = self.path().into();
            t_query = if r.query().is_some() {
                r.query().map(String::from)
            } else {
                self.query().map(String::from)
            };
        } else {
            t_scheme = self.scheme();
            t_authority = self.authority();
            if r.path().starts_with('/') {
                t_path = remove_dot_segments(r.path());
            } else {
                t_path = remove_dot_segments(&merge_paths(self, r.path()));
            }
            t_query = r.query().map(String::from);
        }

        t_fragment = r.fragment().map(String::from);

        // Recompose
        let mut result = String::new();
        if let Some(s) = t_scheme {
            result.push_str(s);
            result.push(':');
        }
        if let Some(a) = t_authority {
            result.push_str("//");
            result.push_str(a);
        }
        result.push_str(&t_path);
        if let Some(q) = t_query {
            result.push('?');
            result.push_str(&q);
        }
        if let Some(f) = t_fragment {
            result.push('#');
            result.push_str(&f);
        }

        Uri::parse(&result)
    }

    /// Normalize the URI: remove dot segments, lowercase scheme and host.
    #[must_use]
    pub fn normalize(&self) -> Uri {
        let mut result = String::new();

        if let Some(s) = self.scheme() {
            let lower: String = s.chars().map(|c| c.to_ascii_lowercase()).collect();
            result.push_str(&lower);
            result.push(':');
        }
        if let Some(a) = self.authority() {
            result.push_str("//");
            let lower: String = a.chars().map(|c| c.to_ascii_lowercase()).collect();
            result.push_str(&lower);
        }
        result.push_str(&remove_dot_segments(self.path()));
        if let Some(q) = self.query() {
            result.push('?');
            result.push_str(q);
        }
        if let Some(f) = self.fragment() {
            result.push('#');
            result.push_str(f);
        }

        Uri::parse(&result).unwrap_or_else(|_| self.clone())
    }
}

impl fmt::Display for Uri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

/// The default root URI used when no base is provided.
pub const DEFAULT_ROOT_URI: &str = "json-schema:///";

/// Resolve a reference URI against a base string.
///
/// WHY: Convenience function for the common case of resolving a reference
/// against a base URI given as strings.
///
/// # Errors
///
/// Returns `UriError` if either URI is malformed.
pub fn resolve_against(base: &str, reference: &str) -> Result<Uri, UriError> {
    if reference.starts_with('#') && base.ends_with(reference) {
        return Uri::parse(base);
    }
    let base_uri = Uri::parse(base)?;
    base_uri.resolve(reference)
}

/// Normalize a URI string: lowercase scheme and host, remove dot segments.
/// Returns the original string if it fails to parse.
#[must_use]
pub fn normalize(raw: &str) -> String {
    match Uri::parse(raw) {
        Ok(uri) => uri.normalize().without_fragment().to_string(),
        Err(_) => raw.to_string(),
    }
}

/// Parse a URI string, resolving against the default root if it has no scheme.
///
/// # Errors
///
/// Returns `UriError` if the URI is malformed.
///
/// # Panics
///
/// Panics if `DEFAULT_ROOT_URI` fails to parse (this is a compile-time constant
/// and will never fail in practice).
pub fn from_str(uri: &str) -> Result<Uri, UriError> {
    let trimmed = uri.strip_suffix('#').unwrap_or(uri);
    let parsed = Uri::parse(trimmed)?;
    if parsed.has_scheme() {
        Ok(parsed.normalize())
    } else {
        let root = Uri::parse(DEFAULT_ROOT_URI).expect("default root URI is valid");
        root.resolve(trimmed)
    }
}

/// Merge paths per RFC 3986 §5.2.3.
fn merge_paths(base: &Uri, reference_path: &str) -> String {
    if base.authority().is_some() && base.path().is_empty() {
        let mut merged = String::from("/");
        merged.push_str(reference_path);
        merged
    } else {
        match base.path().rfind('/') {
            Some(pos) => {
                let mut merged = String::from(&base.path()[..=pos]);
                merged.push_str(reference_path);
                merged
            }
            None => reference_path.into(),
        }
    }
}

/// Remove dot segments per RFC 3986 §5.2.4.
fn remove_dot_segments(path: &str) -> String {
    let mut output_segments: Vec<&str> = Vec::new();
    let mut input = path;

    while !input.is_empty() {
        // A: Remove "../" or "./" prefix
        if input.starts_with("../") {
            input = &input[3..];
            continue;
        }
        if input.starts_with("./") {
            input = &input[2..];
            continue;
        }

        // B: Remove "/./" or "/." (end)
        if input.starts_with("/./") {
            input = &input[2..];
            continue;
        }
        if input == "/." {
            input = "/";
            continue;
        }

        // C: Remove "/../" or "/.." (end), pop last output segment
        if input.starts_with("/../") {
            input = &input[3..];
            output_segments.pop();
            continue;
        }
        if input == "/.." {
            input = "/";
            output_segments.pop();
            continue;
        }

        // D: Remove standalone "." or ".."
        if input == "." || input == ".." {
            break;
        }

        // E: Move first path segment to output
        let seg_end = if let Some(rest) = input.strip_prefix('/') {
            match rest.find('/') {
                Some(pos) => pos + 1,
                None => input.len(),
            }
        } else {
            input.find('/').unwrap_or(input.len())
        };

        output_segments.push(&input[..seg_end]);
        input = &input[seg_end..];
    }

    output_segments.concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_absolute_uri() {
        let uri = Uri::parse("http://example.com/path?query=1#frag").unwrap();
        assert_eq!(uri.scheme(), Some("http"));
        assert_eq!(uri.authority(), Some("example.com"));
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.query(), Some("query=1"));
        assert_eq!(uri.fragment(), Some("frag"));
    }

    #[test]
    fn parse_no_fragment() {
        let uri = Uri::parse("http://example.com/path").unwrap();
        assert_eq!(uri.scheme(), Some("http"));
        assert_eq!(uri.authority(), Some("example.com"));
        assert_eq!(uri.path(), "/path");
        assert_eq!(uri.query(), None);
        assert_eq!(uri.fragment(), None);
    }

    #[test]
    fn parse_fragment_only() {
        let uri = Uri::parse("#/definitions/Foo").unwrap();
        assert_eq!(uri.scheme(), None);
        assert_eq!(uri.authority(), None);
        assert_eq!(uri.path(), "");
        assert_eq!(uri.fragment(), Some("/definitions/Foo"));
    }

    #[test]
    fn parse_relative_path() {
        let uri = Uri::parse("other.json").unwrap();
        assert_eq!(uri.scheme(), None);
        assert_eq!(uri.authority(), None);
        assert_eq!(uri.path(), "other.json");
        assert_eq!(uri.fragment(), None);
    }

    #[test]
    fn parse_urn() {
        let uri = Uri::parse("urn:example:schema").unwrap();
        assert_eq!(uri.scheme(), Some("urn"));
        assert_eq!(uri.authority(), None);
        assert_eq!(uri.path(), "example:schema");
    }

    #[test]
    fn parse_empty() {
        let uri = Uri::parse("").unwrap();
        assert_eq!(uri.scheme(), None);
        assert_eq!(uri.authority(), None);
        assert_eq!(uri.path(), "");
        assert_eq!(uri.fragment(), None);
    }

    // RFC 3986 §5.4 — Normal examples
    #[test]
    fn resolve_rfc3986_normal_examples() {
        let base = Uri::parse("http://a/b/c/d;p?q").unwrap();

        assert_eq!(base.resolve("g:h").unwrap().as_str(), "g:h");
        assert_eq!(base.resolve("g").unwrap().as_str(), "http://a/b/c/g");
        assert_eq!(base.resolve("./g").unwrap().as_str(), "http://a/b/c/g");
        assert_eq!(base.resolve("g/").unwrap().as_str(), "http://a/b/c/g/");
        assert_eq!(base.resolve("/g").unwrap().as_str(), "http://a/g");
        assert_eq!(base.resolve("//g/h").unwrap().as_str(), "http://g/h");
        assert_eq!(base.resolve("?y").unwrap().as_str(), "http://a/b/c/d;p?y");
        assert_eq!(base.resolve("g?y").unwrap().as_str(), "http://a/b/c/g?y");
        assert_eq!(base.resolve("#s").unwrap().as_str(), "http://a/b/c/d;p?q#s");
        assert_eq!(base.resolve("g#s").unwrap().as_str(), "http://a/b/c/g#s");
        assert_eq!(
            base.resolve("g?y#s").unwrap().as_str(),
            "http://a/b/c/g?y#s"
        );
        assert_eq!(base.resolve(";x").unwrap().as_str(), "http://a/b/c/;x");
        assert_eq!(base.resolve("g;x").unwrap().as_str(), "http://a/b/c/g;x");
        assert_eq!(
            base.resolve("g;x?y#s").unwrap().as_str(),
            "http://a/b/c/g;x?y#s"
        );
        assert_eq!(base.resolve("").unwrap().as_str(), "http://a/b/c/d;p?q");
        assert_eq!(base.resolve(".").unwrap().as_str(), "http://a/b/c/");
        assert_eq!(base.resolve("./").unwrap().as_str(), "http://a/b/c/");
        assert_eq!(base.resolve("..").unwrap().as_str(), "http://a/b/");
        assert_eq!(base.resolve("../").unwrap().as_str(), "http://a/b/");
        assert_eq!(base.resolve("../g").unwrap().as_str(), "http://a/b/g");
        assert_eq!(base.resolve("../..").unwrap().as_str(), "http://a/");
        assert_eq!(base.resolve("../../").unwrap().as_str(), "http://a/");
        assert_eq!(base.resolve("../../g").unwrap().as_str(), "http://a/g");
    }

    #[test]
    fn resolve_fragment_only_against_base() {
        let base = Uri::parse("http://example.com/schema.json").unwrap();
        let resolved = base.resolve("#/definitions/Foo").unwrap();
        assert_eq!(
            resolved.as_str(),
            "http://example.com/schema.json#/definitions/Foo"
        );
    }

    #[test]
    fn resolve_relative_path() {
        let base = Uri::parse("http://example.com/schemas/base.json").unwrap();
        let resolved = base.resolve("other.json").unwrap();
        assert_eq!(resolved.as_str(), "http://example.com/schemas/other.json");
    }

    #[test]
    fn resolve_absolute_ref() {
        let base = Uri::parse("http://example.com/base").unwrap();
        let resolved = base.resolve("https://other.com/schema").unwrap();
        assert_eq!(resolved.as_str(), "https://other.com/schema");
    }

    #[test]
    fn normalize_scheme_case() {
        let uri = Uri::parse("HTTP://EXAMPLE.COM/Path").unwrap();
        let normalized = uri.normalize();
        assert_eq!(normalized.scheme(), Some("http"));
        assert_eq!(normalized.authority(), Some("example.com"));
        assert_eq!(normalized.path(), "/Path");
    }

    #[test]
    fn without_fragment() {
        let uri = Uri::parse("http://example.com/schema#foo").unwrap();
        assert_eq!(uri.without_fragment(), "http://example.com/schema");
    }

    #[test]
    fn without_fragment_when_none() {
        let uri = Uri::parse("http://example.com/schema").unwrap();
        assert_eq!(uri.without_fragment(), "http://example.com/schema");
    }

    #[test]
    fn from_str_absolute() {
        let uri = from_str("http://example.com/schema").unwrap();
        assert_eq!(uri.as_str(), "http://example.com/schema");
    }

    #[test]
    fn from_str_relative() {
        let uri = from_str("schema.json").unwrap();
        assert_eq!(uri.scheme(), Some("json-schema"));
        assert!(uri.as_str().contains("schema.json"));
    }

    #[test]
    fn from_str_strip_trailing_hash() {
        let uri = from_str("http://example.com/schema#").unwrap();
        assert_eq!(uri.as_str(), "http://example.com/schema");
    }

    #[test]
    fn remove_dot_segments_basic() {
        assert_eq!(remove_dot_segments("/a/b/c/./../../g"), "/a/g");
        assert_eq!(remove_dot_segments("mid/content=5/../6"), "mid/6");
        assert_eq!(remove_dot_segments("/a/b/../c"), "/a/c");
        assert_eq!(remove_dot_segments("/a/b/./c"), "/a/b/c");
    }
}
