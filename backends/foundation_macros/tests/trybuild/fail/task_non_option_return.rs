//! Compile-fail: `#[task]` transition must return `Option<...>`.
use foundation_macros::task;

#[task]
fn no_option(ctx: Ctx, state: u32) -> (u32, TaskStatus<u32, ()>) {
    let _ = ctx;
    (state, TaskStatus::Ignore)
}

fn main() {}
