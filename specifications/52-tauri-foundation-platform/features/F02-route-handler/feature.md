---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F02-route-handler"
this_file: "specifications/52-tauri-foundation-platform/features/F02-route-handler/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 24
  total: 24
  completion_percentage: 0%
---

# F02 — Route handler trait and PatternRouter

## Overview

Define the `RouteHandler` trait, implement three API surfaces (trait impl,
closure via `FnRouteHandler`, `PatternRouter`), implement pattern matching
with `*` and `**` globs, and wire all three into the session backbone.

[Decision 02](../decisions/02-route-policy-model.md) sections 1-2 define
the trait and API surfaces. Section 3 defines the execution contract that
traverses the handler chain. This feature implements the trait and chain
execution; the execution contract (steps 3-9) spans F03, F06, F07, and F09.

## Dependencies

Depends on:
- `F01-session-backbone` — Handlers register with `PlatformSession` via
  `register_handler()`, `resolve_route()` iterates them.

Required by:
- `F03-ewe-protocol` — Protocol handler resolves URLs through handler chain
- `F09-walking-skeleton` — End-to-end navigation

---

## Part A — `RouteHandler` trait

### A.1 — Trait definition

```rust
// foundation_platform/src/route_handler.rs

use foundation_ui_traits::*;

/// Route policy is code, not a config file. The platform intercepts every
/// navigation intent and delegates decisions to user-provided handlers.
///
/// Patterned after `foundation_http`'s `Serve` trait: trait object,
/// user implements, registers with the platform.
///
/// Handlers return `Some(decision)` to claim a navigation, or `None`
/// to fall through to the next handler in the chain. This is the only
/// hard requirement — everything else (what logic runs inside `resolve`)
/// is up to the user.
pub trait RouteHandler: Send + Sync + 'static {
    /// Resolve a navigation intent into a route decision, or pass.
    ///
    /// # Arguments
    /// * `intent` — The intercepted navigation (URL, method, source, referrer).
    ///   Carries everything the handler needs to make a decision.
    /// * `session` — The platform session. Handlers can inspect session state
    ///   (auth, database, active page) to inform their decision.
    ///
    /// # Returns
    /// * `Some(decision)` — "I'll handle this navigation." The platform
    ///   executes the decision (cache check, presentation, backend query,
    ///   protocol selection, transport, rendering).
    /// * `None` — "Pass." The session continues to the next handler.
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}
```

### A.2 — Why `Send + Sync + 'static`

From the decision: handlers are stored in `Vec<Box<dyn RouteHandler>>` behind
an `RwLock`. They're accessed from multiple threads (navigation callbacks
fire on the main thread; capability invocations may fire from background
tasks). `Send + Sync + 'static` is the minimum bound for shared state.

### A.3 — Handler chain execution

The session iterates handlers in registration order. First `Some(decision)`
wins. If no handler claims the navigation, the platform default applies.

```rust
// In PlatformSession::resolve_route() — implemented in F01:

fn resolve_route(&self, intent: &NavigationIntent) -> RouteDecision {
    let handlers = self.route_handlers.read().unwrap();
    for handler in handlers.iter() {
        if let Some(decision) = handler.resolve(intent, self) {
            return decision;  // first match wins
        }
    }
    // Platform default: external URLs → system browser,
    // same-origin with no handler → UntrustedRemote + OnlineOnly
    self.default_decision_for(intent)
}
```

The execution contract (from decision 02 section 4) continues after
`resolve_route` returns: cache check → presentation → backend query →
protocol selection → transport → rendering → post-render. Those steps
are implemented in F03 (protocol/transport), F06 (presentation/rendering),
F07 (cache), and F09 (end-to-end).

---

## Part B — `PatternRouter`

### B.1 — Pattern struct

