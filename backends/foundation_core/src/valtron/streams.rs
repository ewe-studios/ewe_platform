//! Stream types and traits for valtron stream processing.
//!
//! This module contains the core stream abstractions used throughout valtron:
//! - [`Stream<D, P>`] - Enum representing stream states (Init, Ignore, Delayed, Pending, Next)
//! - [`StreamIterator`] - Trait for iterators yielding Stream items
//! - [`ConcurrentQueueStreamIterator<D, P>`] - Default iterator with configurable polling (max_turns)
//! - [`StreamRecvIterator<D, P>`] - Legacy iterator for channel-based streams (deprecated)
//! - [`StreamIteratorExt`] - Extension trait with stream combinators
//!
//! # Default Iterator
//!
//! The [`ConcurrentQueueStreamIterator`] is the default iterator used by valtron executors.
//! It provides configurable polling behavior via `max_turns` parameter, balancing
//! responsiveness (checking other tasks) against throughput (busy polling).
//!
//! ## Constants
//!
//! - [`DEFAULT_MAX_TURNS`](crate::valtron::executors::DEFAULT_MAX_TURNS) - Default poll attempts before yielding (10)
//! - [`DEFAULT_PARK_DURATION`](crate::valtron::executors::DEFAULT_PARK_DURATION) - Default park duration (20ns)

use std::sync::Arc;

use concurrent_queue::ConcurrentQueue;

use crate::synca::mpp::RecvIterator;

/// [`Stream<D, P>`] represents the state of a stream in the valtron execution model.
///
/// Valtron uses a progress-driven execution model where the executor polls iterators
/// frequently to observe intermediate states and make scheduling decisions. The variants
/// of this enum communicate different stream states to the executor:
///
/// - [`Stream::Init`] - Stream is initializing
/// - [`Stream::Ignore`] - Internal system operations occurring, executor should ignore
/// - [`Stream::Delayed`] - Next response will be delayed by the contained duration
/// - [`Stream::Pending`] - Stream is pending with the contained context value
/// - [`Stream::Next`] - Stream has produced the next value
///
/// # Type Parameters
///
/// * `D` - The data type yielded by the stream when complete
/// * `P` - The progress/context type yielded while pending
#[derive(PartialEq, Clone)]
pub enum Stream<D, P> {
    /// Indicative the stream is instantiating.
    Init,

    /// Indicative of internal system operations occuring that should be ignored.
    Ignore,

    /// Indicative that the stream next response will be delayed by this much duration
    Delayed(std::time::Duration),

    /// Indicating that the stream is a pending state with giving context value.
    Pending(P),

    /// Indicative the stream just issued its next value.
    Next(D),

    /// No data available, queue still open.
    /// Propagates to executor as a yield signal — on JS, maps to a short setTimeout (~4ms).
    Wait,

    /// Emit multiple next values at once.
    /// Each element is delivered individually as `Stream::Next(value)` at the delivery point.
    SpreadDone(Vec<D>),

    /// Emit multiple pending values at once.
    /// Each element is delivered individually as `Stream::Pending(value)` at the delivery point.
    SpreadPending(Vec<P>),
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Display for Stream<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stream::Init => write!(f, "Init"),
            Stream::Ignore => write!(f, "Ignore"),
            Stream::Delayed(d) => write!(f, "Delayed({d:?})"),
            Stream::Pending(p) => write!(f, "Pending({p:?})"),
            Stream::Next(d) => write!(f, "Next({d:?})"),
            Stream::Wait => write!(f, "Wait"),
            Stream::SpreadDone(items) => write!(f, "SpreadDone({items:?})"),
            Stream::SpreadPending(items) => write!(f, "SpreadPending({items:?})"),
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Debug for Stream<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stream::Init => f.write_str("Init"),
            Stream::Ignore => f.write_str("Ignore"),
            Stream::Delayed(d) => f.debug_tuple("Delayed").field(d).finish(),
            Stream::Pending(p) => f.debug_tuple("Pending").field(p).finish(),
            Stream::Next(d) => f.debug_tuple("Next").field(d).finish(),
            Stream::Wait => f.write_str("Wait"),
            Stream::SpreadDone(items) => f.debug_tuple("SpreadDone").field(items).finish(),
            Stream::SpreadPending(items) => f.debug_tuple("SpreadPending").field(items).finish(),
        }
    }
}

