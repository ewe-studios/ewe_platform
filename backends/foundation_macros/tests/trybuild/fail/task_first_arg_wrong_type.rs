//! Compile-fail: the first `#[task]` argument must have type `Ctx`.
use foundation_macros::task;

#[task]
fn bad_ctx_type(ctx: u32, state: u32) -> Option<(u32, TaskStatus<u32, ()>)> {
    let _ = ctx;
    None
}

fn main() {}
