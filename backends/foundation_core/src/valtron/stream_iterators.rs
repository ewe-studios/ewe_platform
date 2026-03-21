//! Extension trait and combinators for `StreamIterator`.
//!
//! This module provides builder-style combinator methods for any type implementing
//! `StreamIterator`. These combinators are applied **AFTER** calling `execute()`.
//!
//! ## Overview
//!
//! The `StreamIteratorExt` trait extends any `StreamIterator` with methods like:
//! - `map_all_done()` - Transform Next (Done) values
//! - `map_all_pending()` - Transform Pending values
//! - `filter_all_done()` - Filter Next values
//! - `collect_all()` - Collect all Next values into a Vec (terminal operation)
//!
//! ## Usage Pattern
//!
//! ```ignore
//! // 1. Execute task to get StreamIterator
//! let stream = execute(my_task)?;
//!
//! // 2. Apply StreamIteratorExt combinators AFTER execute()
//! let stream = stream
//!     .map_all_done(|v| transform(v))
//!     .filter_all_done(|v| should_keep(v));
//!
//! // 3. Consume the stream
//! for item in stream {
//!     match item {
//!         Stream::Next(value) => { /* got result */ }
//!         Stream::Pending(p) => { /* still waiting */ }
//!         Stream::Delayed(d) => { /* will be delayed */ }
//!         Stream::Init => { /* initializing */ }
//!         Stream::Ignore => { /* ignore */ }
//!     }
//! }
//! ```
//!
//! ## Relationship to Other Modules
//!
//! - [`super::synca::mpp`] - Contains `Stream` enum and `StreamIterator` trait
//! - [`super::executors::drivers`] - Contains `DrivenStreamIterator`
//! - [`super::task_iterators`] - Contains `TaskIteratorExt` for pre-execute combinators

use crate::synca::mpp::{Stream, StreamIterator};

/// Extension trait providing combinator methods for any `StreamIterator`.
///
/// This trait is automatically implemented for any type that implements
/// `StreamIterator` with the appropriate bounds. This includes:
/// - `DrivenStreamIterator` from `drivers.rs`
/// - Any custom iterator implementing `StreamIterator`
///
/// ## Combinators
///
/// - [`map_all_done`](StreamIteratorExt::map_all_done) - Transform Next (Done) values
/// - [`map_all_pending`](StreamIteratorExt::map_all_pending) - Transform Pending values
/// - [`map_all_pending_and_done`](StreamIteratorExt::map_all_pending_and_done) - Transform both
/// - [`filter_all_done`](StreamIteratorExt::filter_all_done) - Filter Next values
/// - [`map_all_delayed`](StreamIteratorExt::map_all_delayed) - Transform Delayed durations
/// - [`collect_all`](StreamIteratorExt::collect_all) - Collect all Next values (terminal)
///
/// ## Example
///
/// ```ignore
/// let stream = execute(my_task)?
///     .map_done(|v| v * 2)
///     .map_pending(|p| format!("Pending: {:?}", p))
///     .filter_done(|v| v > 10);
/// ```
pub trait StreamIteratorExt<D, P>: StreamIterator<D, P> + Sized {
    /// Transform Next (Done) values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Pending states pass through unchanged.
    fn map_done<F, R>(self, f: F) -> MapDone<Self, F, D, P>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform Pending values using the provided function.
    ///
    /// Init, Ignore, Delayed, and Next states pass through unchanged.
    fn map_pending<F, R>(self, f: F) -> MapPending<Self, F, P, D>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static;

    /// Transform both Pending and Next values with a single function.
    ///
    /// Returns a unified output type for both states.
    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, F, D, P>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static;

    /// Filter Next (Done) values using the provided predicate.
    ///
    /// Non-Next states pass through unchanged. Next values that don't
    /// satisfy the predicate are skipped.
    fn filter_done<F>(self, f: F) -> FilterDone<Self, F, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static;

    /// Transform Delayed durations.
    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, F, D, P>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static;

    /// Collect all Next values into a Vec.
    ///
    /// This is a non-blocking collect operation. It passes through
    /// Pending, Delayed, and Init states unchanged, and only yields
    /// the collected Vec<Done> when the stream completes.
    fn collect(self) -> Collect<Self, D, P>
    where
        D: Clone;
}

