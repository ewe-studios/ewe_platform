//! Bind a declared route or an explicit method and path to its stored handler.
use super::{Route, RouteResult, Router};
use crate::{shared::serve::ServeWriter, SimpleMethod};
use std::sync::Arc;

/// Converts a handler to the representation stored in a router.
pub trait IntoHandler<Kind> {
    type Stored;
    fn into_handler(self) -> Self::Stored;
}

/// An existing shared handler.
pub struct Shared;
impl<T: ?Sized> IntoHandler<Shared> for Arc<T> {
    type Stored = Self;
    fn into_handler(self) -> Self {
        self
    }
}

/// A handler that writes to a byte destination.
pub struct Writer;
impl<T: ServeWriter + 'static> IntoHandler<Writer> for T {
    type Stored = Arc<dyn ServeWriter>;
    fn into_handler(self) -> Self::Stored {
        Arc::new(self)
    }
}

/// Pairs an HTTP method with its original handler.
pub struct MethodHandler<H> {
    method: SimpleMethod,
    handler: H,
}

macro_rules! methods {
    ($($name:ident => $method:ident),* $(,)?) => {$(
        #[doc = concat!("Pairs a handler with the ", stringify!($method), " method.")]
        pub fn $name<H>(handler: H) -> MethodHandler<H> {
            MethodHandler { method: SimpleMethod::$method, handler }
        }
    )*};
}

methods!(get => GET, post => POST, put => PUT, patch => PATCH, delete => DELETE, head => HEAD, options => OPTIONS);

impl<S: Clone> Router<S> {
    /// Registers a handler at an explicit path.
    ///
    /// # Errors
    /// Returns a route error for an invalid path.
    pub fn at<H: IntoHandler<K, Stored = S>, K>(
        &mut self,
        path: &str,
        handler: MethodHandler<H>,
    ) -> RouteResult<()> {
        self.insert_paths(
            &handler.method,
            std::iter::once(path),
            handler.handler.into_handler(),
        )
    }

    /// Registers the same handler on all declared paths.
    ///
    /// # Errors
    /// Returns a route error before insertion if any path is invalid.
    pub fn mount<H: Route + IntoHandler<K, Stored = S>, K>(
        &mut self,
        handler: H,
    ) -> RouteResult<()> {
        self.insert_paths(
            &H::METHOD,
            std::iter::once(H::PATH).chain(H::ALIASES.iter().copied()),
            handler.into_handler(),
        )
    }
}
