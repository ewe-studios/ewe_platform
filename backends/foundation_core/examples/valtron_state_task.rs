//! Synchronous state-machine task with `#[valtron::task]`.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_state_task
//! ```
//!
//! The ergonomic payoff of valtron's state-task support: instead of writing a
//! full `TaskIterator` impl, you write a single **transition function** that
//! maps `(Ctx, state)` to the next state plus a `TaskStatus`. The `#[task]`
//! attribute generates a same-named struct that wraps it, and the transition
//! stays directly callable.

// The engine macros expand to `crate::valtron::…` when they run inside this
// package's own example targets (`proc_macro_crate` reports `Itself`), so bring
// the *module* name into the example's crate root to make that path resolve.
use foundation_core::valtron;
use foundation_core::valtron::{sync_one, task, valtron, Ctx, TaskStatus};

/// The stages a house passes through on its way to completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Foundation,
    Walls,
    Roof,
}

/// The finished product, emitted as the terminal `Ready` value.
#[derive(Debug, PartialEq)]
struct House {
    walls: u32,
}

/// One building step per poll.
///
/// Intermediate steps return `TaskStatus::Ignore` ("nothing produced yet, keep
/// driving me"); the final step returns `TaskStatus::Ready(House)`, which is
/// terminal. The engine `Ctx` is available on every turn if you need it.
#[task]
fn build_house(ctx: Ctx, stage: Stage) -> Option<(Stage, TaskStatus<House, ()>)> {
    let _ = ctx;
    match stage {
        Stage::Foundation => Some((Stage::Walls, TaskStatus::Ignore)),
        Stage::Walls => Some((Stage::Roof, TaskStatus::Ignore)),
        Stage::Roof => Some((Stage::Roof, TaskStatus::Ready(House { walls: 4 }))),
    }
}

#[valtron]
fn main() {
    // `build_house::new(stage)` is the generated struct — drive it like any task.
    let houses = sync_one(build_house::new(Stage::Foundation)).expect("engine live");
    assert_eq!(houses, vec![House { walls: 4 }]);
    println!("sync  build_house -> {houses:?}");

    // The transition itself remains directly callable:
    let (next, status) = build_house(Ctx::new(), Stage::Walls).expect("transition step");
    assert_eq!(next, Stage::Roof);
    assert_eq!(status, TaskStatus::Ignore);
}
