//! `#[valtron::task]` state machines — one driven by an **enum** state, one by a
//! **struct** state — to show the transition function is agnostic to the shape of
//! the state it threads.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_state_task
//! ```
//!
//! The ergonomic payoff of valtron's state-task support: instead of writing a
//! full `TaskIterator` impl, you write a single **transition function** that maps
//! `(Ctx, state)` to the next state plus a `TaskStatus`. The `#[task]` attribute
//! generates a same-named struct with `new(state)`, and the transition stays
//! directly callable.
//!
//! `Ready` is **not** terminal: the machine emits it, threads the *next* state
//! back, and keeps going — only returning `None` ends it. That is what lets a
//! single transition cleanly express "one `Ready` value, then stop".

use foundation_core::valtron;
use foundation_core::valtron::{execute, task, valtron, Ctx, StreamIteratorExt, TaskStatus};

/// --- Enum state -----------------------------------------------------------

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

/// One building step per poll. The final step emits `Ready(House)` and advances
/// to `Stage::Done`; that variant's `None` is what ends the machine.
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

/// --- Struct state ---------------------------------------------------------

/// A struct state carrying the whole recipe's progress in its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Recipe {
    mixed: bool,
    baked: bool,
}

/// The finished product, emitted as the `Ready` value.
#[derive(Debug, PartialEq)]
struct Cake {
    layers: u32,
}

/// Advances the struct state one field at a time. `baked` doubles as the
/// completion marker: once it is set, the next turn returns `None`.
#[task]
fn bake_cake(ctx: Ctx, recipe: Recipe) -> Option<(Recipe, TaskStatus<Cake, ()>)> {
    let _ = ctx;
    match (recipe.mixed, recipe.baked) {
        (false, _) => Some((Recipe { mixed: true, ..recipe }, TaskStatus::Ignore)),
        (true, false) => Some((
            Recipe { mixed: true, baked: true },
            TaskStatus::Ready(Cake { layers: 3 }),
        )),
        (true, true) => None,
    }
}

#[valtron]
async fn main() {
    // Enum-state task, driven to its `Ready` via the ready-future bridge
    // (nothing is collected into a Vec).
    let (house, _rest) = execute(build_house::new(Stage::Foundation), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a house was built");
    assert_eq!(house, House { walls: 4 });

    // Struct-state task, driven the same way.
    let (cake, _rest) = execute(bake_cake::new(Recipe { mixed: false, baked: false }), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("a cake was baked");
    assert_eq!(cake, Cake { layers: 3 });

    println!("enum   build_house -> {house:?}");
    println!("struct bake_cake   -> {cake:?}");

    // The transition functions themselves remain directly callable:
    assert_eq!(
        build_house(Ctx::new(), Stage::Walls),
        Some((Stage::Roof, TaskStatus::Ignore))
    );
    assert_eq!(
        bake_cake(Ctx::new(), Recipe { mixed: false, baked: false }),
        Some((Recipe { mixed: true, baked: false }, TaskStatus::Ignore))
    );
}
