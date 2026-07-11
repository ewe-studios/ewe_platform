//! Shared types between simple_http modules that must compile on all targets.
//!
//! WHY: `impls.rs` (always compiled) needs `ContentLengthEnforcingIterator` and `Extensions`,
//! which previously lived inside `client/` (native-only). Moving them here breaks the cycle.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use foundation_core::extensions::result_ext::BoxedError;
use foundation_core::io::readers::Data;
use crate::simple_http::shared::errors::Result;

// ============================================================================
// Extensions — type-safe extension storage for middleware
// ============================================================================

/// Type-safe extension storage for middleware data.
///
/// WHY: Middleware attaches arbitrary typed data to requests without touching
/// the core request structure. Values are `Arc`-backed so the whole map is
/// **cheaply clonable** (refcount bumps only) — required by the connectrpc
/// owned-`Clone` `Ctx` / copy-on-write write model, where a layer that adds an
/// extension rebuilds the map by pointer bumps and hands a new context forward
/// (Decision 04 / Decision 12 §13). The `Arc` value is transparent to callers:
/// `insert`/`get`/`get_mut` behave exactly as before.
///
/// WHAT: `HashMap` keyed by `TypeId`, storing `Arc<dyn Any + Send + Sync>`.
///
/// HOW: `insert()` wraps the value in `Arc::new`; `get()` downcasts a shared
/// reference; `get_mut()` mutates in place only when the value is **uniquely
/// owned** (`Arc::get_mut`) — a shared value returns `None`, matching the
/// copy-on-write contract (mutate a private copy, never someone else's clone).
#[derive(Default, Clone)]
pub struct Extensions {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
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
        self.map.insert(TypeId::of::<T>(), Arc::new(value));
    }

    /// Gets immutable reference to value of type T.
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|shared| shared.downcast_ref::<T>())
    }

    /// Gets mutable reference to value of type T.
    ///
    /// Returns `None` when the stored value is shared with another clone of the
    /// map — mutation is only permitted on a uniquely-owned value (copy-on-write
    /// contract). Immediately after `insert` (or on a never-cloned map) the value
    /// is unique, so this is a no-op difference for the common middleware case.
    #[must_use]
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map
            .get_mut(&TypeId::of::<T>())
            .and_then(Arc::get_mut)
            .and_then(|shared| shared.downcast_mut::<T>())
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
