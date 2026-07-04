# 02 — Route policy: `RouteHandler` trait with three composable APIs

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Route policy is code, not a config file. The platform intercepts every
navigation intent through the session backbone and delegates decisions to
user-provided handlers.

Three API surfaces compose together, all implementing a single trait:

**A — `RouteHandler` trait (core abstraction):**

```rust
trait RouteHandler: Send + Sync + 'static {
    fn resolve(
        &self,
        intent: &NavigationIntent,
        session: &PlatformSession,
    ) -> Option<RouteDecision>;
}
```

Handlers return `Some(decision)` to claim a navigation, or `None` to fall
through to the next handler. This is the only hard requirement — patterned
after `foundation_http`'s `Serve` trait: trait object, user implements,
registers with the platform.

**B — Closure convenience (wraps A):**

```rust
session.on_navigate(|intent, _session| {
    if intent.url.starts_with("/app/") {
        Some(RouteDecision::local_wasm())
    } else {
        None
    }
});
```

A `FnRouteHandler(F)` wrapper impls `RouteHandler` for any matching closure.

**C — Declarative pattern table (wraps A):**

```rust
session.route("/app/*", RouteDecision::local_wasm());
session.route("/remote/*", RouteDecision::remote_fetch());
```

A `PatternRouter` struct impls `RouteHandler` — iterates registered patterns,
returns first match. Handles the common case where navigation intent maps
directly to a known destination.

### How they compose

B and C are `RouteHandler` impls registered in the session's handler chain.
On navigation, the session iterates handlers in registration order; the first
`Some(decision)` wins. If no handler claims the navigation, the platform
default applies (treat as external, open in system browser, or a
user-configured fallback).

### Why code, not config

Route policy needs arbitrary logic: database queries, auth state checks,
session context inspection, conditional branching. A config file constrains
decisions to what can be expressed in a static format. The trait approach
lets users run Rust. Server-provided policy hints (stored in the session's
path configuration, like Hotwire Native's `PathConfiguration`) can still
influence handler decisions, but the handler is the final authority — local
compiled policy overrides server suggestions.

### Why three APIs

Users have different needs. A simple app with local vs remote routes just
wants pattern matching (C). An app with per-route auth checks or dynamic
behavior wants a closure (B). A complex app with shared decision logic across
screens wants a trait impl (A). All three are the same type system underneath.

### What `RouteDecision` carries

```rust
struct RouteDecision {
    source: RouteSource,        // local-wasm | ipc-shell | remote-server | cache
    presentation: Presentation, // morph | native-push | modal | replace | external
    render_mode: RenderMode,    // wasm-app | html-doc | domops-stream | fragment-morph
    protocol: ProtocolHint,     // columnar | arrow-ipc | json | html | default
    cache_policy: CachePolicy,  // cache-first | network-first | local-only | online-only
    capabilities: Vec<CapabilityId>, // native capabilities allowed on this route
}
```

### Where it lives

Route handler traits and `RouteDecision` types live in `foundation_platform`.
`foundation_wasm_ui` does not depend on them — the platform calls the handler,
gets a `RouteDecision`, and executes the plan through the appropriate
transport and rendering lane.
