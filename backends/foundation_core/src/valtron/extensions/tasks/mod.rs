#[cfg(not(feature = "multi"))]
mod non_sendable;

#[cfg(not(feature = "multi"))]
pub use non_sendable::*;

#[cfg(feature = "multi")]
mod sendable;

#[cfg(feature = "multi")]
pub use sendable::*;
