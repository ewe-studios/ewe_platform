//! Blocking consumers and the global timer read the clock of the `Ctx` they get.
//!
//! WHY: a blocking consumer (`NotifyQueue::wait_for_item`, the receive
//! iterators) and the timer thread used to build the real clock. A test could
//! not control their deadlines. GitHub issue #482.
//!
//! WHAT: each test uses a [`SimulatedClock`]. It moves only when the test calls
//! `advance`. A wait with a deadline on that clock must stay blocked while the
//! real time passes, and must end after the test advances the clock.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::Duration;

use foundation_context::Clock;
use foundation_context::Ctx;
use foundation_core::valtron::timer::{call_at, sleep_for, sleep_until};
use foundation_core::valtron::{
    NotificationItem, NotifyQueue, NotifyQueueConfig, NotifyQueueStreamIterator,
    NotifyRecvIterator, SimulatedClock, Stream,
};

/// Real time a blocked consumer must survive without ending.
const STILL_BLOCKED: Duration = Duration::from_millis(400);

/// How long a test waits for a consumer to end after the clock moved.
const ENDS_WITHIN: Duration = Duration::from_secs(10);

fn simulated() -> (Arc<SimulatedClock>, Ctx) {
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock.clone());
    (clock, ctx)
}

/// A queue whose single wait lasts `park` on the injected clock.
fn one_wait_queue<T>() -> Arc<NotifyQueue<T>> {
    Arc::new(NotifyQueue::unbounded_with_config(
        NotifyQueueConfig::default().with_max_spins(1),
    ))
}

/// Run `work` on a thread and report its result on a channel.
fn spawn_reporting<R: Send + 'static>(
    work: impl FnOnce() -> R + Send + 'static,
) -> mpsc::Receiver<R> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}

#[test]
fn wait_for_item_deadline_follows_the_injected_clock() {
    let (clock, ctx) = simulated();
    // The queue is built on the waiting thread: without the `multi` feature it is not `Send`.
    let done = spawn_reporting(move || {
        let queue: Arc<NotifyQueue<u32>> = one_wait_queue();
        queue.wait_for_item(Duration::from_millis(50), &ctx)
    });

    // The real time is far past the 50 ms wait, but the simulated clock stood still.
    assert!(
        done.recv_timeout(STILL_BLOCKED).is_err(),
        "the wait ended on real time; it must follow the injected clock"
    );

    clock.advance(Duration::from_secs(1));
    let item = done.recv_timeout(ENDS_WITHIN).expect("the wait must end");
    assert_eq!(item, Some(NotificationItem::None));
}

// A push from another thread needs a `Send` queue, so this test needs `multi`.
#[cfg(feature = "multi")]
#[test]
fn wait_for_item_still_returns_a_pushed_item_at_once() {
    let (_clock, ctx) = simulated();
    let queue: Arc<NotifyQueue<u32>> = one_wait_queue();
    let waiter = Arc::clone(&queue);
    let done = spawn_reporting(move || waiter.wait_for_item(Duration::from_secs(3600), &ctx));

    thread::sleep(Duration::from_millis(50));
    queue.push(7).expect("push");
    let item = done
        .recv_timeout(ENDS_WITHIN)
        .expect("a push ends the wait");
    assert_eq!(item, Some(NotificationItem::Ready(7)));
}

#[test]
fn stream_iterator_next_follows_the_injected_clock() {
    let (clock, ctx) = simulated();
    let done = spawn_reporting(move || {
        let queue: Arc<NotifyQueue<Stream<u32, ()>>> = one_wait_queue();
        let mut stream =
            NotifyQueueStreamIterator::new(queue, 1, Duration::from_millis(50), ctx.fork());
        stream.next().map(|item| matches!(item, Stream::Wait))
    });

    // The old code read the real clock here and returned `Wait` after 50 ms.
    assert!(
        done.recv_timeout(STILL_BLOCKED).is_err(),
        "`next` ended on real time; it must follow the injected clock"
    );

    clock.advance(Duration::from_secs(1));
    let item = done.recv_timeout(ENDS_WITHIN).expect("`next` must end");
    assert_eq!(item, Some(true), "`next` must report `Wait`");
}

