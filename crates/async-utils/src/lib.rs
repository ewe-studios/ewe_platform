// Copied as is from https://github.com/yoshuawuyts/async-convert

//! Async `TryFrom`/`TryInto` traits.
//!
//! # Why
//!
//! In async-std we created async versions of `FromStream`, `IntoStream`, and
//! `Iterator::collect`. These traits represent conversions from one type to
//! another. But the canonical way of performing this conversion is through the
//! `TryFrom` and `TryInto` traits.
//!
//! For example when deserializing some `MyBody` from a `Request`, you will want
//! to declare a `TryFrom<Request> for MyBody` which consumes the bytes in the
//! request and tries to create the body. This operation is fallible, and when
//! writing async code also needs to be async.
//!
//! This crate provides traits for that, through the
//! [`async_trait`](https://doc.rust-lang.org/std/convert/trait.TryFrom.html)
//! crate. This is an experiment, but we'll likely want to extend `async-std`
//! with this at some point too.
//!
//! # Examples
//!
//! ```
//! use ewe_async_utils::{async_trait, TryFrom};
//!
//! struct GreaterThanZero(i32);
//!
//! #[async_trait]
//! impl TryFrom<i32> for GreaterThanZero {
//!     type Error = &'static str;
//!
//!     async fn try_from(value: i32) -> Result<Self, Self::Error> {
//!         // pretend we're actually doing async IO here instead.
//!         if value <= 0 {
//!             Err("GreaterThanZero only accepts value superior than zero!")
//!         } else {
//!             Ok(GreaterThanZero(value))
//!         }
//!     }
//! }
//! ```

#![forbid(unsafe_code, future_incompatible, rust_2018_idioms)]
#![deny(missing_debug_implementations, nonstandard_style)]

pub use async_trait::async_trait;

/// A shared prelude.
pub mod prelude {
    pub use super::TryFrom as _;
    pub use super::TryInto as _;
}

/// Simple and safe type conversions that may fail in a controlled
/// way under some circumstances. It is the reciprocal of [`TryInto`].
///
/// This is useful when you are doing a type conversion that may
/// trivially succeed but may also need special handling.
/// For example, there is no way to convert an [`i64`] into an [`i32`]
/// using the [`From`] trait, because an [`i64`] may contain a value
/// that an [`i32`] cannot represent and so the conversion would lose data.
/// This might be handled by truncating the [`i64`] to an [`i32`] (essentially
/// giving the [`i64`]'s value modulo [`i32::MAX`]) or by simply returning
/// [`i32::MAX`], or by some other method.  The [`From`] trait is intended
/// for perfect conversions, so the `TryFrom` trait informs the
/// programmer when a type conversion could go bad and lets them
/// decide how to handle it.
#[async_trait]
pub trait TryFrom<T>: Sized {
    /// The type returned in the event of a conversion error.
    type Error;

    /// Performs the conversion.
    async fn try_from(value: T) -> Result<Self, Self::Error>;
}

/// An attempted conversion that consumes `self`, which may or may not be
/// expensive.
///
/// Library authors should usually not directly implement this trait,
/// but should prefer implementing the [`TryFrom`] trait, which offers
/// greater flexibility and provides an equivalent `TryInto`
/// implementation for free, thanks to a blanket implementation in the
/// standard library. For more information on this, see the
/// documentation for [`Into`].
///
/// # Implementing `TryInto`
///
/// This suffers the same restrictions and reasoning as implementing
/// [`Into`], see there for details.
#[async_trait(?Send)]
pub trait TryInto<T>: Sized {
    /// The type returned in the event of a conversion error.
    type Error;

    /// Performs the conversion.
    async fn try_into(self) -> Result<T, Self::Error>;
}

// TryFrom implies TryInto
#[async_trait(?Send)]
impl<T, U> TryInto<U> for T
where
    U: TryFrom<T>,
{
    type Error = U::Error;

    async fn try_into(self) -> Result<U, U::Error> {
        U::try_from(self).await
    }
}