/// [`StreamIterator`] defines an iterator that yields [`Stream<D, P>`] items.
///
/// This trait is automatically implemented for any type that implements
/// `Iterator<Item = Stream<D, P>>` where `D` and `P` are `Send + 'static`.
///
/// # Examples
///
/// ```
/// use foundation_core::valtron::Stream;
///
/// // Any iterator yielding Stream<D, P> automatically implements StreamIterator
/// let items = vec![
///     Stream::Init,
///     Stream::Pending("loading..."),
///     Stream::Next(42),
/// ];
/// let mut iter = items.into_iter();
/// assert!(matches!(iter.next(), Some(Stream::Init)));
/// ```
pub trait StreamIterator: Iterator<Item = Stream<Self::D, Self::P>> {
    /// The data type yielded when the stream completes
    type D;
    /// The progress/context type yielded while pending
    type P;
}

// Blanket implementation for any compatible iterator
impl<M, D, P> StreamIterator for M
where
    M: Iterator<Item = Stream<D, P>> + ?Sized,
    D: Send + 'static,
    P: Send + 'static,
{
    type D = D;
    type P = P;
}

/// [`StreamRecvIterator<D, P>`] wraps a [`RecvIterator<Stream<D, P>>`] to provide
/// stream iteration over channel-based streams.
///
/// # Deprecation
///
/// **Deprecated:** Use [`ConcurrentQueueStreamIterator`] instead.
///
/// `StreamRecvIterator` uses blocking reads which can starve other tasks in multi-task
/// scenarios. `ConcurrentQueueStreamIterator` provides configurable polling behavior
/// with `max_turns` for better task scheduling.
///
/// ## Migration
///
/// Replace:
/// ```ignore
/// StreamRecvIterator::new(recv_iter)
/// ```
///
/// With:
/// ```ignore
/// ConcurrentQueueStreamIterator::new(chan, DEFAULT_MAX_TURNS, DEFAULT_PARK_DURATION)
/// ```
///
/// This type is used when receiving [`Stream`] items from a concurrent channel,
/// adapting the blocking channel receiver to the valtron stream model.
#[deprecated(since = "1.3.0", note = "Use ConcurrentQueueStreamIterator instead")]
pub struct StreamRecvIterator<D, P>(RecvIterator<Stream<D, P>>);

#[allow(deprecated)]
impl<D, P> StreamRecvIterator<D, P> {
    /// Returns true if the underlying channel is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of items in the underlying channel
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the underlying channel is closed
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}

#[allow(deprecated)]
impl<D, P> StreamRecvIterator<D, P> {
    /// Creates a new StreamRecvIterator wrapping the given RecvIterator
    #[must_use]
    pub fn new(iter: RecvIterator<Stream<D, P>>) -> Self {
        Self(iter)
    }
}

#[allow(deprecated)]
impl<D, P> Iterator for StreamRecvIterator<D, P> {
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

// ============================================================================
// ConcurrentQueueStreamIterator
// ============================================================================

use concurrent_queue::PopError;
use std::time::Duration;

/// An iterator that polls a concurrent queue with configurable max_turns behavior.
///
/// `ConcurrentQueueStreamIterator` is designed for valtron executors where multiple
/// tasks share CPU time. It polls the queue up to `max_turns` times before yielding
/// `Stream::Ignore`, allowing the executor to check other tasks.
///
/// ## Polling Strategy
///
/// - **Turns 1 to max_turns**: Poll queue, yield thread briefly between attempts
/// - **After max_turns**: Return `Stream::Ignore` (signals executor to check other tasks)
/// - **Queue closed**: Return `None` (iterator exhausted)
///
/// ## std vs no_std
///
/// - **std feature**: Uses `std::thread::park_timeout(self.park_duration)`
/// - **no_std**: Uses `std::hint::spin_loop()` (busy-wait spin)
///
/// ## Type Parameters
///
/// * `D` - The "Done" type (successful result)
/// * `P` - The "Pending" type (intermediate state)
///
/// ## Example
///
/// ```ignore
/// use crate::valtron::streams::{Stream, ConcurrentQueueStreamIterator};
/// use concurrent_queue::ConcurrentQueue;
/// use std::sync::Arc;
/// use std::time::Duration;
///
/// let queue: Arc<ConcurrentQueue<Stream<i32, &str>>> = Arc::new(ConcurrentQueue::bounded(10));
/// let iter = ConcurrentQueueStreamIterator::new(queue.clone(), 10, Duration::from_nanos(20));
///
/// for item in iter {
///     match item {
///         Stream::Next(value) => println!("Got: {}", value),
///         Stream::Ignore => println!("Yielding to check other tasks..."),
///         Stream::Pending(ctx) => println!("Pending: {}", ctx),
///         _ => {}
///     }
/// }
/// ```
pub struct ConcurrentQueueStreamIterator<D, P> {
    chan: Arc<ConcurrentQueue<Stream<D, P>>>,
    max_turns: usize,
    park_duration: Duration,
}

impl<D, P> ConcurrentQueueStreamIterator<D, P> {
    /// Creates a new ConcurrentQueueStreamIterator.
    ///
    /// ## Arguments
    ///
    /// * `chan` - The concurrent queue to poll
    /// * `max_turns` - Number of poll attempts before yielding Stream::Ignore
    /// * `park_duration` - Duration to park thread when queue is empty (std mode)
    ///
    /// ## Panics
    ///
    /// Panics if `max_turns` is 0 (must be at least 1)
    pub fn new(
        chan: Arc<ConcurrentQueue<Stream<D, P>>>,
        max_turns: usize,
        park_duration: Duration,
    ) -> Self {
        assert!(max_turns > 0, "max_turns must be greater than 0");
        tracing::trace!(
            max_turns = max_turns,
            park_duration_ns = park_duration.as_nanos(),
            "created ConcurrentQueueStreamIterator"
        );
        Self {
            chan,
            max_turns,
            park_duration,
        }
    }