```rust
// foundation_platform/src/pattern.rs

/// A URL path pattern. Used by `PatternRouter` to match routes.
///
/// Syntax:
///   - `*` matches a single path segment (e.g., `/app/*/detail`)
///   - `**` matches zero or more path segments (e.g., `/static/**`)
///   - Literal segments match exactly
///
/// Patterns are anchored at the start. `/app/*` matches `/app/items`
/// but not `/other/app/items`. Trailing `**` is implicit — no wildcard
/// at end means exact match on the number of segments.
#[derive(Debug, Clone)]
pub struct Pattern {
    segments: Vec<PatternSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PatternSegment {
    /// Exact string match (e.g., "app", "items", "detail")
    Literal(String),
    /// `*` — matches exactly one path segment
    SingleWildcard,
    /// `**` — matches zero or more path segments
    DoubleWildcard,
}
```

### B.2 — Pattern parsing

```rust
impl Pattern {
    /// Parse a pattern string into a `Pattern`.
    ///
    /// # Examples
    /// ```
    /// Pattern::new("/app/*/detail")?;
    /// Pattern::new("/static/**")?;
    /// Pattern::new("/auth/*")?;
    /// ```
    pub fn new(pattern: &str) -> Result<Self, PatternError> {
        let trimmed = pattern.trim_matches('/');
        if trimmed.is_empty() {
            return Ok(Self { segments: vec![] });
        }

        let segments: Result<Vec<_>, _> = trimmed
            .split('/')
            .map(|seg| match seg {
                "**" => Ok(PatternSegment::DoubleWildcard),
                "*" => Ok(PatternSegment::SingleWildcard),
                lit if !lit.contains('*') => Ok(PatternSegment::Literal(lit.to_string())),
                other => Err(PatternError::InvalidSegment(other.to_string())),
            })
            .collect();

        // Validate: only one `**` allowed, must be the last segment
        let double_wildcards: Vec<_> = segments
            .as_ref()
            .map_err(|_| PatternError::ParseError)?
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, PatternSegment::DoubleWildcard))
            .collect();

        match double_wildcards.len() {
            0 => {} // fine
            1 => {
                let (idx, _) = double_wildcards[0];
                if idx != segments.as_ref().unwrap().len() - 1 {
                    return Err(PatternError::DoubleWildcardNotLast);
                }
            }
            _ => return Err(PatternError::MultipleDoubleWildcards),
        }

        Ok(Self { segments: segments? })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PatternError {
    #[error("invalid pattern segment: {0}")]
    InvalidSegment(String),
    #[error("** must be the last segment")]
    DoubleWildcardNotLast,
    #[error("only one ** allowed per pattern")]
    MultipleDoubleWildcards,
    #[error("failed to parse pattern")]
    ParseError,
}
```

### B.3 — Pattern matching algorithm

```rust
impl Pattern {
    /// Test whether a URL path matches this pattern.
    ///
    /// # Examples
    /// ```
    /// let p = Pattern::new("/app/*/detail").unwrap();
    /// assert!(p.matches("/app/items/detail"));
    /// assert!(!p.matches("/app/items/sub/detail")); // * = one segment
    ///
    /// let p = Pattern::new("/static/**").unwrap();
    /// assert!(p.matches("/static/css/main.css"));
    /// assert!(p.matches("/static"));
    /// assert!(p.matches("/static/js/vendor/lib.js"));
    /// ```
    pub fn matches(&self, path: &str) -> bool {
        let path_segments: Vec<&str> = path
            .trim_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        self.match_recursive(&path_segments, 0, 0)
    }

    /// Recursive matching with backtracking for `**`.
    fn match_recursive(
        &self,
        path: &[&str],
        pat_idx: usize,
        path_idx: usize,
    ) -> bool {
        // Base case: consumed all pattern segments
        if pat_idx >= self.segments.len() {
            return path_idx >= path.len();
        }

        match &self.segments[pat_idx] {
            PatternSegment::DoubleWildcard => {
                // ** matches zero or more segments. Try each possibility.
                for i in path_idx..=path.len() {
                    if self.match_recursive(path, pat_idx + 1, i) {
                        return true;
                    }
                }
                false
            }
            PatternSegment::SingleWildcard => {
                // * matches exactly one segment
                path_idx < path.len()
                    && self.match_recursive(path, pat_idx + 1, path_idx + 1)
            }
            PatternSegment::Literal(lit) => {
                // Exact match on this segment
                path_idx < path.len()
                    && path[path_idx] == lit.as_str()
                    && self.match_recursive(path, pat_idx + 1, path_idx + 1)
            }
        }
    }
}
```

### B.4 — `PatternRouter` struct

```rust
/// Declarative pattern table. Implements `RouteHandler` — iterates
/// registered patterns in registration order, returns the first match.
///
/// # Example
/// ```
/// let mut router = PatternRouter::new();
/// router.route("/app/*", webview_app());
/// router.route("/remote/*", remote_fetch().with_cache_policy(CachePolicy::NetworkFirst));
/// router.route("/auth/*", remote_fetch().with_profile(Profile::Auth));
/// ```
pub struct PatternRouter {
    /// Patterns in registration order. First match wins.
    patterns: Vec<(Pattern, RouteDecision)>,
}

