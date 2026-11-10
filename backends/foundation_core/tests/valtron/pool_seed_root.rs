//! The default engine root draws its RNG from the pool seed.
//!
//! WHY: `initialize_pool(seed, ..)` and `#[valtron_test(seed = ..)]` take a seed
//! so a run can repeat. The engine root used `Ctx::host`, which seeds the task
//! RNG from the entropy source, so `ctx.rng()` ignored the pool seed.
//! WHAT: a task reads one value from `ctx.rng()`. The value must equal the first
//! value of `Ctx::with_seed(_, seed)`. Both tests fail on the old code.

use std::sync::Arc;

use foundation_compact::rng::Rng;
use foundation_context::{Ctx, MonotonicClock};
use foundation_core::valtron::{
    execute, valtron_test, StreamIteratorExt, TaskIterator, TaskStatus,
};

const SEED: u64 = 4242;

/// The first value of the stream that `Ctx::with_seed` makes for `SEED`.
fn first_value_for_seed() -> u64 {
    Ctx::with_seed(Arc::new(MonotonicClock::new()), SEED)
        .rng()
        .lock()
        .expect("rng lock")
        .next_u64()
}

/// Yields one value from the task context RNG, then ends.
struct ReadsRng {
    done: bool,
}

impl TaskIterator for ReadsRng {
    type Ready = u64;
    type Pending = ();

    fn next_status(&mut self, ctx: Ctx) -> Option<TaskStatus<Self::Ready, Self::Pending>> {
        if self.done {
            return None;
        }
        self.done = true;
        let value = ctx.rng().lock().expect("rng lock").next_u64();
        Some(TaskStatus::Ready(value))
    }
}

#[valtron_test(seed = SEED)]
async fn the_default_root_rng_follows_the_pool_seed() {
    let got = execute(ReadsRng { done: false }, None)
        .expect("the macro starts the pool")
        .into_collect_future()
        .await;

    assert_eq!(
        got,
        vec![first_value_for_seed()],
        "the task RNG must start from the pool seed"
    );
}

#[valtron_test(seed = SEED, timeout = 10_000)]
async fn the_timeout_path_root_rng_follows_the_pool_seed() {
    let got = execute(ReadsRng { done: false }, None)
        .expect("the macro starts the pool")
        .into_collect_future()
        .await;

    assert_eq!(
        got,
        vec![first_value_for_seed()],
        "the task RNG must start from the pool seed on the timeout path"
    );
}
