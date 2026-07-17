//! URL path pattern matching with `*` and `**` globs.
//!
//! - `*` matches exactly one path segment
//! - `**` matches zero or more path segments (must be the last segment)
//! - Literal segments match exactly
//!
//! Used by `PatternRouter` to match navigation intents against registered
//! route patterns. First-registered first-matched.
//!
//! # Examples
//! ```
//! use foundation_platform::pattern::Pattern;
//! assert!(Pattern::new("/app/*/detail").unwrap().matches("/app/items/detail"));
//! assert!(!Pattern::new("/app/*/detail").unwrap().matches("/app/items/sub/detail"));
//! assert!(Pattern::new("/static/**").unwrap().matches("/static/js/vendor/lib.js"));
//! ```

use std::fmt;

use foundation_ui_traits::*;

// ── Pattern ──────────────────────────────────────────────────────────

/// A URL path pattern. Anchored at the start of the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    segments: Vec<PatternSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PatternSegment {
    /// Exact string match (e.g., "app", "items", "detail").
    Literal(String),
    /// `*` — matches exactly one path segment.
    SingleWildcard,
    /// `**` — matches zero or more path segments. Must be last.
    DoubleWildcard,
}

// ── Construction ─────────────────────────────────────────────────────

impl Pattern {
    /// Parse a pattern string. The leading `/` is optional.
    ///
    /// # Errors
    /// Returns `PatternError` if:
    /// - A segment contains `*` but is not exactly `*` or `**`
    /// - `**` appears anywhere other than the last position
    /// - More than one `**` appears
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        let trimmed = pattern.trim_matches('/');
        if trimmed.is_empty() {
            return Ok(Self { segments: vec![] });
        }

        let parts: Vec<&str> = trimmed.split('/').collect();
        let mut segments = Vec::with_capacity(parts.len());

        for part in parts {
            if part.is_empty() {
                continue; // skip double slashes
            }
            let seg = match part {
                "**" => PatternSegment::DoubleWildcard,
                "*" => PatternSegment::SingleWildcard,
                lit if lit.contains('*') => {
                    return Err(PatternError::InvalidSegment(lit.to_string()));
                }
                lit => PatternSegment::Literal(lit.to_string()),
            };
            segments.push(seg);
        }

        // Validate: at most one `**`, must be last
        let mut double_wildcard_count = 0usize;
        for (i, seg) in segments.iter().enumerate() {
            if matches!(seg, PatternSegment::DoubleWildcard) {
                double_wildcard_count += 1;
                if i != segments.len() - 1 {
                    return Err(PatternError::DoubleWildcardNotLast);
                }
            }
        }
        if double_wildcard_count > 1 {
            return Err(PatternError::MultipleDoubleWildcards);
        }

        Ok(Self { segments })
    }
}

// ── Matching ─────────────────────────────────────────────────────────

impl Pattern {
    /// Test whether a URL path matches this pattern.
    ///
    /// The path may or may not have a leading `/` — both are accepted.
    /// Empty path segments (from double slashes) are skipped.
    pub fn matches(&self, path: &str) -> bool {
        let path_segments: Vec<&str> = path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        self.match_recursive(&path_segments, 0, 0)
    }

    /// Recursive matching with backtracking for `**`.
    fn match_recursive(&self, path: &[&str], pat_idx: usize, path_idx: usize) -> bool {
        // All pattern segments consumed — path must also be fully consumed.
        if pat_idx >= self.segments.len() {
            return path_idx >= path.len();
        }

        match &self.segments[pat_idx] {
            PatternSegment::DoubleWildcard => {
                // `**` matches zero or more remaining segments.
                // Try each possibility: consume 0, 1, 2, ... all remaining segments.
                for i in path_idx..=path.len() {
                    if self.match_recursive(path, pat_idx + 1, i) {
                        return true;
                    }
                }
                false
            }
            PatternSegment::SingleWildcard => {
                // `*` matches exactly one segment.
                if path_idx < path.len() {
                    self.match_recursive(path, pat_idx + 1, path_idx + 1)
                } else {
                    false
                }
            }
            PatternSegment::Literal(lit) => {
                // Exact character match on this segment.
                if path_idx < path.len() && path[path_idx] == lit.as_str() {
                    self.match_recursive(path, pat_idx + 1, path_idx + 1)
                } else {
                    false
                }
            }
        }
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        for (i, seg) in self.segments.iter().enumerate() {
            if i > 0 {
                write!(f, "/")?;
            }
            match seg {
                PatternSegment::Literal(s) => write!(f, "{s}")?,
                PatternSegment::SingleWildcard => write!(f, "*")?,
                PatternSegment::DoubleWildcard => write!(f, "**")?,
            }
        }
        Ok(())
    }
}

