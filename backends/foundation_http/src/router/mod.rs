//! Simplified router migrated from `ewe_routing`.
//!
//! The route tree (`RouteSegment`) is kept as-is — matching logic unchanged.
//! The `RouteMethod` dispatch now stores `ArcServe` instead of the generic `Servicer`.
//! All async/tower/axum dependencies are removed.

mod segments;
mod method;

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleMethod;

use crate::serve::Serve;

pub use segments::{RouteOp, RouteResult, RouteSegment, SegmentType, ParamStaticValidation};
pub use method::RouteMethod;

/// Type alias for an arc-backed handler.
pub type ArcServe = Arc<dyn Serve>;

/// Router with a root route segment tree.
///
/// Returns `Option<ArcServe>` from dispatch — the worker calls `Serve::serve` directly.
pub struct Router {
    root: RouteSegment,
}

impl Router {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Router {
            root: RouteSegment::root(),
        }
    }

    /// Register a handler for a specific HTTP method and path.
    ///
    /// The path supports: static segments (`/users`), params (`:id`),
    /// restricted params (`:id::numbers`), regex params (`:id::(\d+)`),
    /// regex-only segments (`(\w+)`), and wildcard (`/*`).
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route(&mut self, method: SimpleMethod, path: &str, handler: ArcServe) {
        // Build a RouteSegment tree for this path
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");

        // Walk or extend the existing tree to merge this route
        self.root.merge_route(segment_tree, method, handler);
    }

    /// Register a handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any(&mut self, path: &str, handler: ArcServe) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(segment_tree, handler);
    }

    /// Dispatch a request to the matched handler.
    ///
    /// Returns `Some(ArcServe)` if a route matches, `None` otherwise.
    #[must_use]
    pub fn dispatch(&self, method: &SimpleMethod, path: &str) -> Option<ArcServe> {
        self.root.match_route(method, path).ok()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
