//! The async-first valtron API, on native AND wasm: `.values()`, `.items()`,
//! `.collect_async()`, `.first()`, `TaskStatus::from_stream`, `next_values`.
//!
//! Every scenario is one `async fn`, registered twice by [`dual!`]: as a
//! `#[valtron_test]` natively and as a `#[valtron_bindgen]` async test on
//! wasm (pool initialized, the JS event loop drives it) (no blocking anywhere). Producers and consumers
//! share one task via `zip`, so "data arrives later" works the same on both
//! targets without threads.
//!
//! Besides the plain cases this deliberately covers odd ones: every control
//! state interleaved, empty and huge `Spread`s, streams that close while the
//! consumer is parked, consumers dropped mid-stream, unit items, 100k items,
//! two streams racing, a task built on `from_stream` fed after it starts, and
//! polling past the end.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{
    next_values, NotifyQueue, NotifyQueueStreamIterator, Stream, StreamIteratorExt, StreamSpread,
    TaskSpread, TaskStatus, WakeableStream,
};
use futures_lite::future::{yield_now, zip};
use futures_lite::StreamExt;

/// Register `$name` as an async test on the current target.
macro_rules! dual {
    ($name:ident, $body:path) => {
        #[cfg(not(target_family = "wasm"))]
        #[foundation_core::valtron::valtron_test]
        async fn $name() {
            $body().await;
        }

        #[cfg(target_family = "wasm")]
        #[foundation_macros::valtron_bindgen]
        async fn $name() {
            $body().await;
        }
    };
}

type Item = Stream<u32, char>;

/// A scripted stream: yields exactly the given items, then ends. It cannot
/// signal readiness (the default `WakeableStream`), like a plain iterator.
struct Script<D = u32, P = char> {
    items: VecDeque<Stream<D, P>>,
    pulls: Arc<AtomicUsize>,
}