// ── Errors ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternError {
    InvalidSegment(String),
    DoubleWildcardNotLast,
    MultipleDoubleWildcards,
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::InvalidSegment(seg) => {
                write!(f, "invalid pattern segment '{seg}': use '*' or '**', not partial wildcards")
            }
            PatternError::DoubleWildcardNotLast => {
                write!(f, "'**' must be the last segment in a pattern")
            }
            PatternError::MultipleDoubleWildcards => {
                write!(f, "only one '**' is allowed per pattern")
            }
        }
    }
}

impl std::error::Error for PatternError {}

// ── PatternRouter ────────────────────────────────────────────────────

/// Declarative route pattern table. Implements `RouteHandler` — iterates
/// registered patterns in registration order, returns the first match.
pub struct PatternRouter {
    patterns: Vec<(Pattern, RouteDecision)>,
}

impl PatternRouter {
    /// Create an empty pattern router.
    pub fn new() -> Self {
        Self { patterns: Vec::new() }
    }

    /// Register a pattern → decision mapping. Patterns are checked in
    /// registration order — first match wins.
    ///
    /// # Panics
    /// Panics if the pattern string is invalid. Pattern validity is a
    /// programmer error (not a runtime error) — invalid patterns should
    /// be caught at compile time or in tests.
    pub fn route(&mut self, pattern: &str, decision: RouteDecision) {
        let pat = Pattern::new(pattern).unwrap_or_else(|e| {
            panic!("invalid route pattern '{pattern}': {e}");
        });
        self.patterns.push((pat, decision));
    }

    /// Try to match a URL path against registered patterns.
    /// Returns the decision from the first matching pattern, or `None`.
    pub fn resolve_path(&self, path: &str) -> Option<RouteDecision> {
        for (pattern, decision) in &self.patterns {
            if pattern.matches(path) {
                return Some(decision.clone());
            }
        }
        None
    }

    /// Try to match a navigation intent against registered patterns.
    /// Extracts the path from the URL and calls `resolve_path`.
    pub fn resolve_intent(&self, intent: &NavigationIntent) -> Option<RouteDecision> {
        let path = extract_path(&intent.url);
        self.resolve_path(&path)
    }
}

impl crate::route_handler::RouteHandler for PatternRouter {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        _session: &crate::session::PlatformSession,
    ) -> Option<RouteDecision> {
        self.resolve_intent(intent)
    }
}

// ── URL path extraction ──────────────────────────────────────────────