impl PatternRouter {
    pub fn new() -> Self {
        Self { patterns: Vec::new() }
    }

    /// Register a pattern → decision mapping.
    /// Patterns are checked in registration order.
    pub fn route(&mut self, pattern: &str, decision: RouteDecision) {
        // Pattern::new should not fail for user-provided patterns at
        // registration time — panic on invalid patterns (they're bugs).
        let pattern = Pattern::new(pattern)
            .expect("invalid route pattern");
        self.patterns.push((pattern, decision));
    }
}

impl RouteHandler for PatternRouter {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        _session: &PlatformSession,
    ) -> Option<RouteDecision> {
        // Extract the path component from the URL.
        // ewe://localhost/app/items?proto=arrow → /app/items
        let path = extract_path(&intent.url);

        for (pattern, decision) in &self.patterns {
            if pattern.matches(&path) {
                return Some(decision.clone());
            }
        }
        None
    }
}

/// Extract the path component from a URL string.
/// "ewe://localhost/app/items?proto=arrow" → "/app/items"
/// "https://example.com/remote/dashboard" → "/remote/dashboard"
fn extract_path(url: &str) -> String {
    // Find the first '/' after the scheme + host.
    // ewe://localhost/app/items → start after "ewe://localhost"
    if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        if let Some(path_start) = after_scheme.find('/') {
            let path_and_query = &after_scheme[path_start..];
            // Strip query string
            if let Some(query_start) = path_and_query.find('?') {
                return path_and_query[..query_start].to_string();
            }
            return path_and_query.to_string();
        }
    }
    // Fallback: treat the whole string as a path
    url.to_string()
}
```

---

## Part C — `FnRouteHandler` (closure convenience)

### C.1 — Wrapper struct

```rust
// foundation_platform/src/route_handler.rs

/// Wraps a closure as a `RouteHandler`. Use when the policy is simple
/// enough to inline.
///
/// # Example
/// ```
/// session.register_handler(FnRouteHandler(|intent, session| {
///     if intent.url.contains("/app/") {
///         Some(webview_app())
///     } else {
///         None
///     }
/// }));
/// ```
pub struct FnRouteHandler<F>(pub F)
where
    F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
        + Send + Sync + 'static;

impl<F> RouteHandler for FnRouteHandler<F>
where
    F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
        + Send + Sync + 'static,
{
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision> {
        (self.0)(intent, session)
    }
}
```

---

## Part D — Session integration shorthand

### D.1 — `session.route()` and `session.on_navigate()`

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a pattern-based route handler.
    /// Convenience wrapper around `PatternRouter` + `register_handler`.
    pub fn route(&self, pattern: &str, decision: RouteDecision) {
        // Lazily initialize or reuse a shared PatternRouter.
        // For MVP, each call creates a single-pattern handler.
        // Post-MVP: aggregate into a shared PatternRouter.
        let mut router = PatternRouter::new();
        router.route(pattern, decision);
        self.register_handler(router);
    }

    /// Register a closure-based route handler.
    /// Convenience wrapper around `FnRouteHandler` + `register_handler`.
    pub fn on_navigate<F>(&self, f: F)
    where
        F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision>
            + Send + Sync + 'static,
    {
        self.register_handler(FnRouteHandler(f));
    }
}
```

**Note on `session.route()` implementation:** The current design registers
a separate `PatternRouter` per call. This is correct for handler chain
semantics (first-registered wins, each router is a chain link) but creates
N `PatternRouter` instances for N routes. Post-MVP optimization: aggregate
all `session.route()` calls into a single `PatternRouter` registered once.

---

## Part E — Handler composition example

