//! Simplified router migrated from `ewe_routing`.
//!
//! The route tree (`RouteSegment`) is kept as-is — matching logic unchanged.
//! The `RouteMethod` dispatch stores `Server` (cfg-gated `Serve` or `Writer`).

mod segments;
mod method;

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleMethod;

pub use segments::{RouteOp, RouteResult, RouteSegment, SegmentType, ParamStaticValidation};
pub use method::RouteMethod;

/// Type alias for an arc-backed Serve handler (native only).
#[cfg(not(target_arch = "wasm32"))]
pub use segments::ArcServe;

/// Handler server type — native uses `Serve` (TCP), wasm uses `Writer` (buffer).
pub enum Server {
    #[cfg(not(target_arch = "wasm32"))]
    Serve(Arc<dyn crate::shared::serve::Serve>),
    Writer(Arc<dyn crate::shared::serve::ServeWriter>),
}

impl Clone for Server {
    fn clone(&self) -> Self {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Server::Serve(arc) => Server::Serve(arc.clone()),
            Server::Writer(arc) => Server::Writer(arc.clone()),
        }
    }
}

impl Server {
    /// Dispatch to the native Serve handler.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn serve(
        &self,
        bag: Arc<crate::shared::context::ContextBag>,
        req: foundation_core::wire::simple_http::SimpleIncomingRequest,
        conn: foundation_core::io::ioutils::SharedByteBufferStream<foundation_core::netcap::RawStream>,
    ) -> Option<crate::shared::serve::ConnectionResult> {
        match self {
            Server::Serve(handler) => Some(handler.serve(bag, req, conn)),
            Server::Writer(_) => None,
        }
    }

    /// Get the writer variant, or `None` if this is a native Serve variant.
    pub fn as_writer(&self) -> Option<&Arc<dyn crate::shared::serve::ServeWriter>> {
        match self {
            Server::Writer(w) => Some(w),
            #[cfg(not(target_arch = "wasm32"))]
            Server::Serve(_) => None,
        }
    }
}

/// Router with a root route segment tree.
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
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_route(&mut self, method: SimpleMethod, path: &str, handler: Arc<dyn crate::shared::serve::Serve>) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, Server::Serve(handler));
    }

    /// Register a handler for all HTTP methods on a path (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_route_any(&mut self, path: &str, handler: &Arc<dyn crate::shared::serve::Serve>) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        let server = Server::Serve(handler.clone());
        self.root.merge_route_all_methods(&segment_tree, &server);
    }

    /// Register a writer handler for a specific HTTP method and path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_writer(&mut self, method: SimpleMethod, path: &str, handler: Arc<dyn crate::shared::serve::ServeWriter>) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route(segment_tree, method, Server::Writer(handler));
    }

    /// Register a writer handler for all HTTP methods on a path.
    ///
    /// # Panics
    ///
    /// Panics if the path string is not a valid route pattern.
    pub fn add_route_any_writer(&mut self, path: &str, handler: &Arc<dyn crate::shared::serve::ServeWriter>) {
        let segment_tree = RouteSegment::parse_route(path).expect("valid route pattern");
        self.root.merge_route_all_methods(&segment_tree, &Server::Writer(handler.clone()));
    }

    /// Dispatch a request to the matched handler.
    ///
    /// Query strings are stripped from the path before matching so that
    /// `/search?q=rust` routes to `/search`.
    #[must_use]
    pub fn dispatch(&self, method: &SimpleMethod, path: &str) -> Option<Server> {
        let path_without_query = path.split('?').next().unwrap_or(path);
        self.root.match_route(method, path_without_query).ok()
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
