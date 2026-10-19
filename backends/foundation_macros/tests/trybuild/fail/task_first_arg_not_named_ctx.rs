//! Compile-fail: the first `#[task]` argument must be named `ctx`.
use foundation_macros::task;

#[task]
fn bad_first_name(context: Ctx, state: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = context;
    None
}

fn main() {}
