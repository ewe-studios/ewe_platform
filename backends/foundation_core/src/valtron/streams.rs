//! Stream types and traits for valtron stream processing.
//!
//! This module contains the core stream abstractions used throughout valtron:
//! - [`Stream<D, P>`] - Enum representing stream states (Init, Ignore, Delayed, Pending, Next)
//! - [`StreamIterator`] - Trait for iterators yielding Stream items
//! - [`StreamIter`] - Newtype entry point for chaining
//!   [`StreamIteratorExt`](crate::valtron::StreamIteratorExt) combinators
//! - [`StreamSpread<D, P>`] - Element of a [`Stream::Spread`] batch
//! - [`ShortCircuit<D, P>`] - Control enum for the `map_circuit` combinator
//!
//! # Execution iterator
//!
//! Executors drive tasks through
//! [`NotifyQueueStreamIterator`](crate::valtron::executors::NotifyQueueStreamIterator),
//! a condvar-backed iterator over a
//! [`NotifyQueue`](crate::valtron::executors::NotifyQueue). Its `max_turns` /
//! `park_duration` parameters balance responsiveness (checking other tasks)
//! against throughput.
//!
//! ## Constants
//!
//! - [`DEFAULT_MAX_TURNS`](crate::valtron::executors::DEFAULT_MAX_TURNS) - Default poll attempts before yielding (15)
//! - [`DEFAULT_PARK_DURATION`](crate::valtron::executors::DEFAULT_PARK_DURATION) - Default park duration (1ms)

use crate::valtron::wake::WakeableStream;

#[derive(Clone)]
pub enum StreamSpread<D, P> {
    Pending(P),
    Done(D),
}

impl<D: PartialEq, P: PartialEq> PartialEq for StreamSpread<D, P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Done(me), Self::Done(them)) => me == them,
            (Self::Pending(me), Self::Pending(them)) => me == them,
            _ => false,
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Display for StreamSpread<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(p) => write!(f, "Pending({p:?})"),
            Self::Done(d) => write!(f, "Next({d:?})"),
        }
    }
}

impl<D: core::fmt::Debug, P: core::fmt::Debug> core::fmt::Debug for StreamSpread<D, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending(p) => f.debug_tuple("Pending").field(p).finish(),
            Self::Done(d) => f.debug_tuple("Next").field(d).finish(),
        }
    }
}

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
    Spread(Vec<StreamSpread<D, P>>),
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
            Stream::Spread(items) => write!(f, "Spread({items:?})"),
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
            Stream::Spread(items) => f.debug_tuple("Spread").field(items).finish(),
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

/// Newtype wrapper that provides a clean entry point for chaining valtron
/// [`StreamIteratorExt`] combinators.  Construct via
/// [`StreamIteratorExt::into_stream_iter`].
///
/// `StreamIterator` requires `Iterator` as a supertrait, so this wrapper
/// provides an `Iterator` impl that delegates to the inner value.  The
/// value-add is ergonomic — `.into_stream_iter().map_done(…)` is
/// unambiguously the valtron combinator.
#[derive(Debug)]
pub struct StreamIter<S: StreamIterator>(pub S);

impl<S: StreamIterator> Iterator for StreamIter<S> {
    type Item = Stream<S::D, S::P>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

// `StreamIter` is the plain-iterator entry point: it must NOT forward `wake`, so a
// waking chain that routes through `into_stream_iter()` degrades to self-wake
// (correct — never a lost wake).
#[cfg(feature = "multi")]
impl<S> WakeableStream for StreamIter<S>
where
    S: StreamIterator,
    S::D: Send + 'static,
    S::P: Send + 'static,
{
}

#[cfg(not(feature = "multi"))]
impl<S> WakeableStream for StreamIter<S>
where
    S: StreamIterator,
    S::D: 'static,
    S::P: 'static,
{
}

// Blanket implementation for any compatible iterator (Send variant — multi-threaded)
#[cfg(feature = "multi")]
impl<M, D, P> StreamIterator for M
where
    M: Iterator<Item = Stream<D, P>> + ?Sized,
    D: Send + 'static,
    P: Send + 'static,
{
    type D = D;
    type P = P;
}

// Blanket implementation for any compatible iterator (non-Send variant — wasm32 / single-threaded)
#[cfg(not(feature = "multi"))]
impl<M, D, P> StreamIterator for M
where
    M: Iterator<Item = Stream<D, P>> + ?Sized,
    D: 'static,
    P: 'static,
{
    type D = D;
    type P = P;
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