#[test]
fn recv_iterator_next_follows_the_injected_clock() {
    let (clock, ctx) = simulated();
    let done = spawn_reporting(move || {
        let queue: Arc<NotifyQueue<u32>> = one_wait_queue();
        let mut iterator =
            NotifyRecvIterator::from_notify_queue(queue, Duration::from_millis(50), ctx.fork());
        iterator.next()
    });

    assert!(
        done.recv_timeout(STILL_BLOCKED).is_err(),
        "`next` ended on real time; it must follow the injected clock"
    );

    clock.advance(Duration::from_secs(1));
    let item = done.recv_timeout(ENDS_WITHIN).expect("`next` must end");
    assert_eq!(item, Some(NotificationItem::None));
}

/// A waker that records that the timer thread woke it.
struct Flag(AtomicBool);

impl Wake for Flag {
    fn wake(self: Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn flag_waker() -> (Arc<Flag>, Waker) {
    let flag = Arc::new(Flag(AtomicBool::new(false)));
    (flag.clone(), Waker::from(flag))
}

/// Wait until `flag` is set, or fail after `ENDS_WITHIN`.
fn wait_for_flag(flag: &Flag) {
    let started = std::time::Instant::now();
    while !flag.0.load(Ordering::SeqCst) {
        assert!(started.elapsed() < ENDS_WITHIN, "the timer never woke");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn sleep_for_follows_the_injected_clock() {
    let (clock, ctx) = simulated();
    let (flag, waker) = flag_waker();
    let mut cx = Context::from_waker(&waker);
    let mut sleep = Box::pin(sleep_for(Duration::from_secs(3600), &ctx));

    assert!(sleep.as_mut().poll(&mut cx).is_pending());
    thread::sleep(STILL_BLOCKED);
    assert!(
        !flag.0.load(Ordering::SeqCst),
        "the timer fired without the clock moving"
    );
    assert!(sleep.as_mut().poll(&mut cx).is_pending());

    // One hour passes at once on the simulated clock, with no real wait.
    clock.advance(Duration::from_secs(3600));
    wait_for_flag(&flag);
    assert_eq!(sleep.as_mut().poll(&mut cx), Poll::Ready(()));
}

#[test]
fn sleep_until_uses_a_deadline_on_the_injected_clock() {
    let (clock, ctx) = simulated();
    let (flag, waker) = flag_waker();
    let mut cx = Context::from_waker(&waker);
    let deadline = clock.now() + Duration::from_secs(60);
    let mut sleep = Box::pin(sleep_until(deadline, &ctx));

    assert!(sleep.as_mut().poll(&mut cx).is_pending());
    clock.advance(Duration::from_secs(61));
    wait_for_flag(&flag);
    assert_eq!(sleep.as_mut().poll(&mut cx), Poll::Ready(()));
}

#[test]
fn timers_on_two_clocks_stay_apart() {
    let (clock_a, ctx_a) = simulated();
    let (_clock_b, ctx_b) = simulated();
    let (flag_a, waker_a) = flag_waker();
    let (flag_b, waker_b) = flag_waker();
    let mut sleep_a = Box::pin(sleep_for(Duration::from_secs(10), &ctx_a));
    let mut sleep_b = Box::pin(sleep_for(Duration::from_secs(10), &ctx_b));
    assert!(sleep_a
        .as_mut()
        .poll(&mut Context::from_waker(&waker_a))
        .is_pending());
    assert!(sleep_b
        .as_mut()
        .poll(&mut Context::from_waker(&waker_b))
        .is_pending());

    clock_a.advance(Duration::from_secs(10));
    wait_for_flag(&flag_a);
    thread::sleep(Duration::from_millis(100));
    assert!(
        !flag_b.0.load(Ordering::SeqCst),
        "a timer on another clock fired"
    );
}

#[test]
fn call_at_follows_the_injected_clock() {
    let (clock, ctx) = simulated();
    let fired = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&fired);
    let _call = call_at(clock.now() + Duration::from_secs(5), &ctx, move || {
        seen.store(true, Ordering::SeqCst);
    });

    thread::sleep(STILL_BLOCKED);
    assert!(!fired.load(Ordering::SeqCst), "the callback ran too early");
    clock.advance(Duration::from_secs(5));
    let started = std::time::Instant::now();
    while !fired.load(Ordering::SeqCst) {
        assert!(started.elapsed() < ENDS_WITHIN, "the callback never ran");
        thread::sleep(Duration::from_millis(5));
    }
}
