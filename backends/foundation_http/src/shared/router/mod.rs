//! Generic router — `Router<S>` stores handlers of type `S` in a route tree.
//!
//! The route tree (`RouteSegment<S>`) is kept as-is — matching logic unchanged.
//! The handler type `S` flows from `HttpApp<S>` through `Router<S>` down to
//! `RouteSegment<S>` and `RouteMethod<S>`.

pub mod segments;
pub mod method;

use foundation_netio::simple_http::SimpleMethod;

pub use segments::{RouteOp, RouteResult, RouteSegment, SegmentType, ParamStaticValidation};
pub use method::RouteMethod;

/// Router with a root route segment tree, generic over handler type `S`.
pub struct Router<S> {
    root: RouteSegment<S>,
}

impl<S> Router<S> {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Router {
            root: RouteSegment::root(),
        }
    }

    /// Register a handler for a specific HTTP method and path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route(&mut self, method: SimpleMethod, path: &str, handler: S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, handler);
    }

    /// Register a handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any(&mut self, path: &str, handler: &S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(&segment_tree, handler);
    }

    /// Register a writer handler for a specific HTTP method and path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_writer(&mut self, method: SimpleMethod, path: &str, handler: S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, handler);
    }

    /// Register a writer handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any_writer(&mut self, path: &str, handler: &S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(&segment_tree, handler);
    }

    /// Register a CF handler for a specific HTTP method and path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_cf(&mut self, method: SimpleMethod, path: &str, handler: S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, handler);
    }

    /// Register a CF handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any_cf(&mut self, path: &str, handler: &S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(&segment_tree, handler);
    }

    /// Register a web handler for a specific HTTP method and path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_web(&mut self, method: SimpleMethod, path: &str, handler: S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, handler);
    }

    /// Register a web handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any_web(&mut self, path: &str, handler: &S)
    where
        S: Clone,
    {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(&segment_tree, handler);
    }

    /// Dispatch a request to the matched handler.
    ///
    /// Query strings are stripped from the path before matching so that
    /// `/search?q=rust` routes to `/search`.
    #[must_use]
    pub fn dispatch(&self, method: &SimpleMethod, path: &str) -> Option<S>
    where
        S: Clone,
    {
        let path_without_query = path.split('?').next().unwrap_or(path);
        self.root.match_route(method, path_without_query).ok()
    }
}

impl<S> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}
