//! Bridge parking through the real executor.
//!
//! WHY: `bridge_park.rs` drives each bridge by hand with a counting waker. It
//! never runs a bridged future as a valtron task, so it cannot catch a wake that
//! is registered correctly but never makes the executor re-poll the parked
//! future. `foundation_netio`'s `async_caller_through_future_adapters` hung that
//! way after the stream-wake change (7394ae813).
//!
//! HOW: an async `#[valtron_test]` (so the body runs through `block_on_future`)
//! awaits `execute(task).into_next_stream()`. The producer task yields `Pending`
//! for a while, and in the slow variant sleeps between items, so the consumer
//! must park and be woken by the producer's pushes and by the final close.

use std::pin::Pin;
use std::time::Duration;

use foundation_core::valtron::{
    execute, valtron_test, NoAction, StreamIteratorExt, TaskIterator, TaskStatus,
};
use futures_core::Stream as FuturesStream;

/// Yields `pendings` `Pending` statuses, then each of `items` as `Ready`,
/// sleeping `gap` before every status.
struct Producer {
    pendings: usize,
    items: Vec<u32>,
    gap: Duration,
}

impl TaskIterator for Producer {
    type Ready = u32;
    type Pending = ();
    type Spawner = NoAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        if !self.gap.is_zero() {
            std::thread::sleep(self.gap);
        }
        if self.pendings > 0 {
            self.pendings -= 1;
            return Some(TaskStatus::Pending(()));
        }
        if self.items.is_empty() {
            return None;
        }
        Some(TaskStatus::Ready(self.items.remove(0)))
    }
}

async fn drain(producer: Producer) -> Vec<u32> {
    let mut stream = execute(producer, None).expect("execute").into_next_stream();
    let mut got = Vec::new();
    while let Some(item) = std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await {
        got.push(item);
    }
    got
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn next_stream_in_block_on_future_sees_every_item_and_the_end() {
    let got = drain(Producer {
        pendings: 3,
        items: vec![1, 2, 3],
        gap: Duration::ZERO,
    })
    .await;
    assert_eq!(got, vec![1, 2, 3]);
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn next_stream_parks_and_is_woken_by_a_slow_producer() {
    let got = drain(Producer {
        pendings: 2,
        items: vec![10, 20],
        gap: Duration::from_millis(50),
    })
    .await;
    assert_eq!(got, vec![10, 20]);
}

#[valtron_test(threads = 4, timeout = 20_000)]
async fn next_stream_ends_when_the_task_finishes_after_its_last_item() {
    // The end of the stream arrives on its own, well after the last item: the
    // consumer is parked on an empty, still-open queue when the close lands.
    let got = drain(Producer {
        pendings: 0,
        items: vec![7],
        gap: Duration::from_millis(100),
    })
    .await;
    assert_eq!(got, vec![7]);
}