// Blanket implementation: anything implementing StreamIterator gets StreamIteratorExt
impl<S, D, P> StreamIteratorExt<D, P> for S
where
    S: StreamIterator<D, P> + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
    fn map_done<F, R>(self, f: F) -> MapDone<Self, F, D, P>
    where
        F: Fn(D) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapDone {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending<F, R>(self, f: F) -> MapPending<Self, F, P, D>
    where
        F: Fn(P) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPending {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_pending_and_done<F, R>(self, f: F) -> MapPendingAndDone<Self, F, D, P>
    where
        F: Fn(Stream<D, P>) -> R + Send + 'static,
        R: Send + 'static,
    {
        MapPendingAndDone {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn filter_done<F>(self, f: F) -> FilterDone<Self, F, D, P>
    where
        F: Fn(&D) -> bool + Send + 'static,
    {
        FilterDone {
            inner: self,
            predicate: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn map_delayed<F>(self, f: F) -> MapDelayed<Self, F, D, P>
    where
        F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    {
        MapDelayed {
            inner: self,
            mapper: f,
            _phantom: std::marker::PhantomData,
        }
    }

    fn collect(self) -> Collect<Self, D, P>
    where
        D: Clone,
    {
        Collect {
            inner: self,
            collected: Vec::new(),
            done: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

/// Wrapper type that transforms Next (Done) values.
pub struct MapDone<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, R, P> Iterator for MapDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(D) -> R + Send + 'static,
    R: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<R, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next((self.mapper)(v)),
            Stream::Pending(v) => Stream::Pending(v),
            Stream::Delayed(d) => Stream::Delayed(d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, F, D, R, P> StreamIterator<R, P> for MapDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(D) -> R + Send + 'static,
    R: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Pending values.
pub struct MapPending<I, F, P, D> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(P, D)>,
}

impl<I, F, D, P, R> Iterator for MapPending<I, F, P, D>
where
    I: StreamIterator<D, P>,
    F: Fn(P) -> R + Send + 'static,
    R: Send + 'static,
    D: Send + 'static,
{
    type Item = Stream<D, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next(v),
            Stream::Pending(v) => Stream::Pending((self.mapper)(v)),
            Stream::Delayed(d) => Stream::Delayed(d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, F, D, P, R> StreamIterator<D, R> for MapPending<I, F, P, D>
where
    I: StreamIterator<D, P>,
    F: Fn(P) -> R + Send + 'static,
    R: Send + 'static,
    D: Send + 'static,
{
}

/// Wrapper type that transforms both Pending and Next values.
pub struct MapPendingAndDone<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P, R> Iterator for MapPendingAndDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(Stream<D, P>) -> R + Send + 'static,
    R: Send + 'static,
{
    type Item = Stream<R, R>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match &stream {
            Stream::Next(_) | Stream::Pending(_) => Stream::Next((self.mapper)(stream)),
            Stream::Delayed(d) => Stream::Delayed(*d),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, F, D, P, R> StreamIterator<R, R> for MapPendingAndDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(Stream<D, P>) -> R + Send + 'static,
    R: Send + 'static,
{
}

/// Wrapper type that filters Next (Done) values.
///
/// Filtered-out Next values are returned as `Stream::Ignore` to avoid blocking.
pub struct FilterDone<I, F, D, P> {
    inner: I,
    predicate: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P> Iterator for FilterDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(&D) -> bool + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        let stream = self.inner.next()?;
        match &stream {
            Stream::Next(v) => {
                if (self.predicate)(v) {
                    Some(stream)
                } else {
                    Some(Stream::Ignore)
                }
            }
            _ => Some(stream), // Pass through non-Next states
        }
    }
}

impl<I, F, D, P> StreamIterator<D, P> for FilterDone<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(&D) -> bool + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that transforms Delayed durations.
pub struct MapDelayed<I, F, D, P> {
    inner: I,
    mapper: F,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, F, D, P> Iterator for MapDelayed<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|stream| match stream {
            Stream::Next(v) => Stream::Next(v),
            Stream::Pending(v) => Stream::Pending(v),
            Stream::Delayed(d) => Stream::Delayed((self.mapper)(d)),
            Stream::Init => Stream::Init,
            Stream::Ignore => Stream::Ignore,
        })
    }
}

impl<I, F, D, P> StreamIterator<D, P> for MapDelayed<I, F, D, P>
where
    I: StreamIterator<D, P>,
    F: Fn(std::time::Duration) -> std::time::Duration + Send + 'static,
    D: Send + 'static,
    P: Send + 'static,
{
}

/// Wrapper type that collects all Next (Done) values into a Vec.
///
/// This is a non-blocking collect operation. It passes through
/// Pending, Delayed, and Init states unchanged, and only yields
/// the collected Vec<Done> when the stream completes.
pub struct Collect<I, D, P> {
    inner: I,
    collected: Vec<D>,
    done: bool,
    _phantom: std::marker::PhantomData<(D, P)>,
}

impl<I, D, P> Iterator for Collect<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
    type Item = Stream<Vec<D>, P>;

    fn next(&mut self) -> Option<Self::Item> {
        // If we've already yielded the collected result, we're done
        if self.done {
            return None;
        }

        match self.inner.next() {
            Some(Stream::Next(value)) => {
                self.collected.push(value);
                // Keep collecting, return Ignore to signal still collecting
                Some(Stream::Ignore)
            }
            Some(Stream::Pending(p)) => {
                // Pass through Pending
                Some(Stream::Pending(p))
            }
            Some(Stream::Delayed(d)) => Some(Stream::Delayed(d)),
            Some(Stream::Init) => Some(Stream::Init),
            Some(Stream::Ignore) => Some(Stream::Ignore),
            None => {
                // Inner iterator is done, yield the collected result
                self.done = true;
                Some(Stream::Next(self.collected.clone()))
            }
        }
    }
}

impl<I, D, P> StreamIterator<Vec<D>, P> for Collect<I, D, P>
where
    I: StreamIterator<D, P>,
    D: Clone + Send + 'static,
    P: Send + 'static,
{
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple test stream iterator for unit tests
    struct TestStream {
        items: Vec<Stream<u32, String>>,
    }

    impl TestStream {
        fn new(items: Vec<Stream<u32, String>>) -> Self {
            Self { items }
        }
    }

    impl Iterator for TestStream {
        type Item = Stream<u32, String>;

        fn next(&mut self) -> Option<Self::Item> {
            self.items.pop()
        }
    }

    impl StreamIterator<u32, String> for TestStream {}

    #[test]
    fn test_map_done() {
        let items = vec![
            Stream::Pending("wait".to_string()),
            Stream::Next(10),
            Stream::Next(5),
        ];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_done(|x| x * 2);

        // pop() returns items in reverse order: 5, 10, Pending
        // After mapping: 10 (5*2), 20 (10*2), Pending
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(10)));
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(20)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(Stream::Pending("wait".to_string()))
        );
    }

    #[test]
    fn test_map_pending() {
        let items = vec![Stream::Pending("wait".to_string()), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_pending(|s| s.len());

        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Pending(4)));
    }

    #[test]
    fn test_filter_done() {
        let items = vec![Stream::Next(3), Stream::Next(10), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut filtered = stream.filter_done(|x| *x > 5);

        // pop() returns: Next(5), Next(10), Next(3)
        // filter: 5 > 5 = false → Ignore, 10 > 5 = true → Next(10), 3 > 5 = false → Ignore
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 5 was filtered out
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Next(10)));
        assert_eq!(Iterator::next(&mut filtered), Some(Stream::Ignore)); // 3 was filtered out
        assert_eq!(Iterator::next(&mut filtered), None);
    }

    #[test]
    fn test_collect() {
        let items = vec![
            Stream::Next(3),
            Stream::Next(2),
            Stream::Pending("wait".to_string()),
            Stream::Next(1),
        ];
        let stream = TestStream::new(items);
        let mut collected = StreamIteratorExt::collect(stream);

        // pop() returns: Next(1), Pending, Next(2), Next(3)
        // First Next(1) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Pending passes through unchanged
        assert_eq!(
            Iterator::next(&mut collected),
            Some(Stream::Pending("wait".to_string()))
        );

        // Next(2) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Next(3) is collected, returns Ignore
        assert_eq!(Iterator::next(&mut collected), Some(Stream::Ignore));

        // Should yield collected Vec at the end
        match Iterator::next(&mut collected) {
            Some(Stream::Next(vec)) => {
                assert_eq!(vec.len(), 3);
                assert!(vec.contains(&1));
                assert!(vec.contains(&2));
                assert!(vec.contains(&3));
            }
            other => panic!("Expected Next(Vec), got {:?}", other),
        }

        // Should be done after yielding the collected result
        assert_eq!(Iterator::next(&mut collected), None);
    }

    #[test]
    fn test_map_delayed() {
        use std::time::Duration;

        let items = vec![Stream::Delayed(Duration::from_secs(1)), Stream::Next(5)];
        let stream = TestStream::new(items);
        let mut mapped = stream.map_delayed(|d| d * 2);

        assert_eq!(Iterator::next(&mut mapped), Some(Stream::Next(5)));
        assert_eq!(
            Iterator::next(&mut mapped),
            Some(Stream::Delayed(Duration::from_secs(2)))
        );
    }
}