impl<D, P> Script<D, P> {
    fn new(items: Vec<Stream<D, P>>) -> Self {
        Self {
            items: items.into(),
            pulls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl<D, P> Iterator for Script<D, P> {
    type Item = Stream<D, P>;
    fn next(&mut self) -> Option<Stream<D, P>> {
        self.pulls.fetch_add(1, Ordering::SeqCst);
        self.items.pop_front()
    }
}

#[cfg(feature = "multi")]
impl<D: Send + 'static, P: Send + 'static> WakeableStream for Script<D, P> {}
#[cfg(not(feature = "multi"))]
impl<D: 'static, P: 'static> WakeableStream for Script<D, P> {}

/// A queue-backed stream that signals readiness; it never waits on its own.
fn queue() -> (Arc<NotifyQueue<Item>>, NotifyQueueStreamIterator<u32, char>) {
    let q = Arc::new(NotifyQueue::unbounded());
    let it = NotifyQueueStreamIterator::new(Arc::clone(&q), 1, Duration::ZERO);
    (q, it)
}

/// Every control state, interleaved with values, in one stream.
fn everything() -> Vec<Item> {
    vec![
        Stream::Init,
        Stream::Ignore,
        Stream::Delayed(Duration::ZERO),
        Stream::Pending('w'),
        Stream::Next(1),
        Stream::Wait,
        Stream::Spread(vec![
            StreamSpread::Done(2),
            StreamSpread::Pending('h'),
            StreamSpread::Done(3),
        ]),
        Stream::Spread(vec![]),
        Stream::Pending('a'),
        Stream::Next(4),
        Stream::Ignore,
    ]
}

// ---------------------------------------------------------------------------
// .values()
// ---------------------------------------------------------------------------

async fn values_skip_every_control_state() {
    let got: Vec<u32> = Script::new(everything()).values().collect().await;
    assert_eq!(got, vec![1, 2, 3, 4]);
}
dual!(values_skip_every_control_state_test, values_skip_every_control_state);

async fn values_arrive_later_and_wake_the_consumer() {
    let (q, it) = queue();
    let consumer = it.values().collect::<Vec<_>>();
    let producer = async move {
        for i in 0..5 {
            yield_now().await; // the consumer is parked here
            q.push(Stream::Pending('b')).unwrap();
            q.push(Stream::Next(i)).unwrap();
        }
        yield_now().await;
        q.close();
    };
    let (got, ()) = zip(consumer, producer).await;
    assert_eq!(got, vec![0, 1, 2, 3, 4]);
}
dual!(values_arrive_later_test, values_arrive_later_and_wake_the_consumer);

async fn values_end_when_the_queue_closes_while_parked() {
    let (q, it) = queue();
    let producer = async move {
        yield_now().await;
        yield_now().await;
        q.close(); // no value ever pushed
    };
    let (got, ()) = zip(it.values().collect::<Vec<_>>(), producer).await;
    assert!(got.is_empty());
}
dual!(values_end_on_close_test, values_end_when_the_queue_closes_while_parked);

async fn a_hundred_thousand_values_stay_in_order() {
    let items: Vec<Item> = Iterator::collect((0..100_000).map(Stream::Next));
    let mut expected = 0;
    let mut values = Script::new(items).values();
    while let Some(v) = values.next().await {
        assert_eq!(v, expected);
        expected += 1;
    }
    assert_eq!(expected, 100_000);
}
dual!(hundred_thousand_values_test, a_hundred_thousand_values_stay_in_order);

async fn a_ten_thousand_item_spread_is_flattened_in_order() {
    let spread = (0..10_000)
        .map(|i| if i % 3 == 0 { StreamSpread::Pending('p') } else { StreamSpread::Done(i) })
        .collect();
    let got: Vec<u32> = Script::new(vec![Stream::Spread(spread)]).values().collect().await;
    let expected: Vec<u32> = (0..10_000).filter(|i| i % 3 != 0).collect();
    assert_eq!(got, expected);
}
dual!(huge_spread_test, a_ten_thousand_item_spread_is_flattened_in_order);

async fn unit_values_count_correctly() {
    let items: Vec<Stream<(), ()>> = vec![Stream::Next(()), Stream::Wait, Stream::Next(()), Stream::Next(())];
    let n = Script::new(items).values().count().await;
    assert_eq!(n, 3);
}
dual!(unit_values_test, unit_values_count_correctly);

async fn dropping_the_consumer_midway_leaves_the_producer_unharmed() {
    let (q, it) = queue();
    q.push(Stream::Next(1)).unwrap();
    q.push(Stream::Next(2)).unwrap();
    {
        let mut values = it.values();
        assert_eq!(values.next().await, Some(1));
    } // dropped with 2 still queued
    for i in 3..100 {
        q.push(Stream::Next(i)).unwrap(); // nobody listens; must not panic
    }
    assert!(q.close());
}
dual!(drop_consumer_midway_test, dropping_the_consumer_midway_leaves_the_producer_unharmed);

async fn a_stream_of_only_progress_has_no_values() {
    let items: Vec<Item> = vec![Stream::Pending('a'); 1_000];
    let got: Vec<u32> = Script::new(items).values().collect().await;
    assert!(got.is_empty());
}
dual!(only_progress_test, a_stream_of_only_progress_has_no_values);

async fn two_streams_race_without_losing_a_wake() {
    let (qa, a) = queue();
    let (qb, b) = queue();
    let consumers = zip(a.values().collect::<Vec<_>>(), b.values().collect::<Vec<_>>());
    let producer = async move {
        for i in 0..50 {
            // Alternate, and sometimes feed both before yielding.
            qa.push(Stream::Next(i)).unwrap();
            if i % 2 == 0 {
                yield_now().await;
            }
            qb.push(Stream::Next(i * 10)).unwrap();
            yield_now().await;
        }
        qa.close();
        yield_now().await;
        qb.close();
    };
    let ((ga, gb), ()) = zip(consumers, producer).await;
    assert_eq!(ga, (0..50).collect::<Vec<_>>());
    assert_eq!(gb, (0..50).map(|i| i * 10).collect::<Vec<_>>());
}
dual!(two_streams_race_test, two_streams_race_without_losing_a_wake);

// ---------------------------------------------------------------------------
// .items() / .collect_async() / .first()
// ---------------------------------------------------------------------------

async fn items_keep_values_in_order() {
    let got: Vec<Item> = Script::new(everything()).items().collect().await;
    let values: Vec<u32> = got
        .iter()
        .flat_map(|item| match item {
            Stream::Next(v) => vec![*v],
            Stream::Spread(s) => s
                .iter()
                .filter_map(|x| match x {
                    StreamSpread::Done(v) => Some(*v),
                    StreamSpread::Pending(_) => None,
                })
                .collect(),
            _ => vec![],
        })
        .collect();
    assert_eq!(values, vec![1, 2, 3, 4]);
    assert!(
        !got.iter().any(|i| matches!(i, Stream::Ignore)),
        "Ignore is never surfaced: {got:?}"
    );
}
dual!(items_keep_values_test, items_keep_values_in_order);

async fn collect_async_of_a_closed_empty_queue_is_empty() {
    let (q, it) = queue();
    q.close();
    assert!(it.collect_async().await.is_empty());
}
dual!(collect_async_empty_test, collect_async_of_a_closed_empty_queue_is_empty);

async fn collect_async_gathers_spread_and_next() {
    assert_eq!(Script::new(everything()).collect_async().await, vec![1, 2, 3, 4]);
}
dual!(collect_async_all_test, collect_async_gathers_spread_and_next);

async fn first_is_none_when_the_stream_ends_without_a_value() {
    let script: Script = Script::new(vec![Stream::Pending('x'), Stream::Ignore, Stream::Wait]);
    assert!(script.first().await.is_none());
}
dual!(first_none_test, first_is_none_when_the_stream_ends_without_a_value);

async fn first_then_the_rest_loses_nothing() {
    let script = Script::new(vec![
        Stream::Pending('x'),
        Stream::Next(1),
        Stream::Next(2),
        Stream::Spread(vec![StreamSpread::Done(3)]),
    ]);
    let (one, rest) = script.first().await.expect("a value");
    assert_eq!(one, 1);
    assert_eq!(rest.values().collect::<Vec<_>>().await, vec![2, 3]);
}
dual!(first_then_rest_test, first_then_the_rest_loses_nothing);

async fn first_waits_for_a_value_pushed_later() {
    let (q, it) = queue();
    let producer = async move {
        yield_now().await;
        q.push(Stream::Pending('s')).unwrap();
        yield_now().await;
        q.push(Stream::Next(42)).unwrap();
    };
    let (first, ()) = zip(it.first(), producer).await;
    assert_eq!(first.map(|(v, _)| v), Some(42));
}
dual!(first_waits_test, first_waits_for_a_value_pushed_later);

// ---------------------------------------------------------------------------
// TaskStatus::from_stream
// ---------------------------------------------------------------------------

async fn from_stream_maps_every_item() {
    let mut s = Script::new(vec![
        Stream::Next(1),
        Stream::Pending('p'),
        Stream::Delayed(Duration::from_millis(3)),
        Stream::Init,
        Stream::Ignore,
        Stream::Spread(vec![StreamSpread::Done(2), StreamSpread::Pending('q')]),
        Stream::Wait,
    ]);
    assert!(matches!(TaskStatus::from_stream(&mut s), Some(TaskStatus::Ready(1))));
    assert!(matches!(TaskStatus::from_stream(&mut s), Some(TaskStatus::Pending('p'))));
    assert!(matches!(
        TaskStatus::from_stream(&mut s),
        Some(TaskStatus::Delayed(d)) if d == Duration::from_millis(3)
    ));
    assert!(matches!(TaskStatus::from_stream(&mut s), Some(TaskStatus::Init)));
    assert!(matches!(TaskStatus::from_stream(&mut s), Some(TaskStatus::Ignore)));
    match TaskStatus::from_stream(&mut s) {
        Some(TaskStatus::Spread(items)) => assert!(matches!(
            items.as_slice(),
            [TaskSpread::Ready(2), TaskSpread::Pending('q')]
        )),
        other => panic!("expected Spread, got {:?}", other.map(|_| ())),
    }
    // A stream that cannot signal readiness: Wait stays a plain re-poll.
    assert!(matches!(TaskStatus::from_stream(&mut s), Some(TaskStatus::Wait)));
    // The end, and polling past the end, is None — every time.
    for _ in 0..3 {
        assert!(TaskStatus::from_stream(&mut s).is_none());
    }
}
dual!(from_stream_maps_test, from_stream_maps_every_item);

async fn from_stream_parks_a_waking_stream_until_data_arrives() {
    let (q, mut it) = queue();
    let Some(TaskStatus::Depends(signal)) = TaskStatus::from_stream(&mut it) else {
        panic!("an empty, open, waking stream parks on Depends");
    };
    assert!(!signal.is_ready(None), "not ready before any data");
    q.push(Stream::Next(7)).unwrap();
    assert!(signal.is_ready(None), "the push flips the signal");
    assert!(matches!(TaskStatus::from_stream(&mut it), Some(TaskStatus::Ready(7))));
    q.close();
    assert!(TaskStatus::from_stream(&mut it).is_none(), "closed and drained: the end");
}
dual!(from_stream_parks_test, from_stream_parks_a_waking_stream_until_data_arrives);

/// A task whose whole `next_status` is `from_stream` plus a doubling, run by
/// valtron and consumed with `.values()`, fed only after it has started.
struct Doubler {
    input: NotifyQueueStreamIterator<u32, char>,
}

impl foundation_core::valtron::TaskIterator for Doubler {
    type Ready = u32;
    type Pending = char;

    fn next_status(
        &mut self,
        _ctx: foundation_core::valtron::Ctx,
    ) -> Option<TaskStatus<u32, char>> {
        match TaskStatus::from_stream(&mut self.input)? {
            TaskStatus::Ready(v) => Some(TaskStatus::Ready(v * 2)),
            other => Some(other),
        }
    }
}

async fn a_task_built_on_from_stream_runs_end_to_end() {
    let (q, input) = queue();
    let output = foundation_core::valtron::execute(Doubler { input }, None).expect("scheduled");
    let producer = async move {
        for i in 1..=20 {
            yield_now().await;
            q.push(Stream::Next(i)).unwrap();
        }
        yield_now().await;
        q.close();
    };
    let (got, ()) = zip(output.values().collect::<Vec<_>>(), producer).await;
    assert_eq!(got, (1..=20).map(|i| i * 2).collect::<Vec<_>>());
}
dual!(task_on_from_stream_test, a_task_built_on_from_stream_runs_end_to_end);

// ---------------------------------------------------------------------------
// next_values (pure: no executor needed)
// ---------------------------------------------------------------------------

async fn next_values_skips_control_states_lazily() {
    let script = Script::new(everything());
    let pulls = Arc::clone(&script.pulls);
    let mut values = next_values(script);
    assert_eq!(values.next(), Some(1));
    assert_eq!(pulls.load(Ordering::SeqCst), 5, "pulled only up to the first value");
    assert_eq!(values.collect::<Vec<_>>(), vec![2, 3, 4]);
}
dual!(next_values_lazy_test, next_values_skips_control_states_lazily);

// ---------------------------------------------------------------------------
// Regression: an async consumer must not block in `next()` on an empty queue
// ---------------------------------------------------------------------------

/// `NotifyQueueStreamIterator::next` used to block for its timed waits on an
/// empty queue even for an async consumer, stalling the executor thread on
/// every park (5s for a 20-item task). With a 300 ms wait configured, 20
/// late-arriving values would take ~6 s that way; parked on the wake they
/// take one wait at most.
#[cfg(not(target_family = "wasm"))]
#[foundation_core::valtron::valtron_test]
async fn an_async_consumer_never_blocks_in_next() {
    let q: Arc<NotifyQueue<Item>> = Arc::new(NotifyQueue::unbounded());
    q.set_max_spins(1);
    let it = NotifyQueueStreamIterator::new(Arc::clone(&q), 1, Duration::from_millis(300));
    let started = foundation_compact::Instant::now();
    let producer = async move {
        for i in 0..20 {
            yield_now().await;
            q.push(Stream::Next(i)).unwrap();
        }
        yield_now().await;
        q.close();
    };
    let (got, ()) = zip(it.values().collect::<Vec<_>>(), producer).await;
    assert_eq!(got, (0..20).collect::<Vec<_>>());
    let took = started.elapsed();
    assert!(took < Duration::from_secs(3), "parked consumers blocked in next(): {took:?}");
}
