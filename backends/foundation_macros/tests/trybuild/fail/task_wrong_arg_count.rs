//! Compile-fail: `#[task]` transition must take exactly two arguments.
use foundation_macros::task;

#[task]
fn three_args(ctx: Ctx, a: u32, b: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = (a, b);
    None
}

fn main() {}
