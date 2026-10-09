//! Compile-fail: stacking the attribute is rejected — one invocation owns the
//! whole set, so the fix is one `{ ... }` block per container.
use foundation_macros::docker_container;

#[docker_container(image = "redis:7-alpine", as = "cache")]
#[docker_container(image = "postgres:16", as = "db")]
fn stacked(_: ()) {}

fn main() {}