/// Extract the path component from a URL string.
///
/// ```
/// use foundation_platform::pattern::extract_path;
/// assert_eq!(extract_path("ewe://localhost/app/items?proto=arrow"), "/app/items");
/// assert_eq!(extract_path("https://example.com/remote/dashboard"), "/remote/dashboard");
/// assert_eq!(extract_path("/app/items"), "/app/items");
/// ```
pub fn extract_path(url: &str) -> String {
    // Find first '/' after scheme + authority, or use as-is for bare paths.
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(path_start) = after_scheme.find('/') {
            let path_and_query = &after_scheme[path_start..];
            if let Some(query_start) = path_and_query.find('?') {
                return path_and_query[..query_start].to_string();
            }
            return path_and_query.to_string();
        }
        // URL with scheme but no path: ewe://localhost → "/"
        return "/".to_string();
    }

    // Bare path: /app/items?proto=arrow
    if let Some(query_start) = url.find('?') {
        return url[..query_start].to_string();
    }
    url.to_string()
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::RouteDecisionExt;

    // ── Pattern construction ──────────────────────────────────────

    #[test]
    fn empty_pattern() {
        let p = Pattern::new("").unwrap();
        assert!(p.matches(""));
        assert!(p.matches("/"));
        assert!(!p.matches("/anything"));
    }

    #[test]
    fn root_pattern() {
        let p = Pattern::new("/").unwrap();
        assert!(p.matches(""));
        assert!(p.matches("/"));
        assert!(!p.matches("/app"));
    }

    #[test]
    fn exact_match() {
        let p = Pattern::new("/app/items").unwrap();
        assert!(p.matches("/app/items"));
        assert!(p.matches("app/items"));
        assert!(!p.matches("/app/other"));
        assert!(!p.matches("/app/items/detail"));
    }

    #[test]
    fn single_segment() {
        let p = Pattern::new("/app").unwrap();
        assert!(p.matches("/app"));
        assert!(!p.matches("/app/items"));
    }

    // ── Wildcards ─────────────────────────────────────────────────

    #[test]
    fn single_wildcard_matches_one_segment() {
        let p = Pattern::new("/app/*/detail").unwrap();
        assert!(p.matches("/app/items/detail"));
        assert!(p.matches("/app/42/detail"));
        assert!(p.matches("/app/foo-bar/detail"));
        assert!(!p.matches("/app/items")); // missing /detail
        assert!(!p.matches("/app/items/sub/detail")); // * = one segment, not two
        assert!(!p.matches("/other/items/detail")); // /app doesn't match
    }

    #[test]
    fn multiple_single_wildcards() {
        let p = Pattern::new("/*/*/detail").unwrap();
        assert!(p.matches("/foo/bar/detail"));
        assert!(!p.matches("/foo/detail")); // only one wildcard-matched segment
        assert!(!p.matches("/foo/bar/baz/detail")); // three segments before detail
    }

    #[test]
    fn double_wildcard_matches_zero_or_more() {
        let p = Pattern::new("/static/**").unwrap();
        assert!(p.matches("/static")); // ** matches zero segments
        assert!(p.matches("/static/css/main.css"));
        assert!(p.matches("/static/js/vendor/lib.js"));
        assert!(p.matches("/static/a/b/c/d/e"));
        assert!(!p.matches("/other/css/main.css")); // /static doesn't match
    }

    #[test]
    fn double_wildcard_with_prefix() {
        let p = Pattern::new("/app/**").unwrap();
        assert!(p.matches("/app"));
        assert!(p.matches("/app/items"));
        assert!(p.matches("/app/items/detail"));
        assert!(!p.matches("/api/items")); // /app doesn't match
    }

    #[test]
    fn trailing_wildcard() {
        let p = Pattern::new("/app/*").unwrap();
        assert!(p.matches("/app/items"));
        assert!(p.matches("/app/dashboard"));
        assert!(!p.matches("/app/items/detail")); // * only matches one segment
    }

    // ── Pattern validation ────────────────────────────────────────

    #[test]
    fn double_wildcard_must_be_last() {
        assert!(Pattern::new("/app/**/detail").is_err());
        assert!(Pattern::new("/**/app").is_err());
    }

    #[test]
    fn only_one_double_wildcard() {
        assert!(Pattern::new("/app/**/sub/**").is_err());
    }

    #[test]
    fn partial_wildcards_rejected() {
        assert!(Pattern::new("/app/file*").is_err());
        assert!(Pattern::new("/app/*.html").is_err());
        assert!(Pattern::new("/app/file**.txt").is_err());
    }

    // ── Pattern display ───────────────────────────────────────────

    #[test]
    fn display_roundtrips() {
        for pat in &["/app/*/detail", "/static/**", "/app/items", "/auth/*"] {
            let p = Pattern::new(pat).unwrap();
            assert_eq!(p.to_string(), *pat);
        }
    }

    // ── URL extraction ────────────────────────────────────────────

    #[test]
    fn extract_path_from_ewe_url() {
        assert_eq!(extract_path("ewe://localhost/app/items?proto=arrow"), "/app/items");
        assert_eq!(extract_path("ewe://localhost/remote/dashboard"), "/remote/dashboard");
        assert_eq!(extract_path("ewe://localhost/"), "/");
    }

    #[test]
    fn extract_path_from_https_url() {
        assert_eq!(extract_path("https://example.com/path"), "/path");
        assert_eq!(extract_path("https://example.com/path?a=1&b=2"), "/path");
    }

    #[test]
    fn extract_path_bare() {
        assert_eq!(extract_path("/app/items"), "/app/items");
        assert_eq!(extract_path("/app/items?proto=arrow"), "/app/items");
    }

    // ── PatternRouter ─────────────────────────────────────────────

    fn intent(path: &str) -> NavigationIntent {
        NavigationIntent {
            url: format!("ewe://localhost{path}"),
            method: Method::Get,
            source: IntentSource::LinkClick,
            referrer: None,
        }
    }

    #[test]
    fn router_first_match_wins() {
        let mut router = PatternRouter::new();
        router.route("/app/*", crate::route::webview_app());
        router.route("/app/special", crate::route::remote_fetch());

        // Both patterns match "/app/special" — first registered wins
        let d = router.resolve_intent(&intent("/app/special")).unwrap();
        assert_eq!(d.source, RouteSource::WebviewApp);
    }

    #[test]
    fn router_no_match_returns_none() {
        let mut router = PatternRouter::new();
        router.route("/app/*", crate::route::webview_app());

        let d = router.resolve_intent(&intent("/remote/dashboard"));
        assert!(d.is_none());
    }

    #[test]
    fn router_multiple_patterns() {
        let mut router = PatternRouter::new();
        router.route("/app/*", crate::route::webview_app());
        router.route("/remote/*", crate::route::remote_fetch());
        router.route("/cached/*", crate::route::remote_fetch()
            .with_cache_policy(CachePolicy::CacheFirst));

        assert_eq!(
            router.resolve_intent(&intent("/app/items")).unwrap().source,
            RouteSource::WebviewApp
        );
        assert_eq!(
            router.resolve_intent(&intent("/remote/dashboard")).unwrap().source,
            RouteSource::RemoteServer
        );
        assert_eq!(
            router.resolve_intent(&intent("/cached/data")).unwrap().cache_policy,
            CachePolicy::CacheFirst
        );
    }

    #[test]
    fn router_as_route_handler() {
        use crate::route_handler::RouteHandler;

        let mut router = PatternRouter::new();
        router.route("/app/*", crate::route::webview_app());

        let session = crate::session::PlatformSession::new();
        let d = router.resolve(&intent("/app/dashboard"), &session).unwrap();
        assert_eq!(d.source, RouteSource::WebviewApp);
    }
}