    /// Returns a reference to the underlying channel.
    #[must_use]
    pub fn chan(&self) -> &Arc<ConcurrentQueue<Stream<D, P>>> {
        &self.chan
    }

    /// Returns the configured max_turns value.
    #[must_use]
    pub fn max_turns(&self) -> usize {
        self.max_turns
    }

    /// Returns the configured park_duration value.
    #[must_use]
    pub fn park_duration(&self) -> Duration {
        self.park_duration
    }

    /// Returns true if the underlying queue is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.chan.is_closed()
    }

    /// Returns the current number of items in the queue.
    #[must_use]
    pub fn len(&self) -> usize {
        self.chan.len()
    }

    /// Returns true if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chan.is_empty()
    }
}

impl<D, P> Iterator for ConcurrentQueueStreamIterator<D, P> {
    type Item = Stream<D, P>;

    fn next(&mut self) -> Option<Self::Item> {
        tracing::trace!(max_turns = self.max_turns, "starting poll cycle");

        for turn in 0..self.max_turns {
            match self.chan.pop() {
                Ok(value) => {
                    tracing::debug!(turn = turn, "received value from queue");
                    return Some(value);
                }
                Err(PopError::Empty) => {
                    #[cfg(feature = "std")]
                    std::thread::park_timeout(self.park_duration);
                    #[cfg(not(feature = "std"))]
                    std::hint::spin_loop();
                }
                Err(PopError::Closed) => {
                    tracing::debug!("queue closed, ending iteration");
                    return None;
                }
            }
        }

        tracing::trace!("max_turns reached, yielding Ignore");
        Some(Stream::Ignore)
    }
}

// ============================================================================
// StreamIterator Extension Trait and Combinators
// ============================================================================

/// Control enum for the `map_circuit` combinator.
///
/// Allows short-circuiting iterator chains with three possible outcomes:
/// - `Continue(item)` - Continue iteration with the transformed item
/// - `ReturnAndStop(item)` - Return this item and then stop iteration permanently
/// - `Stop` - Stop iteration without returning anything
///
/// This is useful for error handling patterns where you want to:
/// - Return an error value and immediately stop when an error is encountered
/// - Stop silently without returning anything in certain conditions
/// - Continue normal iteration otherwise
///
/// ## Example
///
/// ```ignore
/// let stream = execute(my_task)?
///     .map_circuit(|item| {
///         match item {
///             Stream::Next(err_result) if err_result.is_error() => {
///                 ShortCircuit::ReturnAndStop(Stream::Next(err_result))
///             }
///             Stream::Pending(_) | Stream::Next(_) => {
///                 ShortCircuit::Continue(item)
///             }
///             _ => ShortCircuit::Stop,
///         }
///     });
/// ```
pub enum ShortCircuit<D, P> {
    /// Continue iteration with the wrapped item
    Continue(Stream<D, P>),
    /// Return this item and then stop iteration permanently
    ReturnAndStop(Stream<D, P>),
    /// Stop iteration without returning anything
    Stop,
}
