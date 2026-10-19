//! Bridge parking through the real executor.
//!
//! WHY: `bridge_park.rs` drives each bridge by hand with a counting waker. It
//! never runs a bridged future as a valtron task, so it cannot catch a wake that
//! is registered but never makes the executor re-poll the parked future.
//! `foundation_netio`'s `async_caller_through_future_adapters` hung that way
//! after the stream-wake change (7394ae813): a fast producer pushed every status
//! and closed before the consumer registered its wake, so nothing fired.
//!
//! HOW: async `#[valtron_test]`s (the body runs through `block_on_future`) await
//! each bridge over `execute(producer)`. Producers emit every non-value status a
//! task can (`Pending`, `Delayed`, `Init`, `Wait`, `Ignore`) before and between
//! their values, either as fast as possible or with a pause before each status,
//! so the consumer both races the producer and parks waiting on it.

use std::pin::Pin;
use std::time::Duration;

use foundation_core::valtron::{
    execute, valtron_test, Ctx, NoAction, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// One status a producer emits.
#[derive(Clone, Copy, Debug)]
enum Step {
    Pending,
    Delayed(Duration),
    Init,
    Wait,
    Ignore,
    Value(u32),
}

/// Emits `steps` in order, pausing `gap` before each.
struct Producer {
    steps: Vec<Step>,
    gap: Duration,
}

impl Producer {
    fn new(steps: &[Step], gap: Duration) -> Self {
        Self {
            steps: steps.to_vec(),
            gap,
        }
    }
}

impl TaskIterator for Producer {
    type Ready = u32;
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(
        &mut self,
        _ctx: Ctx,
    ) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if !self.gap.is_zero() {
            std::thread::sleep(self.gap);
        }
        if self.steps.is_empty() {
            return None;
        }
        Some(match self.steps.remove(0) {
            Step::Pending => TaskStatus::Pending(()),
            Step::Delayed(d) => TaskStatus::Delayed(d),
            Step::Init => TaskStatus::Init,
            Step::Wait => TaskStatus::Wait,
            Step::Ignore => TaskStatus::Ignore,
            Step::Value(v) => TaskStatus::Ready(v),
        })
    }
}

fn values(steps: &[Step]) -> Vec<u32> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Value(v) => Some(*v),
            _ => None,
        })
        .collect()
}

/// Every non-value status, then values with statuses between them.
const MIXED: &[Step] = &[
    Step::Init,
    Step::Pending,
    Step::Wait,
    Step::Delayed(Duration::from_micros(200)),
    Step::Ignore,
    Step::Value(1),
    Step::Wait,
    Step::Pending,
    Step::Value(2),
    Step::Delayed(Duration::from_millis(2)),
    Step::Value(3),
    Step::Wait,
];

/// Statuses only after the last value: the consumer must still see the end.
const TRAILING: &[Step] = &[
    Step::Value(9),
    Step::Wait,
    Step::Delayed(Duration::from_millis(1)),
    Step::Pending,
    Step::Wait,
];

async fn drain_next_stream(producer: Producer) -> Vec<u32> {
    let mut stream = execute(producer, None).expect("execute").into_next_stream();
    let mut got = Vec::new();
    while let Some(item) = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await {
        got.push(item);
    }
    got
}

async fn drain_collect(producer: Producer) -> Vec<u32> {
    execute(producer, None)
        .expect("execute")
        .into_collect_future()
        .await
}

async fn first_ready(producer: Producer) -> Option<u32> {
    execute(producer, None)
        .expect("execute")
        .into_ready_future()
        .await
        .map(|(v, _rest)| v)
}

const FAST: Duration = Duration::ZERO;
const SLOW: Duration = Duration::from_millis(20);

#[valtron_test(threads = 4, timeout = 20_000)]
async fn next_stream_sees_every_value_and_the_end() {
    for gap in [FAST, SLOW] {
        for steps in [MIXED, TRAILING] {
            let got = drain_next_stream(Producer::new(steps, gap)).await;
            assert_eq!(got, values(steps), "gap {gap:?}, steps {steps:?}");
        }
    }
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn collect_future_sees_every_value_and_the_end() {
    for gap in [FAST, SLOW] {
        for steps in [MIXED, TRAILING] {
            let got = drain_collect(Producer::new(steps, gap)).await;
            assert_eq!(got, values(steps), "gap {gap:?}, steps {steps:?}");
        }
    }
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn ready_future_gets_the_first_value() {
    for gap in [FAST, SLOW] {
        let got = first_ready(Producer::new(MIXED, gap)).await;
        assert_eq!(got, Some(1), "gap {gap:?}");
    }
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn ready_future_ends_with_none_when_no_value_comes() {
    let steps = [
        Step::Pending,
        Step::Wait,
        Step::Delayed(Duration::from_millis(1)),
    ];
    for gap in [FAST, SLOW] {
        assert_eq!(
            first_ready(Producer::new(&steps, gap)).await,
            None,
            "gap {gap:?}"
        );
    }
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn pending_future_gets_the_first_pending() {
    let steps = [
        Step::Value(1),
        Step::Wait,
        Step::Init,
        Step::Pending,
        Step::Value(2),
    ];
    for gap in [FAST, SLOW] {
        let got = execute(Producer::new(&steps, gap), None)
            .expect("execute")
            .into_pending_future()
            .await
            .map(|(p, _rest)| p);
        assert_eq!(got, Some(()), "gap {gap:?}");
    }
}

/// The race is timing-dependent: repeat the fast case to give every
/// interleaving of producer and consumer a chance to occur.
#[valtron_test(threads = 4, timeout = 60_000)]
async fn fast_producers_never_strand_a_consumer() {
    for round in 0..200 {
        let got = drain_next_stream(Producer::new(MIXED, FAST)).await;
        assert_eq!(got, values(MIXED), "round {round}");
    }
}
