/// Delay devises a way for Iterators to communicate a delay of the completion
/// of an operation without requiring the caller to be forced to be blocked
/// till the operation is done. This is similar to the async/await model where
/// operations that need completion can call `func().await` to signal the runtime
/// to pause that operation till the underlying response is ready.
/// We replicate similar but in an Iterator only world.
///
/// One interesting question is what does a delay mean:
///
/// 1. In one sense this can be the delay result of a one time operation
/// upon which completio we get our result from the `Delayed::Done` option at
/// which point we can expect no further results.
/// 2. But in another sense can also represent a re-occuring operation that will be
/// delayed a specific period of time upon which after completionm, may or may not repeat.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Delayed<T> {
    /// Pending represents to the reciever two important information:
    ///
    /// 1. The actual time instant when the delay began (think) of this
    /// as the actual beginning when we start to count time (start of time)
    /// for the delayed output
    /// 2. The total duration upon which we will be delayed from the start of
    /// time.
    /// 3. The remaining duration left ontil the delay is finished
    /// (this is more of a bonus).
    ///
    /// This allows more communication about an operation still awaiting completion.
    Pending(std::time::Instant, std::time::Duration, std::time::Duration),
    Done(T),
}

/// AsDelayedIterator represents a blanket iterator that always returns
/// a Delayed value which indicate when some operation will finish
/// where each call communicates the 3 key information:
///
/// 1. The instant of time such delay began
/// 2. The total duration things will be delayed from the start of time in (1).
/// 3. The total duration left till it is completed.
pub trait AsDelayedIterator<T>: Iterator<Item = Delayed<T>> {}

/// DelayedIterator represents a new type of iterator that represents an operation
/// that is to be completed at some future point in time due to a delay.
///
/// Unlike an async operation where we do not know when it is done, a delayed operation
/// stipulates when it begins that an operation is delayed till some duration of time
/// is done at which you can expect the result in a `Delayed::Done` response.
///
/// DelayedIterators can be thought of as two forms:
///
/// 1. In one sense this can be the delay result of a one time operation
/// upon which completio we get our result from the `Delayed::Done` option at
/// which point we can expect no further results.
///
/// 2. But in another sense can also represent a re-occuring operation that will be
/// delayed a specific period of time upon which after completionm, may or may not repeat.
///
/// Each response from the iterator is either a `Delayed::Pending` marking
/// an operation as still delayed and to be completed. It provides the following pieces:
///
/// 1. The instant of time such delay began
/// 2. The total duration things will be delayed from the start of time in (1).
/// 3. The total duration left till it is completed.
///
/// And a `Delayed::Done` indicate the finalization of the operation and the underlying
/// result.
pub trait DelayedIterator {
    type Item;

    /// Advances the iterator and returns the next value.
    fn next(&mut self) -> Option<Delayed<Self::Item>>;

    /// into_iter consumes the implementation and wraps
    /// it in an iterator type that emits `Multi<MutliIterator::Item>`
    /// match the behavior desired for an iterator.
    fn into_iter(self) -> impl Iterator<Item = Delayed<Self::Item>>
    where
        Self: Sized + 'static,
    {
        DelayedAsIterator(Box::new(self))
    }
}

pub struct DelayedAsIterator<T>(Box<dyn DelayedIterator<Item = T>>);

impl<T> DelayedAsIterator<T> {
    pub fn from_impl(t: impl DelayedIterator<Item = T> + 'static) -> Self {
        Self(Box::new(t))
    }

    pub fn new(t: Box<dyn DelayedIterator<Item = T>>) -> Self {
        Self(t)
    }
}

impl<T> AsDelayedIterator<T> for DelayedAsIterator<T> {}

impl<T> Iterator for DelayedAsIterator<T> {
    type Item = Delayed<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

/// SleepIterator implements a custom non-thread pausing sleep operation
/// which implements the `DelayedIterator`.
///
/// It will keep responding with a `Delayed::Pending` till the time marked
/// for sleeping to end which indicates to the caller to perform whatever
/// task they were waiting for.
pub struct SleepIterator<T>(std::time::Instant, std::time::Duration, Option<T>);

impl<T> SleepIterator<T> {
    pub fn new(from: std::time::Instant, until: std::time::Duration, value: T) -> Self {
        Self(from, until, Some(value))
    }

    pub fn until(duration: std::time::Duration, value: T) -> Self {
        let from = std::time::Instant::now();
        Self(from, duration, Some(value))
    }
}

impl<T> DelayedIterator for SleepIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Delayed<Self::Item>> {
        if self.2.is_none() {
            return None;
        }

        let now = std::time::Instant::now();
        let result = match self.0.checked_add(self.1) {
            Some(completed_at) => match completed_at.checked_duration_since(now) {
                Some(diff) => Delayed::Pending(self.0.clone(), self.1.clone(), diff),
                None => Delayed::Done(self.2.take().expect("item should not be taken yet")),
            },
            None => Delayed::Done(self.2.take().expect("item should not be taken yet")),
        };

        Some(result)
    }
}

#[cfg(test)]
mod test_sleep_iterator {
    use super::*;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use super::SleepIterator;

    #[test]
    fn can_sleep_thread_and_get_final() {
        let now = Instant::now();
        let wait = Duration::from_secs(3);
        let mut sleeper = SleepIterator::new(now, wait, ());

        assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));
        assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));
        assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

        thread::sleep(Duration::from_secs(1));

        assert!(matches!(sleeper.next(), Some(Delayed::Pending(_, _, _))));

        thread::sleep(Duration::from_secs(2));
        assert_eq!(sleeper.next(), Some(Delayed::Done(())));
    }
}