From [decision 02](../decisions/02-route-policy-model.md#how-they-compose):

```rust
// All three API surfaces compose. Handlers fire in registration order.
// First match wins.

// A — Trait impl (full control, shared state)
struct MyAppRouter {
    db: DatabaseHandle,
    auth: AuthManager,
}

impl RouteHandler for MyAppRouter {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision> {
        let user = self.auth.current_user(session)?;

        if intent.url.contains("/app/") {
            Some(webview_app().with_profile(Profile::App))
        } else if intent.url.contains("/remote/") {
            let profile = if user.is_authenticated() {
                Profile::TrustedRemote
            } else {
                Profile::UntrustedRemote
            };
            Some(remote_fetch()
                .with_profile(profile)
                .with_cache_policy(CachePolicy::NetworkFirst))
        } else {
            None // fall through
        }
    }
}

// Registration:
session.register_handler(MyAppRouter::new(db, auth));       // A
session.on_navigate(|intent, _session| {                     // B
    if intent.url.starts_with("ewe://localhost/app/") {
        Some(webview_app())
    } else {
        None
    }
});
session.route("/remote/*", remote_fetch());                   // C
session.route("/auth/*", remote_fetch()                       // C
    .with_profile(Profile::Auth));
```

---

## Verification

### Tests for Pattern matching
```rust
// tests/pattern.rs
#[test]
fn exact_path_match() {
    let p = Pattern::new("/app/items").unwrap();
    assert!(p.matches("/app/items"));
    assert!(!p.matches("/app/other"));
}

#[test]
fn single_wildcard() {
    let p = Pattern::new("/app/*/detail").unwrap();
    assert!(p.matches("/app/items/detail"));
    assert!(p.matches("/app/42/detail"));
    assert!(!p.matches("/app/items/sub/detail")); // * = one segment
}

#[test]
fn double_wildcard() {
    let p = Pattern::new("/static/**").unwrap();
    assert!(p.matches("/static/css/main.css"));
    assert!(p.matches("/static"));
    assert!(p.matches("/static/js/vendor/lib.js"));
    assert!(!p.matches("/other/css/main.css"));
}

#[test]
fn double_wildcard_must_be_last() {
    assert!(Pattern::new("/app/**/detail").is_err());
}

#[test]
fn only_one_double_wildcard() {
    assert!(Pattern::new("/app/**/sub/**").is_err());
}
```

### Tests for PatternRouter
```rust
// tests/route.rs
#[test]
fn pattern_router_first_match_wins() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());
    router.route("/app/special", remote_fetch());

    let intent = NavigationIntent {
        url: "ewe://localhost/app/special".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };

    // "/app/*" matches first → webview_app wins
    let decision = router.resolve(&intent, &session).unwrap();
    assert_eq!(decision.source, RouteSource::WebviewApp);
}

#[test]
fn pattern_router_falls_through() {
    let mut router = PatternRouter::new();
    router.route("/app/*", webview_app());

    let intent = NavigationIntent {
        url: "ewe://localhost/remote/dashboard".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };

    // "/remote/*" doesn't match "/app/*" → None
    assert!(router.resolve(&intent, &session).is_none());
}
```

### Tests for FnRouteHandler
```rust
#[test]
fn closure_handler_captures_state() {
    let allowed = vec!["/app/", "/public/"];
    let handler = FnRouteHandler(move |intent, _session| {
        if allowed.iter().any(|prefix| intent.url.contains(prefix)) {
            Some(webview_app())
        } else {
            None
        }
    });

    let intent = NavigationIntent {
        url: "ewe://localhost/app/items".into(),
        method: Method::Get,
        source: IntentSource::LinkClick,
        referrer: None,
    };

    assert!(handler.resolve(&intent, &session).is_some());
}
```

### Tests for handler chain ordering
```rust
#[test]
fn handler_chain_respects_registration_order() {
    // Register A → B → C
    // A passes on /remote/*, B claims it, C never checked
    session.register_handler(handler_a); // passes on /remote/*
    session.register_handler(handler_b); // claims /remote/*
    session.register_handler(handler_c); // never reached

    let decision = session.resolve_route(&remote_intent);
    assert_eq!(decision.source, RouteSource::RemoteServer); // from handler_b
}
```

### Verification commands
```bash
cargo test --package foundation_platform -- pattern
cargo test --package foundation_platform -- route_handler
```
