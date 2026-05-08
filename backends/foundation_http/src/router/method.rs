//! `RouteMethod` — stores a handler (`ArcServe`) per HTTP method.
//!
//! Replaces the old generic `RouteMethod<R, S, Server>` from `ewe_routing`.
//! Each leaf in the route tree holds one handler per method.

use std::sync::Arc;

use foundation_core::wire::simple_http::SimpleMethod;

use crate::serve::Serve;

use super::segments::{RouteResult, RouteOp};

/// Handler type stored per method.
pub type ArcServe = Arc<dyn Serve>;

/// Stores handlers for each HTTP method at a route leaf.
#[derive(Clone, Default)]
pub struct RouteMethod {
    get: Option<ArcServe>,
    post: Option<ArcServe>,
    put: Option<ArcServe>,
    delete: Option<ArcServe>,
    patch: Option<ArcServe>,
    head: Option<ArcServe>,
    options: Option<ArcServe>,
    connect: Option<ArcServe>,
    trace: Option<ArcServe>,
    custom: Option<(String, ArcServe)>,
}

impl RouteMethod {
    /// Create an empty `RouteMethod` with no handlers.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set the handler for a specific HTTP method.
    pub fn set_method(&mut self, method: SimpleMethod, handler: ArcServe) {
        match method {
            SimpleMethod::GET => self.get = Some(handler),
            SimpleMethod::POST => self.post = Some(handler),
            SimpleMethod::PUT => self.put = Some(handler),
            SimpleMethod::DELETE => self.delete = Some(handler),
            SimpleMethod::PATCH => self.patch = Some(handler),
            SimpleMethod::HEAD => self.head = Some(handler),
            SimpleMethod::OPTIONS => self.options = Some(handler),
            SimpleMethod::CONNECT => self.connect = Some(handler),
            SimpleMethod::TRACE => self.trace = Some(handler),
            SimpleMethod::Custom(s) => self.custom = Some((s, handler)),
        }
    }

    /// Get the handler for a specific HTTP method.
    ///
    /// # Errors
    ///
    /// Returns `RouteOp::NoMatchingRoute` if no handler is registered
    /// for the given method.
    #[must_use]
    pub fn get_method(&self, method: &SimpleMethod) -> RouteResult<ArcServe> {
        let opt = match method {
            SimpleMethod::GET => &self.get,
            SimpleMethod::POST => &self.post,
            SimpleMethod::PUT => &self.put,
            SimpleMethod::DELETE => &self.delete,
            SimpleMethod::PATCH => &self.patch,
            SimpleMethod::HEAD => &self.head,
            SimpleMethod::OPTIONS => &self.options,
            SimpleMethod::CONNECT => &self.connect,
            SimpleMethod::TRACE => &self.trace,
            SimpleMethod::Custom(s) => {
                if let Some((name, handler)) = &self.custom {
                    if name == s {
                        return Ok(handler.clone());
                    }
                }
                return Err(RouteOp::NoMatchingRoute(format!("method {s}")));
            }
        };

        opt.clone().ok_or_else(|| RouteOp::NoMatchingRoute(format!("{method:?}")))
    }

    /// Returns true if any HTTP method has a handler registered.
    #[must_use]
    pub fn has_any(&self) -> bool {
        self.get.is_some()
            || self.post.is_some()
            || self.put.is_some()
            || self.delete.is_some()
            || self.patch.is_some()
            || self.head.is_some()
            || self.options.is_some()
            || self.connect.is_some()
            || self.trace.is_some()
            || self.custom.is_some()
    }

    /// Take all handlers from another `RouteMethod` and merge them into self.
    ///
    /// Only overwrites methods that are not already set in self.
    pub fn take(&mut self, other: Self) {
        if self.get.is_none() {
            self.get = other.get;
        }
        if self.post.is_none() {
            self.post = other.post;
        }
        if self.put.is_none() {
            self.put = other.put;
        }
        if self.delete.is_none() {
            self.delete = other.delete;
        }
        if self.patch.is_none() {
            self.patch = other.patch;
        }
        if self.head.is_none() {
            self.head = other.head;
        }
        if self.options.is_none() {
            self.options = other.options;
        }
        if self.connect.is_none() {
            self.connect = other.connect;
        }
        if self.trace.is_none() {
            self.trace = other.trace;
        }
        if self.custom.is_none() {
            self.custom = other.custom;
        }
    }
}
