/// WHY: Middleware needs type-safe storage to pass data between layers.
/// `TimingMiddleware` stores Instant, `RetryMiddleware` stores `RetryState`, etc.
///
/// WHAT: Type-safe extension storage using `TypeId` as key. Allows middleware
/// to store arbitrary data in requests without coupling middleware implementations.
///
/// HOW: Uses `HashMap`<`TypeId`, Box<dyn Any + Send + Sync>> for type-erased storage.
/// Type safety restored via `downcast_ref/downcast_mut` at retrieval.
///
/// # Examples
///
/// ```
/// use foundation_core::wire::simple_http::client::Extensions;
/// use std::time::Instant;
///
/// let mut extensions = Extensions::new();
/// extensions.insert(Instant::now());
/// extensions.insert(42u32);
///
/// assert!(extensions.get::<Instant>().is_some());
/// assert_eq!(extensions.get::<u32>(), Some(&42u32));
/// ```
///
/// # Panics
///
/// Never panics.
use std::any::{Any, TypeId};
use std::collections::HashMap;

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
    ///
    /// WHY: Initialize storage for middleware data.
    ///
    /// WHAT: Returns empty Extensions with no stored values.
    ///
    /// HOW: Creates default `HashMap`.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Inserts a value of type T into extensions.
    ///
    /// WHY: Store typed data for later retrieval by middleware.
    ///
    /// WHAT: Stores value, replacing any existing value of same type.
    ///
    /// HOW: Uses `TypeId::of::`<T>() as key, boxes value as trait object.
    ///
    /// # Panics
    ///
    /// Never panics.
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Gets immutable reference to value of type T.
    ///
    /// WHY: Retrieve stored data without consuming it.
    ///
    /// WHAT: Returns Some(&T) if value exists, None otherwise.
    ///
    /// HOW: Looks up by `TypeId`, downcasts trait object to concrete type.
    ///
    /// # Panics
    ///
    /// Never panics.
    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Gets mutable reference to value of type T.
    ///
    /// WHY: Retrieve and modify stored data in place.
    ///
    /// WHAT: Returns Some(&mut T) if value exists, None otherwise.
    ///
    /// HOW: Looks up by `TypeId`, downcasts trait object to concrete type.
    ///
    /// # Panics
    ///
    /// Never panics.
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
