//! Compile-fail: `#[task]` does not support generic transitions (yet).
use foundation_macros::task;

#[task]
fn generic<T>(ctx: Ctx, state: T) -> Option<(T, TaskStatus<u32, ()>)> {
    let _ = ctx;
    None
}

fn main() {}
