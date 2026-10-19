//! Compile-fail: the return tuple's state type must match the `state` argument.
use foundation_macros::task;

#[task]
fn state_mismatch(ctx: Ctx, state: u32) -> Option<(u64, TaskStatus<u32, ()>)> {
    let _ = ctx;
    None
}

fn main() {}
