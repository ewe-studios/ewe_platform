//! # `ResultExt` — extension trait for `Result<T, E>`
//!
//! **WHY:** Most fallible functions return `Result<T, E>`. Callers want
//! to ergnomically convert those into `Result<T, ErrorTrace<E>>` and
//! attach context without unwrapping and rewrapping manually.
//!
//! **WHAT:** This module defines [`ResultExt`], implemented for both
//! plain errors and `ErrorTrace`. For plain errors, operations wrap in
//! a new `ErrorTrace`. For existing `ErrorTrace`, operations work directly.
//!
//! **HOW:** Two separate impls with explicit method names. Users import
//! both `ResultExt` (for plain errors) and `ErrorTraceExt` (for
//! `ErrorTrace` values).

use core::error::Error;

use crate::error_trace::ErrorTrace;

/// Extension trait for `Result<T, E>` where `E` is a plain error.
pub trait PlainResultExt<T, E> {
    /// Attach a printable context.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace`.
    fn attach<C>(self, context: C) -> Result<T, ErrorTrace<E>>
    where
        C: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static;

    /// Attach a printable context lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace`.
    fn attach_with<C, F>(self, f: F) -> Result<T, ErrorTrace<E>>
    where
        C: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> C;

    /// Attach an opaque context.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace`.
    fn attach_opaque<A>(self, context: A) -> Result<T, ErrorTrace<E>>
    where
        A: Send + Sync + 'static;

    /// Attach an opaque context lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace`.
    fn attach_opaque_with<A, F>(self, f: F) -> Result<T, ErrorTrace<E>>
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A;

    /// Change the context type.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace<D>`.
    fn change_context<D>(self, context: D) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static;

    /// Change the context type lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant, now wrapped in `ErrorTrace<D>`.
    fn change_context_with<D, F>(self, f: F) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
        F: FnOnce() -> D;
}

/// Extension trait for `Result<T, ErrorTrace<C>>`.
pub trait ErrorTraceResultExt<T, C> {
    /// Attach a printable context.
    ///
    /// # Errors
    /// Returns the original `Err` variant with attachment added.
    fn attach<A>(self, attachment: A) -> Result<T, ErrorTrace<C>>
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static;

    /// Attach a printable context lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant with attachment added.
    fn attach_with<A, F>(self, f: F) -> Result<T, ErrorTrace<C>>
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> A;

    /// Attach an opaque context.
    ///
    /// # Errors
    /// Returns the original `Err` variant with attachment added.
    fn attach_opaque<A>(self, attachment: A) -> Result<T, ErrorTrace<C>>
    where
        A: Send + Sync + 'static;

    /// Attach an opaque context lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant with attachment added.
    fn attach_opaque_with<A, F>(self, f: F) -> Result<T, ErrorTrace<C>>
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A;

    /// Change the context type.
    ///
    /// # Errors
    /// Returns the original `Err` variant with new context.
    fn change_context<D>(self, context: D) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static;

    /// Change the context type lazily.
    ///
    /// # Errors
    /// Returns the original `Err` variant with new context.
    fn change_context_with<D, F>(self, f: F) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
        F: FnOnce() -> D;
}

impl<T, E> PlainResultExt<T, E> for Result<T, E>
where
    E: Error + Send + Sync + 'static,
{
    fn attach<C>(self, context: C) -> Result<T, ErrorTrace<E>>
    where
        C: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
    {
        self.map_err(|e| ErrorTrace::new(e).attach(context))
    }

    fn attach_with<C, F>(self, f: F) -> Result<T, ErrorTrace<E>>
    where
        C: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> C,
    {
        self.map_err(|e| ErrorTrace::new(e).attach(f()))
    }

    fn attach_opaque<A>(self, context: A) -> Result<T, ErrorTrace<E>>
    where
        A: Send + Sync + 'static,
    {
        self.map_err(|e| ErrorTrace::new(e).attach_opaque(context))
    }

    fn attach_opaque_with<A, F>(self, f: F) -> Result<T, ErrorTrace<E>>
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A,
    {
        self.map_err(|e| ErrorTrace::new(e).attach_opaque(f()))
    }

    fn change_context<D>(self, context: D) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
    {
        self.map_err(|e| ErrorTrace::new(e).change_context(context))
    }

    fn change_context_with<D, F>(self, f: F) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|e| ErrorTrace::new(e).change_context(f()))
    }
}

impl<T, C> ErrorTraceResultExt<T, C> for Result<T, ErrorTrace<C>>
where
    C: Error + Send + Sync + 'static,
{
    fn attach<A>(self, attachment: A) -> Result<T, ErrorTrace<C>>
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
    {
        self.map_err(|trace| trace.attach(attachment))
    }

    fn attach_with<A, F>(self, f: F) -> Result<T, ErrorTrace<C>>
    where
        A: core::fmt::Display + core::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> A,
    {
        self.map_err(|trace| trace.attach(f()))
    }

    fn attach_opaque<A>(self, attachment: A) -> Result<T, ErrorTrace<C>>
    where
        A: Send + Sync + 'static,
    {
        self.map_err(|trace| trace.attach_opaque(attachment))
    }

    fn attach_opaque_with<A, F>(self, f: F) -> Result<T, ErrorTrace<C>>
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A,
    {
        self.map_err(|trace| trace.attach_opaque(f()))
    }

    fn change_context<D>(self, context: D) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
    {
        self.map_err(|trace| trace.change_context(context))
    }

    fn change_context_with<D, F>(self, f: F) -> Result<T, ErrorTrace<D>>
    where
        D: Error + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.map_err(|trace| trace.change_context(f()))
    }
}

/// Trait for converting a value into an [`ErrorTrace`].
pub trait IntoErrorTrace<C: Error + Send + Sync + 'static> {
    /// Convert `self` into an `ErrorTrace<C>`.
    fn into_error_trace(self) -> ErrorTrace<C>;
}

impl<E> IntoErrorTrace<E> for E
where
    E: Error + Send + Sync + 'static,
{
    fn into_error_trace(self) -> ErrorTrace<E> {
        ErrorTrace::new(self)
    }
}

impl<C> Error for ErrorTrace<C>
where
    C: Error + Send + Sync + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}
