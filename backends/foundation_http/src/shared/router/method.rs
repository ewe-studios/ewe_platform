//! `RouteMethod<S>` — stores a handler per HTTP method.

use foundation_netio::shared::http::SimpleMethod;

use super::segments::{RouteResult, RouteOp};

/// Stores handlers for each HTTP method at a route leaf.
pub struct RouteMethod<S> {
    get: Option<S>,
    post: Option<S>,
    put: Option<S>,
    delete: Option<S>,
    patch: Option<S>,
    head: Option<S>,
    options: Option<S>,
    connect: Option<S>,
    trace: Option<S>,
    custom: Option<(String, S)>,
}

impl<S: Clone> Clone for RouteMethod<S> {
    fn clone(&self) -> Self {
        RouteMethod {
            get: self.get.clone(),
            post: self.post.clone(),
            put: self.put.clone(),
            delete: self.delete.clone(),
            patch: self.patch.clone(),
            head: self.head.clone(),
            options: self.options.clone(),
            connect: self.connect.clone(),
            trace: self.trace.clone(),
            custom: self.custom.clone(),
        }
    }
}

impl<S> Default for RouteMethod<S> {
    fn default() -> Self {
        Self {
            get: None, post: None, put: None, delete: None, patch: None,
            head: None, options: None, connect: None, trace: None,
            custom: None,
        }
    }
}

impl<S> RouteMethod<S> {
    /// Create an empty `RouteMethod` with no handlers.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Set the handler for a specific HTTP method.
    pub fn set_method(&mut self, method: SimpleMethod, handler: S) {
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
    pub fn get_method(&self, method: &SimpleMethod) -> RouteResult<S>
    where
        S: Clone,
    {
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
    pub fn take(&mut self, other: Self) {
        if self.get.is_none() { self.get = other.get; }
        if self.post.is_none() { self.post = other.post; }
        if self.put.is_none() { self.put = other.put; }
        if self.delete.is_none() { self.delete = other.delete; }
        if self.patch.is_none() { self.patch = other.patch; }
        if self.head.is_none() { self.head = other.head; }
        if self.options.is_none() { self.options = other.options; }
        if self.connect.is_none() { self.connect = other.connect; }
        if self.trace.is_none() { self.trace = other.trace; }
        if self.custom.is_none() { self.custom = other.custom; }
    }
}
