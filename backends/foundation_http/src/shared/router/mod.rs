//! Generic router — `Router<S>` stores handlers of type `S` in a route tree.
//!
//! The route tree (`RouteSegment<S>`) is kept as-is — matching logic unchanged.
//! The handler type `S` flows from `HttpApp<S>` through `Router<S>` down to
//! `RouteSegment<S>` and `RouteMethod<S>`.

mod declared;
pub use declared::*;
pub mod method;
pub mod segments;

use foundation_netio::shared::http::SimpleMethod;

pub use method::RouteMethod;
pub use segments::{ParamStaticValidation, RouteOp, RouteResult, RouteSegment, SegmentType};

/// Declares the method and paths of a handler.
pub trait Route {
    const METHOD: SimpleMethod;
    const PATH: &'static str;
    const ALIASES: &'static [&'static str] = &[];
}

/// Router with a root route segment tree, generic over handler type `S`.
pub struct Router<S> {
    root: RouteSegment<S>,
    fallback: Option<S>,
}

impl<S> Router<S> {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Router {
            root: RouteSegment::root(),
            fallback: None,
        }
    }

    /// Set the handler for every path that no route matches.
    ///
    /// A catch-all server (for example a Durable Streams adapter, where every path is a stream)
    /// registers one fallback.
    pub fn set_fallback(&mut self, handler: S) {
        self.fallback = Some(handler);
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

    fn insert_paths<'a>(
        &mut self,
        method: &SimpleMethod,
        paths: impl Iterator<Item = &'a str>,
        handler: S,
    ) -> RouteResult<()>
    where
        S: Clone,
    {
        let segments: Vec<_> = paths
            .map(|path| {
                if !path.starts_with('/') {
                    return Err(RouteOp::InvalidSegment);
                }
                RouteSegment::parse_route(path)
            })
            .collect::<RouteResult<_>>()?;
        for segment in segments {
            self.root
                .merge_route(segment, method.clone(), handler.clone());
        }
        Ok(())
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
        self.dispatch_result(method, path).ok()
    }

    /// Dispatch a request and keep the reason of a miss.
    ///
    /// WHY: a path that matches a route node but has no handler for the request
    /// method is a 405, while a path that matches no node is a 404. The caller
    /// needs both answers.
    /// WHAT: the matched handler, or [`RouteOp::MethodNotAllowed`] / another
    /// [`RouteOp`] for the miss.
    /// HOW: `match_route` checks the path first, then the method.
    ///
    /// # Errors
    ///
    /// Returns [`RouteOp::MethodNotAllowed`] when the path matched but the
    /// method did not. Returns another [`RouteOp`] when no path matched.
    pub fn dispatch_result(&self, method: &SimpleMethod, path: &str) -> RouteResult<S>
    where
        S: Clone,
    {
        let path_without_query = path.split('?').next().unwrap_or(path);
        match self.root.match_route(method, path_without_query) {
            Ok(handler) => Ok(handler),
            // A known path with a wrong method is a 405, not a fallback miss.
            Err(RouteOp::MethodNotAllowed) => Err(RouteOp::MethodNotAllowed),
            Err(_) => self
                .fallback
                .clone()
                .ok_or_else(|| RouteOp::NoMatchingRoute(path_without_query.to_string())),
        }
    }

    /// The methods that `path` supports, for the `Allow` header of a 405.
    ///
    /// Query strings are stripped, as in [`Self::dispatch`]. An empty list
    /// means no path matched.
    #[must_use]
    pub fn allowed_methods(&self, path: &str) -> Vec<String>
    where
        S: Clone,
    {
        let path_without_query = path.split('?').next().unwrap_or(path);
        self.root.allowed_methods(path_without_query)
    }
}

impl<S> Default for Router<S> {
    fn default() -> Self {
        Self::new()
    }
}
