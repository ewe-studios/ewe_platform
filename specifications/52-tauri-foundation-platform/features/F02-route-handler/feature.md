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
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F02 — Route handler trait and pattern router

## Overview

Define the `RouteHandler` trait, implement three API surfaces (trait impl,
closure, `PatternRouter`), wire them into the session backbone. Route handlers
are the core user-facing API — every navigation flows through them.

[Decision 02](../decisions/02-route-policy-model.md) defines the full route
policy model.

## Dependencies

Depends on:
- `F01-session-backbone` — Handlers register with `PlatformSession`

Required by:
- `F03-ewe-protocol` — Protocol handler routes URLs through handler chain
- `F08-walking-skeleton` — End-to-end navigation flow

## Requirements

### 1. `RouteHandler` trait

Patterned after `foundation_http`'s `Serve` trait:

```rust
// foundation_platform/src/route.rs

pub trait RouteHandler: Send + Sync + 'static {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}
```

Return `Some(decision)` to claim the navigation, `None` to fall through.

### 2. `PatternRouter` — declarative pattern table

```rust
pub struct PatternRouter {
    patterns: Vec<(Pattern, RouteDecision)>,
}

impl PatternRouter {
    pub fn new() -> Self { ... }
    pub fn route(&mut self, pattern: &str, decision: RouteDecision) { ... }
}

impl RouteHandler for PatternRouter {
    fn resolve(&self, intent: &NavigationIntent, session: &PlatformSession) -> Option<RouteDecision> {
        // Iterate patterns in registration order. First match wins.
        for (pattern, decision) in &self.patterns {
            if pattern.matches(intent.url.path()) {
                return Some(decision.clone());
            }
        }
        None
    }
}
```

Pattern syntax:
- `*` matches a single path segment
- `**` matches any depth
- Exact path segments match literally

```rust
// Example: "/app/*/detail" matches "/app/items/detail" but not "/app/items/sub/detail"
// Example: "/static/**" matches "/static/css/main.css" and "/static/js/vendor/lib.js"
```

### 3. `FnRouteHandler` — closure convenience

```rust
pub struct FnRouteHandler<F: Fn(&NavigationIntent, &PlatformSession) -> Option<RouteDecision> + Send + Sync + 'static>(F);

impl<F> RouteHandler for FnRouteHandler<F> { ... }

// Usage:
session.on_navigate(|intent, session| {
    if intent.url.path().starts_with("/app/") {
        Some(RouteDecision::webview_app())
    } else {
        None
    }
});
```

### 4. Session integration: `session.route()` shorthand

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a pattern-based route handler (wraps PatternRouter).
    pub fn route(&self, pattern: &str, decision: RouteDecision) { ... }

    /// Register a closure-based route handler (wraps FnRouteHandler).
    pub fn on_navigate<F>(&self, f: F) where F: Fn(...) -> Option<RouteDecision> + ... { ... }
}
```

### 5. Pattern matching implementation

```rust
struct Pattern { segments: Vec<PatternSegment> }

enum PatternSegment {
    Literal(String),    // exact match
    SingleWildcard,     // * — single segment
    DoubleWildcard,     // ** — any depth
}

impl Pattern {
    fn matches(&self, path: &str) -> bool {
        let path_segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        self.match_segments(&path_segments, 0, 0)
    }

    fn match_segments(&self, path: &[&str], pat_idx: usize, path_idx: usize) -> bool {
        // Base case: consumed all pattern segments
        if pat_idx >= self.segments.len() {
            return path_idx >= path.len();
        }
        match &self.segments[pat_idx] {
            PatternSegment::DoubleWildcard => {
                // ** matches zero or more segments
                for i in path_idx..=path.len() {
                    if self.match_segments(path, pat_idx + 1, i) {
                        return true;
                    }
                }
                false
            }
            PatternSegment::SingleWildcard => {
                // * matches exactly one segment
                path_idx < path.len()
                    && self.match_segments(path, pat_idx + 1, path_idx + 1)
            }
            PatternSegment::Literal(lit) => {
                path_idx < path.len()
                    && path[path_idx] == lit
                    && self.match_segments(path, pat_idx + 1, path_idx + 1)
            }
        }
    }
}
```

## Architecture

```
NavigationIntent arrives
  │
  ▼
session.resolve_route(&intent)
  │
  ├── Handler 0: MyAppRouter (trait impl) → checks DB, auth → Some/Nothing
  ├── Handler 1: FnRouteHandler (closure)  → simple path check → Some/Nothing
  ├── Handler 2: PatternRouter             → pattern match      → Some/Nothing
  └── Platform default                     → external browser
  │
  ▼
RouteDecision returned → session executes it
```

## Tasks

### RouteHandler trait
- [ ] Define `RouteHandler` trait with `resolve()` method
- [ ] Make it `Send + Sync + 'static` for thread-safe registration

### PatternRouter
- [ ] Implement `Pattern` struct with `PatternSegment` enum
- [ ] Implement glob matching (`*` and `**`)
- [ ] Implement `PatternRouter` struct with `route()` and `resolve()`
- [ ] Test: exact path matches
- [ ] Test: single wildcard `*` matches
- [ ] Test: double wildcard `**` matches
- [ ] Test: first-registered-first-matched ordering

### FnRouteHandler
- [ ] Implement `FnRouteHandler` wrapping a closure
- [ ] Test: closure captures external state (db handle, auth manager)

### Session integration
- [ ] Add `route()` shorthand to `PlatformSession`
- [ ] Add `on_navigate()` shorthand to `PlatformSession`
- [ ] Test: multiple handlers registered, priority ordering verified

## Verification Commands

```bash
cargo test --package foundation_platform -- route
```
