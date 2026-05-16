//! Shared types between simple_http modules that must compile on all targets.
//!
//! WHY: `impls.rs` (always compiled) needs `ContentLengthEnforcingIterator` and `Extensions`,
//! which previously lived inside `client/` (native-only). Moving them here breaks the cycle.

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::extensions::result_ext::BoxedError;
use crate::io::readers::Data;
use crate::wire::simple_http::errors::Result;

// ============================================================================
// Extensions — type-safe extension storage for middleware
// ============================================================================

/// Type-safe extension storage for middleware data.
///
/// WHY: Allows middleware to attach arbitrary typed data to requests
/// without modifying the core request structure.
///
/// WHAT: `HashMap` keyed by `TypeId`, storing boxed trait objects.
///
/// HOW: `insert()` boxes the value, `get()/get_mut()` downcasts back to concrete type.
#[derive(Default)]
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    /// Creates a new empty Extensions container.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Inserts a value of type T into extensions.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Gets immutable reference to value of type T.
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Gets mutable reference to value of type T.
    #[must_use]
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Extensions")
            .field("count", &self.map.len())
            .finish()
    }
}

// ============================================================================
// ContentLengthEnforcingIterator — body size validation
// ============================================================================

/// Iterator wrapper that tracks total bytes read and validates against
/// expected `Content-Length` when the inner iterator reaches EOF.
///
/// WHY: When `Content-Length` is declared, the body must match the promised size.
/// If the stream ends early, callers should get an error rather than silently
/// receiving a truncated body.
pub struct ContentLengthEnforcingIterator<I> {
    inner: Option<I>,
    expected: usize,
    bytes_read: usize,
}

impl<I> ContentLengthEnforcingIterator<I> {
    pub fn new(inner: I, expected: usize) -> Self {
        Self {
            inner: Some(inner),
            expected,
            bytes_read: 0,
        }
    }
}

impl<I> Iterator for ContentLengthEnforcingIterator<I>
where
    I: Iterator<Item = Result<Data, BoxedError>>,
{
    type Item = Result<Data, BoxedError>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut inner = self.inner.take()?;

        match inner.next() {
            Some(Ok(Data::Bytes(bytes))) => {
                self.bytes_read += bytes.len();
                self.inner = Some(inner);
                Some(Ok(Data::Bytes(bytes)))
            }
            Some(other) => {
                self.inner = Some(inner);
                Some(other)
            }
            None => {
                if self.bytes_read != self.expected {
                    Some(Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!(
                            "body truncated: expected {} bytes per Content-Length, got {}",
                            self.expected, self.bytes_read
                        ),
                    ))))
                } else {
                    None
                }
            }
        }
    }
}
