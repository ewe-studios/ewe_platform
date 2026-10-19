//! # Synchronous state task (`#[valtron::task]`)
//!
//! The ergonomic payoff of valtron's state-task support: instead of writing a
//! full `TaskIterator` impl (see the `Counting` example at the top of
//! [`README.md`](../README.md)), you write a single **transition function** that
//! maps `(Ctx, state)` to the next state plus a `TaskStatus`. The `#[task]`
//! attribute generates a same-named struct wrapping it, and the transition stays
//! directly callable.
//!
//! This file is a *readable* reference copy. The compiled, runnable version is
//! [`foundation_core/examples/valtron_state_task.rs`][run]:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_state_task
//! ```
//!
//! [run]: ../../../examples/valtron_state_task.rs

use foundation_core::valtron::{execute, task, valtron, Ctx, StreamIteratorExt, TaskStatus};

/// The stages a house passes through on its way to completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Foundation,
    Walls,
    Roof,
    Done,
}

/// The finished product, emitted as the `Ready` value.
#[derive(Debug, PartialEq)]
struct House {
    walls: u32,
}

/// One building step per poll.
///
/// Intermediate steps return `TaskStatus::Ignore` ("nothing produced yet, keep
/// driving me"). `Ready` is **not** terminal: the final step emits
/// `Ready(House)` and advances to `Stage::Done`, whose `None` is what ends the
/// machine. The engine `Ctx` is available on every turn if you need it.
#[task]
fn build_house(ctx: Ctx, stage: Stage) -> Option<(Stage, TaskStatus<House, ()>)> {
    let _ = ctx;
    match stage {
        Stage::Foundation => Some((Stage::Walls, TaskStatus::Ignore)),
        Stage::Walls => Some((Stage::Roof, TaskStatus::Ignore)),
        Stage::Roof => Some((Stage::Done, TaskStatus::Ready(House { walls: 4 }))),
        Stage::Done => None,
    }
}

#[valtron]
async fn main() {
    // `build_house::new(stage)` is the generated struct — drive it like any task,
    // and await its `Ready` through the ready-future bridge (no collect-all).
    let (house, _rest) = execute(build_house::new(Stage::Foundation), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a house was built");
    assert_eq!(house, House { walls: 4 });
    println!("sync  build_house -> {house:?}");

    // The transition itself remains directly callable:
    let (next, status) = build_house(Ctx::new(), Stage::Walls).expect("transition step");
    assert_eq!(next, Stage::Roof);
    assert_eq!(status, TaskStatus::Ignore);
}
